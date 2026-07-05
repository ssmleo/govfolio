//! Document-level LLM-vision extraction seam (design §5.3, regime doc §6),
//! reusing the goal-021 machinery in `pipeline::extraction`. This regime is
//! LLM-vision-FIRST: the Bronze document is a scanned, often-handwritten,
//! multi-column paper form (§3.9), so EVERY document routes here — there is no
//! deterministic v1 path.
//!
//! Fail-closed order of business (invariant 6 at every exit):
//! 1. committed file cache (`fixtures/<case>/extraction.cache.json`) — this is
//!    what keeps conformance and e2e OFFLINE; a hit never calls the API;
//! 2. `extraction_cache` Postgres tier (pool-backed runs);
//! 3. live vision extraction — requires the Azure-WAF browser-engine fetch seam
//!    (§2.3) AND schema-constrained vision transcription; both are recorded
//!    follow-ups. Until they land, a cache miss freezes the document behind a
//!    `needs_llm_extraction` error → review path, never silent rows.

use anyhow::Context as _;
use async_trait::async_trait;

use pipeline::adapter::{RawDocRef, RunCtx, StagingRow};
use pipeline::extraction::{CacheKey, FileCache, Models, pg_get};

use crate::normalize::SilverRow;

/// Extractor tag recorded on every Silver row (regime doc §4; fixtures
/// MANIFEST `extraction_cache_priming`). One tag — there is no text fast path
/// in v1 (§6 step 2).
pub(crate) const EXTRACTOR_LLM: &str = "australia_register/llm@1";

/// Wrapper-confidence floor: a cached row below this fails closed (review
/// path, never silent Gold). `text_layer` rows carry `1.0`, `scanned_vision`
/// rows `0.98` (MANIFEST `confidence_literals`) — both clear this floor.
const MIN_ACCEPT_CONFIDENCE: f32 = 0.9;

/// Extraction seam for the vision-first regime.
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Extracts Silver rows from one Bronze document.
    ///
    /// # Errors
    /// Extraction failure — the document freezes behind the error (review
    /// path); rows are never guessed.
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>>;
}

/// The goal-021 LLM extractor: cache tiers + (deferred) live vision fallback.
#[derive(Debug, Clone)]
pub struct LlmExtractor {
    file_cache: FileCache,
}

impl Default for LlmExtractor {
    fn default() -> Self {
        // Committed conformance/e2e cache tier; on hosts without a source tree
        // the directory is simply absent (= empty cache, Postgres tier over).
        Self {
            file_cache: FileCache::open(pipeline::conformance::fixtures_dir("australia_register")),
        }
    }
}

#[async_trait]
impl Extractor for LlmExtractor {
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let models = Models::from_env();
        let key = CacheKey::new(&doc.sha256, EXTRACTOR_LLM, &models.primary);
        // Tier 1: committed file cache (conformance/e2e — offline, no API).
        if let Some(rows) = self.file_cache.get(&key)? {
            return validated(rows, &doc.sha256);
        }
        // Tier 2: extraction_cache (pool-backed runs).
        if let Some(pool) = &ctx.pool
            && let Some(rows) = pg_get(pool, &key).await?
        {
            return validated(rows, &doc.sha256);
        }
        // Tier 3: live vision extraction — blocked on the browser-engine fetch
        // seam + schema-constrained vision transcription (follow-ups, §2.3/§6).
        anyhow::bail!(
            "needs_llm_extraction: no cached vision extraction for document {} \
             (key: {EXTRACTOR_LLM} / {}); the live path requires the Azure-WAF \
             browser-engine fetch seam AND schema-constrained vision transcription \
             (recorded follow-ups) — freeze + review_task (invariant 6)",
            doc.sha256,
            models.primary
        )
    }
}

/// Fail-closed validation of cached rows: every payload must be a real
/// `SilverRow` carrying the llm@1 tag, and the wrapper confidence must sit in
/// `[0.9, 1.0]` — below-threshold or schema-invalid entries never publish.
fn validated(rows: Vec<StagingRow>, sha256: &str) -> anyhow::Result<Vec<StagingRow>> {
    anyhow::ensure!(
        !rows.is_empty(),
        "cached extraction for {sha256} is empty — fail closed (invariant 6)"
    );
    for (index, staged) in rows.iter().enumerate() {
        let row: SilverRow = serde_json::from_value(staged.payload.clone()).with_context(|| {
            format!("cached extraction row {index} for {sha256} is not a SilverRow — fail closed")
        })?;
        anyhow::ensure!(
            row.extractor == EXTRACTOR_LLM,
            "cached extraction row {index} for {sha256} carries tag {:?}, want {EXTRACTOR_LLM:?} \
             — fail closed",
            row.extractor
        );
        anyhow::ensure!(
            (MIN_ACCEPT_CONFIDENCE..=1.0).contains(&staged.confidence),
            "cached extraction row {index} for {sha256} has confidence {} below the {} floor — \
             fail closed (review path, never silent Gold)",
            staged.confidence,
            MIN_ACCEPT_CONFIDENCE
        );
    }
    Ok(rows)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)] // exact f32 bit-image IS the contract
mod tests {
    use pipeline::adapter::{BronzeStore, Clock, PolitenessCfg};
    use serde_json::json;

    use super::*;

    fn scanned_fixture_ctx(case: &str, tag: &str) -> (RunCtx, RawDocRef) {
        let root =
            std::env::temp_dir().join(format!("govfolio-au-seam-{tag}-{}", std::process::id()));
        let ctx = RunCtx::new(
            BronzeStore::open(root).unwrap(),
            None,
            Clock::System,
            &PolitenessCfg::new(std::time::Duration::ZERO, "test@govfolio.io"),
        )
        .unwrap();
        let bytes = std::fs::read(
            pipeline::conformance::fixtures_dir("australia_register")
                .join(case)
                .join("input.pdf"),
        )
        .unwrap();
        let doc = ctx.bronze.put(&bytes).unwrap();
        (ctx, doc)
    }

    /// Cache hit = no API call (design §5.3): no transport is constructed —
    /// this runs with no key and no network and must return the primed
    /// ground-truth rows verbatim.
    #[tokio::test]
    async fn cache_hit_extracts_offline_without_any_api_call() {
        let (ctx, doc) = scanned_fixture_ctx("scanned_formA", "hit");
        let rows = LlmExtractor::default().extract(&doc, &ctx).await.unwrap();
        assert_eq!(rows.len(), 16, "Katter Form A = 16 interest rows");
        assert_eq!(rows[0].payload["document_filename"], json!("Katter_48P"));
        assert_eq!(rows[0].payload["extractor"], json!(EXTRACTOR_LLM));
        // scanned_vision wrapper confidence: the exact f64 image of 0.98f32.
        assert_eq!(
            serde_json::to_value(&rows[0]).unwrap()["confidence"],
            json!(0.980_000_019_073_486_3_f64)
        );
    }

    /// A cache miss with no live seam freezes the document behind the review
    /// path — never silent rows (invariant 6).
    #[tokio::test]
    async fn cache_miss_fails_closed() {
        let (ctx, _) = scanned_fixture_ctx("scanned_formA", "miss");
        let unknown = ctx.bronze.put(b"some other document").unwrap();
        let err = LlmExtractor::default()
            .extract(&unknown, &ctx)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs_llm_extraction"), "{err}");
        assert!(err.contains("review_task"), "{err}");
    }

    #[test]
    fn cached_rows_below_confidence_floor_or_foreign_tag_fail_closed() {
        let good = vec![StagingRow {
            payload: json!({
                "document_filename": "Katter_48P",
                "parliament_no": 48,
                "row_ordinal": 1,
                "page_ordinal": 2,
                "section_kind": "statement",
                "category_number": 1,
                "category_name_raw": "Shareholdings in public and private companies",
                "owner_band_raw": "Self",
                "addition_deletion_raw": null,
                "electoral_division_raw": "Kennedy",
                "state_raw": "QLD",
                "entry_text_raw": "A number of AMP shares",
                "entry_fields_raw": {},
                "date_raw": null,
                "parliament_stamp_raw": null,
                "extractor": EXTRACTOR_LLM
            }),
            confidence: 0.98,
        }];
        assert!(validated(good.clone(), "sha").is_ok());

        let mut low = good.clone();
        low[0].confidence = 0.5;
        assert!(validated(low, "sha").is_err());

        let mut foreign = good.clone();
        foreign[0].payload["extractor"] = json!("australia_register/text@1");
        assert!(validated(foreign, "sha").is_err());

        let mut garbage = good;
        garbage[0].payload = json!({"not": "a silver row"});
        assert!(validated(garbage, "sha").is_err());
        assert!(validated(Vec::new(), "sha").is_err(), "empty cache entry");
    }
}
