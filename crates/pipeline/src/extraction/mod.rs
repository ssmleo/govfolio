//! Schema-constrained LLM document extraction (design §5.3, goal 021).
//!
//! Layered fail-closed machinery shared by every adapter's LLM seam:
//!
//! 1. **Cache by sha** ([`cache`]): lookup key `(document_sha256,
//!    extractor_tag, model_id)` — re-extraction happens only on a version
//!    bump; pay per document VERSION, once (design §5.3). Two tiers: a
//!    committed file cache for conformance/e2e (offline by construction) and
//!    the `extraction_cache` Postgres table for production runs.
//! 2. **Anthropic Messages API** ([`anthropic`]): minimal reqwest client — no
//!    SDK dependency — sending the raw document as a base64 PDF block with
//!    FORCED TOOL USE (`tool_choice: {type: "tool"}`); the tool
//!    `input_schema` IS the adapter's silver-row JSON Schema, and the tool
//!    output is re-validated against that schema locally (schema-invalid →
//!    fail closed, never silent Gold).
//! 3. **Cross-check on impact** (design §5.3): high-impact documents (the
//!    adapter supplies the predicate; watchlist stub below) are re-extracted
//!    by a SECOND, distinct model and compared field by field — any mismatch
//!    is a [`CrossCheckMismatch`] (freeze + `review_task`), agreement proceeds.
//!
//! The API key is read from `ANTHROPIC_API_KEY` at transport construction and
//! is never logged, echoed, or serialized ([`anthropic::HttpTransport`]'s
//! `Debug` impl redacts it).

pub mod anthropic;
pub mod cache;

pub use anthropic::{
    CrossCheckMismatch, DocumentToolSpec, HttpTransport, LlmDocumentExtractor, Models, Transport,
    build_request,
};
pub use cache::{
    CacheKey, CachedExtraction, FileCache, pg_get, pg_put, prime_from_expected_silver,
};

/// Politicians whose filings always take the second-model cross-check
/// (design §5.3 "watchlist"). Stubbed empty until the watchlist product
/// surface exists (goal 021 scope); adapters match on the as-filed name.
pub const WATCHLIST_POLITICIANS: &[&str] = &[];
