//! Document-level LLM-extraction seam (design §5.3, regime doc §6.3).
//! Routes here: (a) paper filings (`/search/view/paper/` GIF scans, §3.8),
//! (b) documents the deterministic parse rejects per §3.7.
//!
//! v1 STUB: freeze the document behind a `needs_llm_extraction` error —
//! review path, never silent rows (invariant 6). The real LLM leg lands
//! behind this same trait with the reserved extractor tag
//! `us_senate_ptr/llm@1` (`us_house` goal-021 pattern: cache tiers + forced
//! tool use + second-model cross-check).

use async_trait::async_trait;

use pipeline::adapter::{RawDocRef, RunCtx, StagingRow};

/// Extraction seam for documents the deterministic HTML path cannot handle.
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Extracts Silver rows from one Bronze document.
    ///
    /// # Errors
    /// Extraction failure — the document freezes behind the error (review
    /// path); rows are never guessed.
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>>;
}

/// The v1 fail-closed stub: every routed document freezes.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubExtractor;

#[async_trait]
impl Extractor for StubExtractor {
    async fn extract(&self, doc: &RawDocRef, _ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        anyhow::bail!(
            "needs_llm_extraction: document {} routed to the LLM seam (paper filing or \
             deterministic reject, regime doc §6.3) — v1 stub freezes + review_task \
             (invariant 6)",
            doc.sha256
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use pipeline::adapter::{BronzeStore, Clock, PolitenessCfg, RunCtx};

    use super::*;

    #[tokio::test]
    async fn stub_freezes_every_document() {
        let root = std::env::temp_dir().join(format!(
            "govfolio-us-senate-extractor-test-{}",
            std::process::id()
        ));
        let ctx = RunCtx::new(
            BronzeStore::open(root).unwrap(),
            None,
            Clock::System,
            &PolitenessCfg::new(std::time::Duration::ZERO, "test@govfolio.io"),
        )
        .unwrap();
        let doc = RawDocRef {
            sha256: "ab".repeat(32),
        };
        let err = StubExtractor
            .extract(&doc, &ctx)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs_llm_extraction"), "{err}");
        assert!(err.contains(&doc.sha256), "{err}");
    }
}
