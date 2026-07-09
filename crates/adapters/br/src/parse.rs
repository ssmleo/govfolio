//! Bronze → Silver (plan.md "Parse strategy & rationale"). Bronze here holds
//! one candidate's whole declaration, joined at fetch time from the two TSE
//! bulk CSVs (`consulta_cand` + `bem_candidato`) — the same
//! `{"consulta_cand": {...}, "bem_candidato": [...]}` shape whether the join
//! was performed by the real adapter's `fetch()` (`crate::adapter`,
//! production) or pre-packaged directly into the fixture (fixtures
//! `MANIFEST.json` `packaging_note`). `parse()` itself is a pure,
//! deterministic JSON deserialize + row emission — no scraping, no LLM seam
//! (plan.md).
//!
//! Row unit: one Silver row per `bem_candidato` item; a candidate with zero
//! items is a legitimate "no assets declared" outcome (plan.md edge case 1),
//! not a parse failure — the empty-array case simply yields an empty `Vec`.

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

/// Extractor id recorded on every Silver row and echoed as Gold's
/// `extracted_by` (fixtures `MANIFEST.json` `extractor_convention`).
pub(crate) const EXTRACTOR: &str = "br_bem_candidato/csv@1";

/// `DS_CARGO` values in this regime's scope (`docs/regimes/br/AUTHORITY.md`
/// scope: Câmara + Senado). Discovery is meant to filter to these already
/// (plan.md field-mapping table); `parse()` re-checks and fails closed
/// rather than silently promoting an out-of-scope body — defense in depth,
/// matching the `uk_commons_register` §3.8 check 3 precedent.
pub(crate) const IN_SCOPE_CARGOS: &[&str] =
    &["DEPUTADO FEDERAL", "SENADOR", "1º SUPLENTE", "2º SUPLENTE"];

const BASE_CONFIDENCE: f32 = 1.0;

/// One joined candidate declaration (fixtures' `input.json` shape / the real
/// adapter's fetch-time join, plan.md "Row unit").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceDoc {
    pub(crate) consulta_cand: ConsultaCand,
    #[serde(default)]
    pub(crate) bem_candidato: Vec<BemCandidato>,
}

/// The slice of `consulta_cand` columns this regime needs (join key, scope
/// filter, production-only PII passthrough — plan.md field-mapping table);
/// the source row carries dozens more columns, left unmodeled and ignored
/// (not `deny_unknown_fields` — a wide CSV row, not a fixed API contract).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConsultaCand {
    #[serde(rename = "SQ_CANDIDATO")]
    pub(crate) sq_candidato: String,
    /// Candidate name — public disclosure content, not PII (see
    /// [`SilverRow::nm_candidato`]).
    #[serde(rename = "NM_CANDIDATO")]
    pub(crate) nm_candidato: String,
    /// Candidate's state — this regime's district-equivalent (see
    /// [`SilverRow::sg_uf`]).
    #[serde(rename = "SG_UF")]
    pub(crate) sg_uf: String,
    #[serde(rename = "DS_CARGO")]
    pub(crate) ds_cargo: String,
    #[serde(rename = "NR_TITULO_ELEITORAL_CANDIDATO")]
    pub(crate) nr_titulo_eleitoral_candidato: String,
    #[serde(rename = "NR_CPF_CANDIDATO")]
    pub(crate) nr_cpf_candidato: String,
}

/// One `bem_candidato` asset-line-item row (plan.md field-mapping table).
/// Field names mirror the source column vocabulary verbatim (Silver keeps
/// the source's own vocabulary) — several end in `_bem_candidato`, the same
/// suffix as the struct's own name (`uk_commons_register` `Category`/`Member`
/// precedent for this exact lint).
///
/// **2006/2010 column-rename fork** (`docs/regimes/br/AUTHORITY.md`
/// `open_questions`, confirmed by direct download+inspection of the real
/// `bem_candidato_2006.zip`/`bem_candidato_2010.zip` headers, goal 093 Phase
/// 2): those two years use `NR_ORDEM_CANDIDATO`/`DT_ULTIMA_ATUALIZACAO`/
/// `HH_ULTIMA_ATUALIZACAO` instead of this struct's 2014+ column names for
/// the same three fields — every other column this struct reads
/// (`SQ_CANDIDATO`, `DT_ELEICAO`, `ANO_ELEICAO`, `CD_TIPO_BEM_CANDIDATO`,
/// `DS_TIPO_BEM_CANDIDATO`, `DS_BEM_CANDIDATO`, `VR_BEM_CANDIDATO`) is
/// present under the IDENTICAL name in both schemas. `#[serde(alias = ...)]`
/// on just the three renamed fields lets the `csv` crate's header-based
/// deserializer (which resolves each CSV header string through the same
/// generated field-identifier matching serde uses for map keys — proven
/// against the real downloaded 2006/2010 files, not assumed) accept EITHER
/// column name with no version dispatch needed: the extra 2006/2010-only
/// metadata columns (`DT_GERACAO`, `CD_TIPO_ELEICAO`, `SG_UE`, …) are simply
/// unmodeled and ignored, same as any other wide CSV row this adapter reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub(crate) struct BemCandidato {
    #[serde(rename = "SQ_CANDIDATO")]
    pub(crate) sq_candidato: String,
    #[serde(rename = "DT_ELEICAO")]
    pub(crate) dt_eleicao: String,
    #[serde(rename = "ANO_ELEICAO")]
    pub(crate) ano_eleicao: String,
    #[serde(rename = "NR_ORDEM_BEM_CANDIDATO", alias = "NR_ORDEM_CANDIDATO")]
    pub(crate) nr_ordem_bem_candidato: String,
    #[serde(rename = "CD_TIPO_BEM_CANDIDATO")]
    pub(crate) cd_tipo_bem_candidato: String,
    #[serde(rename = "DS_TIPO_BEM_CANDIDATO")]
    pub(crate) ds_tipo_bem_candidato: String,
    #[serde(rename = "DS_BEM_CANDIDATO")]
    pub(crate) ds_bem_candidato: String,
    #[serde(rename = "VR_BEM_CANDIDATO")]
    pub(crate) vr_bem_candidato: String,
    #[serde(rename = "DT_ULT_ATUAL_BEM_CANDIDATO", alias = "DT_ULTIMA_ATUALIZACAO")]
    pub(crate) dt_ult_atual_bem_candidato: String,
    #[serde(rename = "HH_ULT_ATUAL_BEM_CANDIDATO", alias = "HH_ULTIMA_ATUALIZACAO")]
    pub(crate) hh_ult_atual_bem_candidato: String,
}

/// One `stg_br` payload: source-faithful verbatim strings (Silver keeps the
/// source's own vocabulary, not the Gold contract — design §5.1). Every date
/// and value stays an unparsed string here; `normalize()` does the
/// `DD/MM/YYYY` and comma-decimal parsing (plan.md "Date parsing"/"Value
/// parsing").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) sq_candidato: String,
    /// Verbatim `NM_CANDIDATO` (candidate name) — PUBLIC disclosure content,
    /// not PII (AUTHORITY.md is explicit that candidate identity is the
    /// disclosure's whole point, unlike CPF/Titulo/DOB, which correctly stay
    /// gated). Always present: required for `RunnerBinding::filing_identity()`'s
    /// `filer_name` (design §5.4 roster resolution) — a real production need
    /// this regime's original conformance-only pass did not carry.
    pub(crate) nm_candidato: String,
    /// Verbatim `SG_UF` (candidate's state) — this regime's
    /// district-equivalent for roster resolution (`FilingIdentity.district`):
    /// Brazilian federal deputies/senators are elected per-state, unlike
    /// `us_house`'s single-member districts. Always present, same rationale
    /// as `nm_candidato` above.
    pub(crate) sg_uf: String,
    pub(crate) dt_eleicao_raw: String,
    pub(crate) election_year_raw: String,
    pub(crate) line_item_ordinal_raw: String,
    pub(crate) asset_type_code_raw: String,
    pub(crate) asset_type_label_raw: String,
    pub(crate) asset_description_raw: String,
    pub(crate) value_raw: String,
    pub(crate) last_updated_date_raw: String,
    pub(crate) last_updated_time_raw: String,
    pub(crate) extractor: String,
    /// Production-only politician-resolution passthrough (plan.md "Row
    /// unit": the real `normalize()`/filing-identity path needs
    /// `NR_TITULO_ELEITORAL_CANDIDATO` since `SQ_CANDIDATO` is only unique
    /// within one election cycle — AUTHORITY.md `identifiers_available`).
    /// `None` — and therefore ABSENT from the serialized payload
    /// (`skip_serializing_if`) — in conformance mode, which keeps
    /// `expected.silver.json` byte-exact per the test-designer's
    /// conservative, PII-free fixture pass (fixtures `MANIFEST.json`
    /// `pii_excluded_from_silver_flag`); `Some(..)` whenever a real Postgres
    /// pool is present (`crate::adapter`'s `parse()` gates on
    /// `ctx.pool.is_some()`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) nr_titulo_eleitoral_candidato: Option<String>,
    /// Same production-only passthrough as above; suppressed at source
    /// (sentinel `-4`) from the 2024 cycle onward (AUTHORITY.md
    /// `regime_versions`) — carried verbatim when present, never guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) nr_cpf_candidato: Option<String>,
}

/// A Silver row plus its extraction confidence. The CSV parses cleanly end
/// to end for every row (no free-text ambiguity, unlike a scanned-PDF or
/// prose source) — every row starts and stays at full confidence here; the
/// one real judgment call in this regime, `CD_TIPO_BEM_CANDIDATO ->
/// AssetClass`, is a classification decision scored at `normalize()` time
/// instead (`crate::normalize`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredRow {
    pub(crate) row: SilverRow,
    pub(crate) confidence: f32,
}

/// Parses one joined candidate document into zero-or-more scored Silver rows
/// (one per `bem_candidato` item, in source order — fixtures
/// `MANIFEST.json` `row_order_assumption_flag`: no sort step). `include_pii`
/// gates the production-only `nr_titulo_eleitoral_candidato`/
/// `nr_cpf_candidato` passthrough (see [`SilverRow`]).
///
/// # Errors
/// The bytes are outside the `consulta_cand`/`bem_candidato` join shape, the
/// candidate's `DS_CARGO` is outside this regime's scope, or a `bem_candidato`
/// item's own `SQ_CANDIDATO` disagrees with the joined candidate's (join
/// integrity) — all hard rejects (invariant 6), never guessed.
pub(crate) fn parse_document(text: &str, include_pii: bool) -> anyhow::Result<Vec<ScoredRow>> {
    let doc: SourceDoc = serde_json::from_str(text)
        .context("document outside the consulta_cand/bem_candidato join shape")?;
    anyhow::ensure!(
        IN_SCOPE_CARGOS.contains(&doc.consulta_cand.ds_cargo.as_str()),
        "DS_CARGO {:?} is outside this regime's scope (Câmara/Senado only) — hard reject",
        doc.consulta_cand.ds_cargo
    );
    doc.bem_candidato
        .into_iter()
        .map(|item| {
            anyhow::ensure!(
                item.sq_candidato == doc.consulta_cand.sq_candidato,
                "bem_candidato.SQ_CANDIDATO {:?} disagrees with consulta_cand.SQ_CANDIDATO {:?} \
                 — hard reject (join integrity)",
                item.sq_candidato,
                doc.consulta_cand.sq_candidato
            );
            Ok(ScoredRow {
                row: SilverRow {
                    sq_candidato: item.sq_candidato,
                    nm_candidato: doc.consulta_cand.nm_candidato.clone(),
                    sg_uf: doc.consulta_cand.sg_uf.clone(),
                    dt_eleicao_raw: item.dt_eleicao,
                    election_year_raw: item.ano_eleicao,
                    line_item_ordinal_raw: item.nr_ordem_bem_candidato,
                    asset_type_code_raw: item.cd_tipo_bem_candidato,
                    asset_type_label_raw: item.ds_tipo_bem_candidato,
                    asset_description_raw: item.ds_bem_candidato,
                    value_raw: item.vr_bem_candidato,
                    last_updated_date_raw: item.dt_ult_atual_bem_candidato,
                    last_updated_time_raw: item.hh_ult_atual_bem_candidato,
                    extractor: EXTRACTOR.to_owned(),
                    nr_titulo_eleitoral_candidato: include_pii
                        .then(|| doc.consulta_cand.nr_titulo_eleitoral_candidato.clone()),
                    nr_cpf_candidato: include_pii
                        .then(|| doc.consulta_cand.nr_cpf_candidato.clone()),
                },
                confidence: BASE_CONFIDENCE,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn doc_json() -> serde_json::Value {
        json!({
            "consulta_cand": {
                "SQ_CANDIDATO": "10001595344",
                "NM_CANDIDATO": "MARIA TESTE CANDIDATA",
                "SG_UF": "AC",
                "DS_CARGO": "DEPUTADO FEDERAL",
                "NR_TITULO_ELEITORAL_CANDIDATO": "[SYNTHETIC-TITULO]",
                "NR_CPF_CANDIDATO": "[SYNTHETIC-CPF]"
            },
            "bem_candidato": [
                {
                    "SQ_CANDIDATO": "10001595344",
                    "DT_ELEICAO": "02/10/2022",
                    "ANO_ELEICAO": "2022",
                    "NR_ORDEM_BEM_CANDIDATO": "1",
                    "CD_TIPO_BEM_CANDIDATO": "12",
                    "DS_TIPO_BEM_CANDIDATO": "Casa",
                    "DS_BEM_CANDIDATO": "Casa na zona rural de xapuri",
                    "VR_BEM_CANDIDATO": "10000,00",
                    "DT_ULT_ATUAL_BEM_CANDIDATO": "02/10/2022",
                    "HH_ULT_ATUAL_BEM_CANDIDATO": "23:21:28"
                }
            ]
        })
    }

    #[test]
    #[allow(clippy::float_cmp)] // bit-equality IS the contract (MANIFEST float rule)
    fn typical_row_parses_at_full_confidence_without_pii() {
        let rows = parse_document(&doc_json().to_string(), false).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].confidence, 1.0);
        assert_eq!(rows[0].row.sq_candidato, "10001595344");
        assert_eq!(rows[0].row.nm_candidato, "MARIA TESTE CANDIDATA");
        assert_eq!(rows[0].row.sg_uf, "AC");
        assert_eq!(rows[0].row.extractor, EXTRACTOR);
        assert_eq!(rows[0].row.nr_titulo_eleitoral_candidato, None);
        assert_eq!(rows[0].row.nr_cpf_candidato, None);
        let payload = serde_json::to_value(&rows[0].row).unwrap();
        assert!(
            payload.as_object().unwrap().len() == 13,
            "PII fields must be absent, not null, in conformance mode: {payload}"
        );
    }

    #[test]
    fn production_mode_threads_pii_for_politician_resolution() {
        let rows = parse_document(&doc_json().to_string(), true).unwrap();
        assert_eq!(
            rows[0].row.nr_titulo_eleitoral_candidato.as_deref(),
            Some("[SYNTHETIC-TITULO]")
        );
        assert_eq!(
            rows[0].row.nr_cpf_candidato.as_deref(),
            Some("[SYNTHETIC-CPF]")
        );
        let payload = serde_json::to_value(&rows[0].row).unwrap();
        assert_eq!(payload.as_object().unwrap().len(), 15);
    }

    #[test]
    fn zero_asset_candidate_yields_zero_rows_not_an_error() {
        let mut doc = doc_json();
        doc["bem_candidato"] = json!([]);
        let rows = parse_document(&doc.to_string(), false).unwrap();
        assert!(
            rows.is_empty(),
            "plan.md edge case 1: legitimate, not a fail"
        );
    }

    #[test]
    fn out_of_scope_cargo_is_a_hard_reject() {
        let mut doc = doc_json();
        doc["consulta_cand"]["DS_CARGO"] = json!("GOVERNADOR");
        assert!(parse_document(&doc.to_string(), false).is_err());
    }

    #[test]
    fn join_integrity_mismatch_is_a_hard_reject() {
        let mut doc = doc_json();
        doc["bem_candidato"][0]["SQ_CANDIDATO"] = json!("99999999999");
        assert!(parse_document(&doc.to_string(), false).is_err());
    }

    #[test]
    fn row_order_matches_source_array_order() {
        let mut doc = doc_json();
        doc["bem_candidato"] = json!([
            {
                "SQ_CANDIDATO": "10001595344", "DT_ELEICAO": "02/10/2022",
                "ANO_ELEICAO": "2022", "NR_ORDEM_BEM_CANDIDATO": "3",
                "CD_TIPO_BEM_CANDIDATO": "21", "DS_TIPO_BEM_CANDIDATO": "Veículo",
                "DS_BEM_CANDIDATO": "Moto", "VR_BEM_CANDIDATO": "15000,00",
                "DT_ULT_ATUAL_BEM_CANDIDATO": "02/10/2022", "HH_ULT_ATUAL_BEM_CANDIDATO": "23:21:28"
            },
            {
                "SQ_CANDIDATO": "10001595344", "DT_ELEICAO": "02/10/2022",
                "ANO_ELEICAO": "2022", "NR_ORDEM_BEM_CANDIDATO": "1",
                "CD_TIPO_BEM_CANDIDATO": "12", "DS_TIPO_BEM_CANDIDATO": "Casa",
                "DS_BEM_CANDIDATO": "Casa", "VR_BEM_CANDIDATO": "10000,00",
                "DT_ULT_ATUAL_BEM_CANDIDATO": "02/10/2022", "HH_ULT_ATUAL_BEM_CANDIDATO": "23:21:28"
            }
        ]);
        let rows = parse_document(&doc.to_string(), false).unwrap();
        assert_eq!(
            rows[0].row.line_item_ordinal_raw, "3",
            "source order, not ordinal-sorted"
        );
        assert_eq!(rows[1].row.line_item_ordinal_raw, "1");
    }
}
