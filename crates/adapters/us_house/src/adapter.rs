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

/// The US House PTR adapter (`FilingType == "P"` only).
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
    /// Discover one archive year's PTR filings (backfill, design §5.6 — the same
    /// pipeline pointed at the Clerk's historical `{YYYY}FD.zip` indexes back to
    /// the 2012 STOCK Act era). The live [`discover`](Self::discover) is this for
    /// the current year. Validators are cached PER YEAR (see the struct field):
    /// each year is fetched once unconditionally, so a backfill sweep never
    /// false-304s, while a live re-poll of the same year still sends conditional
    /// headers. Early archive years legitimately hold zero `FilingType == "P"`
    /// rows (PTR e-filing post-dates the 2012 STOCK Act; verified empty for
    /// 2012 — goal 080) — that is a valid empty result, not a failure. If a
    /// historical index's shape ever diverges from the current `Member` layout,
    /// the XML parse fails HERE for that year and the caller fails it closed,
    /// continuing the range.
    ///
    /// # Errors
    /// Index GET transport failure, a non-success (non-304) status, or an
    /// unparseable index archive.
    pub async fn discover_year(&self, year: i32, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
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
            return Ok(Vec::new()); // index unchanged — nothing new
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

        // Filter FilingType == "P"; publish-time dedup by (regime, external_id)
        // also captures amended PTRs, which arrive as NEW DocIDs (§2.4).
        Ok(index::parse_index_zip(&bytes)?
            .into_iter()
            .filter(|member| {
                member.filing_type == "P" && !member.doc_id.is_empty() && !member.year.is_empty()
            })
            .map(|member| FilingRef {
                url: index::ptr_pdf_url(&member.year, &member.doc_id),
                external_id: member.doc_id,
            })
            .collect())
    }
}
