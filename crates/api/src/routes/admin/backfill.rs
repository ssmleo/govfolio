//! `GET /v1/admin/backfill` — section B, backfill & ingestion: `backfill_run`
//! rows (B1; pre-migration history is log-only and the page says so),
//! historical completion vs declared targets (B2), per-source freshness +
//! filing lag percentiles (B3), politeness proxy (B4), queue depths (B5).
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
