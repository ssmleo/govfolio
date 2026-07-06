//! [`JurisdictionAdapter`] implementation for the `us_house` PTR regime:
//! discover (index zip, conditional GET), fetch (Bronze by sha256), parse
//! (text-layer state machine + LLM seam), normalize (Silver → Gold).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::Datelike as _;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::extractor::{Extractor as _, LlmExtractor};
use crate::{index, normalize, parse};

/// Mean row confidence below this routes the whole document to the LLM seam
/// (regime doc §6.3b).
const LLM_SEAM_CONFIDENCE_FLOOR: f64 = 0.90;

/// Cached index validators for conditional GETs (regime doc §2.4).
#[derive(Debug, Default, Clone)]
struct IndexValidators {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// The US House PTR adapter (`FilingType == "P"`, 2015+; `DisclosureType ==
/// "PTR"` + `FilingType` `O`/`A`, pre-2015 — see [`is_ptr`]).
#[derive(Debug, Default)]
pub struct UsHouseAdapter {
    /// Conditional-GET validators keyed BY ARCHIVE YEAR (regime doc §2.4). Each
    /// `{YYYY}FD.zip` is a distinct resource, so validators must NOT be shared
    /// across years: `If-Modified-Since` is a date comparison the server honors
    /// per-resource, and a later year re-poll carrying an earlier year's
    /// `Last-Modified` would wrongly 304 (goal 080 — the whole backfill sweep
    /// otherwise reads empty). One entry per year: the current year is
    /// re-polled conditionally in the live loop; each backfill year is fetched
    /// once, unconditionally, then cached.
    index_validators: Mutex<HashMap<i32, IndexValidators>>,
    /// LLM seam for documents the text path cannot handle (goal 021).
    extractor: LlmExtractor,
}

#[async_trait]
impl JurisdictionAdapter for UsHouseAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef { code: "us_house" }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Regime doc §2.4: identified UA with reachable contact, concurrency 1
        // (cfg default), >= 2 s between requests to the host.
        PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com")
    }

    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        // Live discovery is backfill pointed at the current year.
        self.discover_year(ctx.clock.now().year(), ctx).await
    }

    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let response = ctx.http.get(&r.url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "PDF GET {} -> {}",
            r.url,
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading PDF body of {}", r.url))?;
        ctx.bronze.put(&bytes) // immutable, sha256-addressed (invariant 2)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let Ok(text) = pdf_extract::extract_text_from_mem(&bytes) else {
            // Unreadable-by-text-path document (scanned/paper): LLM seam
            // (§6.3c) — the seam itself fails closed when it cannot extract.
            return self.extractor.extract(d, ctx).await;
        };
        if !text.contains("Filing ID #") {
            // No usable text layer (paper/scanned filing): LLM seam (§6.3a/c).
            return self.extractor.extract(d, ctx).await;
        }
        let doc = parse::parse_document(&text)
            .with_context(|| format!("parsing PTR text layer of {}", d.sha256))?;
        if doc.rows.is_empty() || doc.doc_id.len() == 7 {
            // Zero rows / paper DocID shape: LLM seam (§6.3a/c).
            return self.extractor.extract(d, ctx).await;
        }
        let row_count = u32::try_from(doc.rows.len()).context("row count overflow")?;
        let mean_confidence = doc
            .rows
            .iter()
            .map(|scored| f64::from(scored.confidence))
            .sum::<f64>()
            / f64::from(row_count);
        if mean_confidence < LLM_SEAM_CONFIDENCE_FLOOR {
            return self.extractor.extract(d, ctx).await; // §6.3b
        }
        doc.rows
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

impl UsHouseAdapter {
    /// Fetches + unzips one archive year's index to its raw `*FD.xml` text
    /// (conditional GET, validators cached per year — regime doc §2.4).
    /// Shared by [`discover_year`](Self::discover_year) (filing discovery)
    /// and historical roster seeding (`seed::seed_historical_rosters`, goal
    /// 081 Task 1) so the SAME year's archive is fetched exactly once, never
    /// twice, across both uses (invariant 10).
    ///
    /// Returns `Ok(None)` on a 304 (index unchanged since the last poll for
    /// this year); `Ok(Some(xml))` otherwise.
    ///
    /// # Errors
    /// Index GET transport failure, a non-success (non-304) status, or an
    /// unparseable index zip.
    pub(crate) async fn fetch_index_xml(
        &self,
        year: i32,
        ctx: &RunCtx,
    ) -> anyhow::Result<Option<String>> {
        let url = index::index_zip_url(year);
        let cached = self
            .index_validators
            .lock()
            .map_err(|_| anyhow::anyhow!("index validator lock poisoned"))?
            .get(&year)
            .cloned()
            .unwrap_or_default();
        let response = ctx
            .http
            .get_conditional(
                &url,
                cached.etag.as_deref(),
                cached.last_modified.as_deref(),
            )
            .await?;
        if response.status().as_u16() == 304 {
            return Ok(None); // index unchanged — nothing new
        }
        anyhow::ensure!(
            response.status().is_success(),
            "index GET {url} -> {}",
            response.status()
        );
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let fresh = IndexValidators {
            etag: header("etag"),
            last_modified: header("last-modified"),
        };
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading index body of {url}"))?;
        self.index_validators
            .lock()
            .map_err(|_| anyhow::anyhow!("index validator lock poisoned"))?
            .insert(year, fresh);
        Ok(Some(index::unzip_index_xml(&bytes)?))
    }

    /// Discover one archive year's PTR filings (backfill, design §5.6 — the same
    /// pipeline pointed at the Clerk's historical `{YYYY}FD.zip` indexes back to
    /// the 2012 STOCK Act era). The live [`discover`](Self::discover) is this for
    /// the current year. Validators are cached PER YEAR (see the struct field):
    /// each year is fetched once unconditionally, so a backfill sweep never
    /// false-304s, while a live re-poll of the same year still sends conditional
    /// headers. The index schema forks before ~2015 (goal 081 Task 4.5 —
    /// AUTHORITY.md's `open_questions`/Quirks log): [`is_ptr`] recognizes both
    /// eras, so pre-2015 archive years are no longer silently read as empty.
    /// If a historical index's shape ever diverges from the current `Member`
    /// layout, the XML parse fails HERE for that year and the caller fails it
    /// closed, continuing the range.
    ///
    /// # Errors
    /// Index GET transport failure, a non-success (non-304) status, or an
    /// unparseable index archive.
    pub async fn discover_year(&self, year: i32, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let Some(xml) = self.fetch_index_xml(year, ctx).await? else {
            return Ok(Vec::new()); // index unchanged — nothing new
        };
        // is_ptr() recognizes both schema eras; publish-time dedup by
        // (regime, external_id) also captures amended PTRs, which arrive as
        // NEW DocIDs (§2.4).
        Ok(index::parse_index_xml(&xml)?
            .into_iter()
            .filter(|member| is_ptr(member) && !member.doc_id.is_empty() && !member.year.is_empty())
            .map(|member| FilingRef {
                url: index::ptr_pdf_url(&member.year, &member.doc_id),
                external_id: member.doc_id,
            })
            .collect())
    }
}

/// Whether an index `Member` row is a PTR filing, under either era the
/// Clerk's index schema uses (goal 081 Task 4.5 —
/// `docs/regimes/us_house/AUTHORITY.md`'s schema-fork finding): 2015+ tags
/// `FilingType == "P"`; pre-2015 has no `P` code at all and instead tags
/// `DisclosureType == "PTR"` under `FilingType` `O` (original) or `A`
/// (amended). Filtering on `FilingType == "P"` alone silently drops every
/// pre-2015 PTR (not fail-closed — the bug this fixes).
fn is_ptr(member: &index::IndexMember) -> bool {
    member.filing_type == "P"
        || (member.disclosure_type == "PTR"
            && (member.filing_type == "O" || member.filing_type == "A"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Real 2012FD.zip records (docs/regimes/us_house/AUTHORITY.md
    // historical_depth; evidence archive
    // 3ef175309c99f036fe053814fb2a8939e5adb7e3cf33ab00cfd1c11667036251) mixing
    // pre-2015 PTRs (`DisclosureType==PTR` under `FilingType` `O`/`A`) with a
    // non-PTR annual-FD row, plus one 2015+-style `FilingType==P` row (no
    // `DisclosureType`) proving both schema eras are recognized side by side.
    const MIXED_ERA_SLICE: &str = "<FinancialDisclosure>\
          <Member><Prefix /><Last>ABERNATHY</Last><First>SARAH L.</First><Suffix />\
            <FilingType>O</FilingType><StateDst>BU00</StateDst><Year>2012</Year>\
            <FilingDate>8/1/2012</FilingDate><DocID>2000077</DocID>\
            <DisclosureType>PTR</DisclosureType></Member>\
          <Member><Prefix /><Last>ANDRES</Last><First>GARY J.</First><Suffix />\
            <FilingType>A</FilingType><StateDst>CM00</StateDst><Year>2012</Year>\
            <FilingDate>11/29/2012</FilingDate><DocID>2000655</DocID>\
            <DisclosureType>PTR</DisclosureType></Member>\
          <Member><Prefix /><Last>ABBOTT</Last><First>JESSICA A.</First><Suffix />\
            <FilingType>O</FilingType><StateDst>AO00</StateDst><Year>2012</Year>\
            <FilingDate>5/14/2012</FilingDate><DocID>9101116</DocID>\
            <DisclosureType>FD</DisclosureType></Member>\
          <Member><Prefix>Hon.</Prefix><Last>Begich</Last><First>Nicholas</First>\
            <FilingType>P</FilingType><StateDst>AK00</StateDst><Year>2026</Year>\
            <FilingDate>6/12/2026</FilingDate><DocID>20020055</DocID></Member>\
        </FinancialDisclosure>";

    #[test]
    fn pre_2015_and_current_schema_ptrs_are_both_discovered() {
        let members = index::parse_index_xml(MIXED_ERA_SLICE).unwrap();
        // The old, pre-fix filter (`FilingType == "P"` only) finds ONLY the
        // 2015+-style row — reproducing goal 080's dry run reporting 0 PTRs
        // for 2012/2013 (the exact bug goal 081 Task 4.5 fixes).
        let old_filter_count = members.iter().filter(|m| m.filing_type == "P").count();
        assert_eq!(
            old_filter_count, 1,
            "old FilingType=='P'-only filter silently misses both pre-2015 PTRs"
        );
        let discovered = members.iter().filter(|m| is_ptr(m)).count();
        assert_eq!(
            discovered, 3,
            "both pre-2015 PTRs (FilingType O/A + DisclosureType PTR) and the \
             2015+-style FilingType=='P' PTR must be discovered; the non-PTR FD \
             row must not be"
        );
    }

    #[test]
    fn non_ptr_filing_type_o_without_disclosure_type_ptr_is_excluded() {
        let member = index::parse_index_xml(
            "<FinancialDisclosure><Member><Last>X</Last><FilingType>O</FilingType>\
             <StateDst>AO00</StateDst><Year>2012</Year><DocID>1</DocID>\
             <DisclosureType>FD</DisclosureType></Member></FinancialDisclosure>",
        )
        .unwrap();
        assert!(
            !is_ptr(&member[0]),
            "FilingType O with DisclosureType FD is not a PTR"
        );
    }
}
