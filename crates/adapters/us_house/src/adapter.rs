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
        let Ok(text) = extract_text_catching_panics(&bytes) else {
            // Unreadable-by-text-path document (scanned/paper), OR
            // `pdf_extract` panicked internally and `extract_text_catching_panics`
            // converted it into this same `Err` (goal 081 Task 4.7): LLM seam
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

/// Runs `pdf_extract::extract_text_from_mem`, converting an internal panic
/// into an ordinary `Err` instead of letting it crash the process (goal 081
/// Task 4.7). A real `backfill-real` run crashed outright on a real 2020-era
/// document: `pdf-extract-0.12.0/src/lib.rs:950` does
/// `String::from_utf16(&be).unwrap()` while decoding a font's embedded
/// `ToUnicode` `CMap`, which panics on malformed UTF-16 (e.g. an unpaired
/// surrogate) rather than returning an `Err` — a panic never produces an
/// `Err` for the caller's existing `let Ok(text) = ... else { ... }` to
/// match on, so it unwinds straight past that fail-closed handling. Wrapping
/// the call in `catch_unwind` lets one poison-pill document fail closed like
/// any other unparseable PDF, instead of taking down the whole run.
///
/// The default panic hook is swapped out for the duration of the call so
/// this expected, handled failure mode doesn't spam stderr with a full
/// panic backtrace on every occurrence; the previous hook is always
/// restored immediately after.
fn extract_text_catching_panics(bytes: &[u8]) -> anyhow::Result<String> {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes));
    std::panic::set_hook(previous_hook);
    let Ok(extracted) = result else {
        eprintln!(
            "pdf_extract::extract_text_from_mem panicked internally (caught) — \
             treating this document as an ordinary extraction failure"
        );
        anyhow::bail!("pdf_extract::extract_text_from_mem panicked internally (caught)");
    };
    extracted.map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fmt::Write as _;

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

    /// Builds a minimal, syntactically-valid PDF (hand-assembled, byte
    /// offsets computed here rather than hand-counted) whose one font's
    /// embedded `ToUnicode` `CMap` maps a character code to `<D800D800>` — two
    /// consecutive UTF-16 high-surrogate code units, which is not valid
    /// UTF-16 (a high surrogate must be followed by a low surrogate, not
    /// another high surrogate). This is fed straight into
    /// `pdf_extract-0.12.0`'s `get_unicode_map` (upstream `src/lib.rs:950`),
    /// which does `String::from_utf16(&be).unwrap()` — reproducing, from a
    /// self-contained fixture, the exact panic class
    /// (`FromUtf16Error(())`) a real 2020-era production document triggered
    /// (goal 081 Task 4.7). The font is `Tf`-referenced from the one page's
    /// content stream, since `pdf_extract` only constructs (and so only
    /// reads the `ToUnicode` `CMap` of) a font when a `Tf` operator selects it.
    fn malformed_utf16_cmap_pdf() -> Vec<u8> {
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_owned(),
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_owned(),
            "3 0 obj\n<< /Type /Page /Parent 2 0 R \
             /Resources << /Font << /F1 4 0 R >> >> \
             /MediaBox [0 0 200 200] /Contents 6 0 R >>\nendobj\n"
                .to_owned(),
            "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
             /ToUnicode 5 0 R >>\nendobj\n"
                .to_owned(),
            {
                let stream = "1 beginbfchar\n<41> <D800D800>\nendbfchar\n";
                format!(
                    "5 0 obj\n<< /Length {} >>\nstream\n{stream}endstream\nendobj\n",
                    stream.len()
                )
            },
            {
                let stream = "BT /F1 12 Tf ET\n";
                format!(
                    "6 0 obj\n<< /Length {} >>\nstream\n{stream}endstream\nendobj\n",
                    stream.len()
                )
            },
        ];

        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for object in &objects {
            offsets.push(pdf.len());
            pdf.push_str(object);
        }

        let xref_start = pdf.len();
        let entry_count = offsets.len() + 1; // + the free-list head entry
        writeln!(pdf, "xref\n0 {entry_count}").unwrap();
        pdf.push_str("0000000000 65535 f \n");
        for offset in &offsets {
            writeln!(pdf, "{offset:010} 00000 n ").unwrap();
        }
        write!(
            pdf,
            "trailer\n<< /Size {entry_count} /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF"
        )
        .unwrap();

        pdf.into_bytes()
    }

    #[test]
    fn malformed_utf16_cmap_pdf_reproduces_the_real_pdf_extract_panic() {
        // Proves the fixture genuinely reproduces pdf-extract's own bug
        // (not just that our fixture is malformed for some other, unrelated
        // reason): calling the crate's function DIRECTLY (bypassing our
        // catch_unwind wrapper) panics, matching the real crash goal 081
        // Task 4.7 documents (`pdf-extract-0.12.0/src/lib.rs:950`,
        // `Result::unwrap()` on `FromUtf16Error(())`).
        let bytes = malformed_utf16_cmap_pdf();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // this is the EXPECTED panic; don't print it
        let result = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes));
        std::panic::set_hook(previous_hook);
        assert!(
            result.is_err(),
            "fixture must panic when called through pdf_extract directly, proving it \
             reproduces the real upstream bug rather than merely being malformed"
        );
    }

    #[test]
    fn extract_text_catching_panics_converts_the_real_panic_into_an_err() {
        // The actual fix under test: going through
        // `extract_text_catching_panics` instead of calling `pdf_extract`
        // directly must turn that SAME panic into an ordinary `Err`, not
        // crash the test process (goal 081 Task 4.7's acceptance).
        let bytes = malformed_utf16_cmap_pdf();
        let outcome = extract_text_catching_panics(&bytes);
        assert!(
            outcome.is_err(),
            "a panicking pdf_extract call must be caught and surfaced as Err, not crash"
        );
    }

    #[test]
    fn extract_text_catching_panics_still_returns_ok_for_a_normal_document() {
        // Guards against a trivial "always Err" implementation: a PDF with
        // no malformed CMap must still extract normally through the same
        // wrapper.
        let mut bytes = malformed_utf16_cmap_pdf();
        // Replace the poison-pill surrogate pair with an ordinary character
        // mapping (same byte length, so no offsets need recomputing).
        let patched = String::from_utf8(bytes.clone())
            .unwrap()
            .replace("<D800D800>", "<00410041>");
        bytes = patched.into_bytes();
        let outcome = extract_text_catching_panics(&bytes);
        assert!(
            outcome.is_ok(),
            "a document with a well-formed ToUnicode CMap must still extract, got {outcome:?}"
        );
    }
}
