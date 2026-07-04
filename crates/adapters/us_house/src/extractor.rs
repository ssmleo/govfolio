//! Document-level LLM-extraction seam (design §5.3, regime doc §6.3).
//! v1 ships the trait plus a fail-closed stub; goal 021 wires the real
//! extractor (and the second-model cross-check rides the same seam).

use async_trait::async_trait;

use pipeline::adapter::{RawDocRef, RunCtx, StagingRow};

/// Extraction seam for documents the deterministic text-layer path cannot
/// handle: zero rows, mean row confidence < 0.90, or paper filings.
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Extracts Silver rows from one Bronze document.
    ///
    /// # Errors
    /// Extraction failure; the v1 stub always fails closed.
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>>;
}

/// v1 stub: freezes the document behind a `needs_llm_extraction` error so a
/// `review_task` is opened downstream (invariant 6) — no silent rows.
#[derive(Debug, Default, Clone, Copy)]
pub struct NeedsLlmExtraction;

#[async_trait]
impl Extractor for NeedsLlmExtraction {
    async fn extract(&self, doc: &RawDocRef, _ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        anyhow::bail!(
            "needs_llm_extraction: document {} requires the LLM extraction seam \
             (goal 021 wires it); freeze + review_task (invariant 6)",
            doc.sha256
        )
    }
}
