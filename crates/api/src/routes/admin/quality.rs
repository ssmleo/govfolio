//! `GET /v1/admin/quality?sweep=br` — section D, data quality & review ops:
//! review-queue analytics (D1), unmatched-entity counts (D2), opt-in br CPF
//! collision sweep reusing the check-bin's SELECT verbatim (D3), idempotency
//! evidence from `backfill_run` (D4), raw retention spot check (D5),
//! `sample_audit` precision estimates (D6).
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
