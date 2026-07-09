//! [`JurisdictionAdapter`] implementation for the `br` regime: discover +
//! fetch the nationwide TSE bulk ZIPs (plan.md "Politeness config"), parse
//! (pure `serde_json` over the fetch-time join — no scraping, no LLM seam),
//! normalize (Silver → Gold).
//!
//! Bronze-doc granularity is a genuine architectural decision this regime
//! forces that no prior adapter needed (plan.md's "Row unit" leaves the
//! exact join-field set, and implicitly Bronze-doc shape, to rust-builder):
//! TSE never serves one candidate's declaration as an individually
//! addressable document — `bem_candidato`/`consulta_cand` each ship as ONE
//! nationwide ZIP covering every candidate. Since [`JurisdictionAdapter`]
//! expects one `fetch()` call to produce one Bronze document for one
//! [`FilingRef`], `discover()` does the real work here: it downloads both
//! nationwide ZIPs once, unzips every per-UF CSV (Latin-1 → UTF-8), joins
//! `bem_candidato` rows to their `consulta_cand` candidate by `SQ_CANDIDATO`,
//! and caches the joined per-candidate JSON (the same
//! `{"consulta_cand": {...}, "bem_candidato": [...]}` shape the fixtures use)
//! in-process, keyed by `FilingRef.external_id`. `fetch()` then just
//! re-serializes the cached join to Bronze — it does not re-hit the
//! network. This requires `discover()` to run before `fetch()` in the same
//! adapter instance (true of the in-process `Runner`, `pipeline::run`); a
//! cache miss (e.g. a bare `fetch()` call) fails closed rather than
//! guessing.

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::{DateTime, Datelike as _, Utc};
use serde::de::DeserializeOwned;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::normalize;
use crate::parse::{self, BemCandidato, ConsultaCand, SourceDoc};

/// Brazil's 27 official UF (state) abbreviations. TSE's nationwide ZIP ships
/// one CSV per UF plus a nationwide aggregate whose suffix is neither of
/// these (`MANIFEST.json uf_zip_pattern_correction`) — filtering to this
/// whitelist is how [`read_csv_entries_by_uf`] skips the aggregate without
/// depending on guessing its exact filename.
const BRAZIL_UF_CODES: &[&str] = &[
    "AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA", "MT", "MS", "MG", "PA", "PB", "PR",
    "PE", "PI", "RJ", "RN", "RS", "RO", "RR", "SC", "SP", "SE", "TO",
]; // 26 states + DF (Distrito Federal), the whole federation.

/// Conditional-GET validators for one nationwide ZIP (plan.md "Politeness
/// config": `cdn.tse.jus.br` confirmed to return `ETag`/`Last-Modified`).
#[derive(Debug, Default, Clone)]
struct ZipValidators {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// The Brazil TSE candidacy-time asset-declaration adapter.
#[derive(Debug, Default)]
pub struct BrAdapter {
    /// Conditional-GET validators keyed by ZIP url — one nationwide ZIP per
    /// (dataset, year), re-polled conditionally on a later `discover()` call
    /// (`us_house` index-validator precedent).
    zip_validators: Mutex<HashMap<String, ZipValidators>>,
    /// Per-candidate joined declarations materialized by the most recent
    /// `discover()` call, keyed by `FilingRef.external_id` — see the module
    /// doc comment for why `fetch()` needs this instead of re-hitting a
    /// per-candidate URL.
    joined_cache: Mutex<HashMap<String, serde_json::Value>>,
}

#[async_trait]
impl JurisdictionAdapter for BrAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef { code: "br" }
    }

    fn politeness(&self) -> PolitenessCfg {
        // plan.md "Politeness config": concurrency 1 (cfg default), 2s
        // between requests, identified UA — matches every other adapter's
        // convention.
        PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com")
    }

    /// Live discovery targets the most recent federal general-election
    /// cycle (plan.md scope: quadrennial, `DEPUTADO FEDERAL`/`SENADOR` are
    /// elected together — AUTHORITY.md `cadence_and_lag`). Historical
    /// backfill across earlier cycles is [`BrAdapter::discover_year`],
    /// called directly (the `us_house` backfill-bin precedent).
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let year = latest_federal_election_year(ctx.clock.now());
        self.discover_year(year, ctx).await
    }

    /// Re-serializes the cached joined declaration (see module doc comment)
    /// to Bronze. Does not perform network I/O — the bulk ZIPs were already
    /// fetched by `discover()`.
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let joined = self
            .joined_cache
            .lock()
            .map_err(|_| anyhow::anyhow!("joined declaration cache lock poisoned"))?
            .get(&r.external_id)
            .cloned()
            .with_context(|| {
                format!(
                    "no joined declaration cached for {} — discover() must run in the same \
                     process before fetch() (bem_candidato ships as one bulk nationwide ZIP, \
                     no per-candidate URL; see crate::adapter module doc comment)",
                    r.external_id
                )
            })?;
        let bytes = serde_json::to_vec(&joined).context("serializing joined declaration")?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let text = std::str::from_utf8(&bytes)
            .context("candidate declaration document is not UTF-8 — contract drift, freeze")?;
        // Production threads the politician-resolution PII passthrough only
        // when a real pool is present (plan.md "Row unit" tension — see
        // crate::parse::SilverRow doc comment); conformance (pool: None)
        // keeps expected.silver.json byte-exact.
        let scored = parse::parse_document(text, ctx.pool.is_some())
            .with_context(|| format!("parsing candidate declaration {}", d.sha256))?;
        scored
            .into_iter()
            .map(|scored| {
                Ok(StagingRow {
                    payload: serde_json::to_value(&scored.row)
                        .context("serializing staging payload")?,
                    confidence: scored.confidence,
                })
            })
            .collect()
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        normalize::normalize_rows(rows, ctx)
    }
}

impl BrAdapter {
    /// Discovers every in-scope candidate declaration for one federal
    /// election `year`: downloads (conditionally) both nationwide ZIPs,
    /// joins `bem_candidato` to `consulta_cand` by `SQ_CANDIDATO`, filters to
    /// `DS_CARGO` in scope, and caches each joined declaration for `fetch()`
    /// (module doc comment). A zero-asset candidate still gets a
    /// [`FilingRef`] with an empty `bem_candidato` array (plan.md edge case
    /// 1 — a legitimate outcome, discovered like any other candidacy).
    ///
    /// Returns an empty result when neither nationwide ZIP changed since the
    /// last poll for this year (both conditional GETs 304) — mirroring the
    /// `us_house` index-304 convention.
    ///
    /// # Errors
    /// A bulk-ZIP GET transport failure or non-success status, an
    /// unparseable ZIP/CSV, or a joined-declaration cache lock failure.
    pub async fn discover_year(&self, year: i32, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let Some(consulta_bytes) = self
            .fetch_zip_conditional(ctx, &zip_url("consulta_cand", year))
            .await?
        else {
            return Ok(Vec::new());
        };
        let Some(bem_bytes) = self
            .fetch_zip_conditional(ctx, &zip_url("bem_candidato", year))
            .await?
        else {
            return Ok(Vec::new());
        };
        let candidates: Vec<ConsultaCand> = read_csv_entries_by_uf(&consulta_bytes)?;
        let assets: Vec<BemCandidato> = read_csv_entries_by_uf(&bem_bytes)?;

        let mut assets_by_candidate: HashMap<String, Vec<BemCandidato>> = HashMap::new();
        for asset in assets {
            assets_by_candidate
                .entry(asset.sq_candidato.clone())
                .or_default()
                .push(asset);
        }

        let mut cache = self
            .joined_cache
            .lock()
            .map_err(|_| anyhow::anyhow!("joined declaration cache lock poisoned"))?;
        let mut refs = Vec::new();
        for candidate in candidates {
            if !parse::IN_SCOPE_CARGOS.contains(&candidate.ds_cargo.as_str()) {
                continue;
            }
            let external_id = format!("{year}:{}", candidate.sq_candidato);
            let bem_candidato = assets_by_candidate
                .remove(&candidate.sq_candidato)
                .unwrap_or_default();
            let joined = SourceDoc {
                consulta_cand: candidate,
                bem_candidato,
            };
            let value = serde_json::to_value(&joined).context("serializing joined declaration")?;
            cache.insert(external_id.clone(), value);
            refs.push(FilingRef {
                external_id,
                url: zip_url("bem_candidato", year),
            });
        }
        Ok(refs)
    }

    /// Conditional GET of one nationwide ZIP; `Ok(None)` on a 304 (unchanged
    /// since the last poll for this exact url).
    async fn fetch_zip_conditional(
        &self,
        ctx: &RunCtx,
        url: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let cached = self
            .zip_validators
            .lock()
            .map_err(|_| anyhow::anyhow!("zip validator lock poisoned"))?
            .get(url)
            .cloned()
            .unwrap_or_default();
        let response = ctx
            .http
            .get_conditional(url, cached.etag.as_deref(), cached.last_modified.as_deref())
            .await?;
        if response.status().as_u16() == 304 {
            return Ok(None);
        }
        anyhow::ensure!(
            response.status().is_success(),
            "bulk ZIP GET {url} -> {}",
            response.status()
        );
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let fresh = ZipValidators {
            etag: header("etag"),
            last_modified: header("last-modified"),
        };
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading bulk ZIP body of {url}"))?;
        self.zip_validators
            .lock()
            .map_err(|_| anyhow::anyhow!("zip validator lock poisoned"))?
            .insert(url.to_owned(), fresh);
        Ok(Some(bytes.to_vec()))
    }
}

/// `https://cdn.tse.jus.br/estatistica/sead/odsele/{dataset}/{dataset}_{year}.zip`
/// — NO `[_{UF}]` suffix (plan.md politeness section / `MANIFEST.json`
/// `uf_zip_pattern_correction`: confirmed 404; per-UF CSVs ship inside the
/// single nationwide ZIP).
fn zip_url(dataset: &str, year: i32) -> String {
    format!("https://cdn.tse.jus.br/estatistica/sead/odsele/{dataset}/{dataset}_{year}.zip")
}

/// Most recent federal general-election year (`year % 4 == 2`: 2018, 2022,
/// 2026, …) not after `now` — AUTHORITY.md `cadence_and_lag`: Câmara/Senado
/// elect on this quadrennial federal calendar, offset by 2 years from the
/// municipal Prefeito/Vereador cycle. A simplifying heuristic (real
/// production scheduling — exactly when a given cycle's bulk data goes live
/// relative to election day — is a runner/scheduler concern, not resolved
/// here).
fn latest_federal_election_year(now: DateTime<Utc>) -> i32 {
    let mut year = now.year();
    while year.rem_euclid(4) != 2 {
        year -= 1;
    }
    year
}

/// Unzips every per-UF CSV entry (skipping the nationwide aggregate via the
/// [`BRAZIL_UF_CODES`] whitelist) and deserializes each row by CSV header
/// name into `T` (`;`-delimited, quoted — plan.md "Encoding"). Source bytes
/// are Latin-1 (ISO-8859-1); [`latin1_to_string`] transcodes before parsing.
fn read_csv_entries_by_uf<T: DeserializeOwned>(zip_bytes: &[u8]) -> anyhow::Result<Vec<T>> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).context("opening bulk ZIP")?;
    let mut rows = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("reading ZIP entry")?;
        let name = entry.name().to_owned();
        let Some(stem) = name.strip_suffix(".csv") else {
            continue; // e.g. leiame.pdf
        };
        let Some(uf) = stem.rsplit('_').next() else {
            continue;
        };
        if !BRAZIL_UF_CODES.contains(&uf) {
            continue; // nationwide aggregate entry, not a per-UF file
        }
        let mut latin1 = Vec::new();
        entry
            .read_to_end(&mut latin1)
            .with_context(|| format!("reading {name}"))?;
        let text = latin1_to_string(&latin1);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_reader(text.as_bytes());
        for record in reader.deserialize::<T>() {
            rows.push(record.with_context(|| format!("parsing a row of {name}"))?);
        }
    }
    Ok(rows)
}

/// ISO-8859-1 (Latin-1) → UTF-8: every byte maps directly onto the Unicode
/// scalar value of the same ordinal (an infallible, total mapping, unlike
/// Windows-1252) — plan.md "Encoding".
fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn politeness_is_concurrency_one_with_two_second_spacing() {
        let cfg = BrAdapter::default().politeness();
        assert_eq!(cfg.concurrency, 1, "invariant 10");
        assert_eq!(cfg.min_interval, Duration::from_secs(2));
        assert!(cfg.user_agent().contains("ssm.leo@outlook.com"));
    }

    #[test]
    fn zip_urls_have_no_uf_suffix() {
        assert_eq!(
            zip_url("consulta_cand", 2022),
            "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
        );
        assert_eq!(
            zip_url("bem_candidato", 2022),
            "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
        );
    }

    #[test]
    fn latest_federal_election_year_lands_on_quadrennial_years() {
        let at = |y: i32, m: u32, d: u32| {
            DateTime::parse_from_rfc3339(&format!("{y:04}-{m:02}-{d:02}T00:00:00Z"))
                .unwrap()
                .with_timezone(&Utc)
        };
        assert_eq!(latest_federal_election_year(at(2026, 7, 6)), 2026);
        assert_eq!(latest_federal_election_year(at(2025, 1, 1)), 2022);
        assert_eq!(latest_federal_election_year(at(2023, 12, 31)), 2022);
    }

    #[test]
    fn latin1_transcodes_every_byte_value() {
        // 0xE7 is Latin-1 "ç" (as in "quinhões" — plan.md "Encoding").
        assert_eq!(
            latin1_to_string(&[0x71, 0x75, 0x6F, 0x74, 0x61, 0xE7, 0xE3, 0x6F]),
            "quotação"
        );
    }

    #[test]
    fn brazil_uf_whitelist_has_27_units() {
        assert_eq!(BRAZIL_UF_CODES.len(), 27, "26 states + DF");
        assert!(
            !BRAZIL_UF_CODES.contains(&"BR"),
            "nationwide aggregate excluded"
        );
    }

    /// Builds an in-memory single-entry ZIP, mirroring [`read_csv_entries_by_uf`]'s
    /// real per-UF-CSV-inside-one-nationwide-ZIP shape closely enough to
    /// exercise it end to end.
    fn zip_with_one_csv(entry_name: &str, csv_body: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            writer
                .start_file(entry_name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, csv_body.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        buf
    }

    /// The 2006/2010 column-rename fork (`BemCandidato`'s own doc comment,
    /// `docs/regimes/br/AUTHORITY.md` `open_questions`, goal 093 Phase 2):
    /// `NR_ORDEM_CANDIDATO`/`DT_ULTIMA_ATUALIZACAO`/`HH_ULTIMA_ATUALIZACAO`
    /// instead of the 2014+ column names, everything else identical. Header/
    /// values here mirror the REAL 2010 `bem_candidato_2010_AC.csv` shape
    /// (column order, extra `DT_GERACAO`/`CD_TIPO_ELEICAO`/`SG_UE`/`NM_UE`
    /// metadata columns this struct doesn't model), confirmed by directly
    /// downloading and inspecting that file this session — synthetic
    /// candidate/value content, not a real person's data.
    #[test]
    fn legacy_2006_2010_bem_candidato_header_deserializes_via_alias() {
        let csv = "\"DT_GERACAO\";\"HH_GERACAO\";\"ANO_ELEICAO\";\"CD_TIPO_ELEICAO\";\"NM_TIPO_ELEICAO\";\"CD_ELEICAO\";\"DS_ELEICAO\";\"DT_ELEICAO\";\"SG_UF\";\"SG_UE\";\"NM_UE\";\"SQ_CANDIDATO\";\"NR_ORDEM_CANDIDATO\";\"CD_TIPO_BEM_CANDIDATO\";\"DS_TIPO_BEM_CANDIDATO\";\"DS_BEM_CANDIDATO\";\"VR_BEM_CANDIDATO\";\"DT_ULTIMA_ATUALIZACAO\";\"HH_ULTIMA_ATUALIZACAO\"\n\
             \"19/04/2021\";\"13:16:05\";2010;2;\"Eleição Ordinária\";37;\"Eleições 2010\";\"03/10/2010\";\"AC\";\"AC\";\"ACRE\";10000000122;1;12;\"Casa\";\"Casa de teste\";\"297185,71\";\"05/07/2010\";\"17:56:37\"\n";
        let zip_bytes = zip_with_one_csv("bem_candidato_2010_AC.csv", csv);
        let rows: Vec<BemCandidato> = read_csv_entries_by_uf(&zip_bytes).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sq_candidato, "10000000122");
        assert_eq!(
            rows[0].nr_ordem_bem_candidato, "1",
            "via NR_ORDEM_CANDIDATO alias"
        );
        assert_eq!(
            rows[0].dt_ult_atual_bem_candidato, "05/07/2010",
            "via DT_ULTIMA_ATUALIZACAO alias"
        );
        assert_eq!(
            rows[0].hh_ult_atual_bem_candidato, "17:56:37",
            "via HH_ULTIMA_ATUALIZACAO alias"
        );
        assert_eq!(rows[0].vr_bem_candidato, "297185,71");
    }

    /// The 2014+ modern header must still deserialize unchanged (the alias
    /// is additive — proves this fix cannot regress the already-real
    /// 2014/2018/2022 data).
    #[test]
    fn modern_2014_plus_bem_candidato_header_still_deserializes() {
        let csv = "\"SQ_CANDIDATO\";\"DT_ELEICAO\";\"ANO_ELEICAO\";\"NR_ORDEM_BEM_CANDIDATO\";\"CD_TIPO_BEM_CANDIDATO\";\"DS_TIPO_BEM_CANDIDATO\";\"DS_BEM_CANDIDATO\";\"VR_BEM_CANDIDATO\";\"DT_ULT_ATUAL_BEM_CANDIDATO\";\"HH_ULT_ATUAL_BEM_CANDIDATO\"\n\
             \"10001595344\";\"02/10/2022\";\"2022\";\"1\";\"12\";\"Casa\";\"Casa de teste\";\"10000,00\";\"02/10/2022\";\"23:21:28\"\n";
        let zip_bytes = zip_with_one_csv("bem_candidato_2022_AC.csv", csv);
        let rows: Vec<BemCandidato> = read_csv_entries_by_uf(&zip_bytes).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nr_ordem_bem_candidato, "1");
        assert_eq!(rows[0].dt_ult_atual_bem_candidato, "02/10/2022");
    }
}
