//! `GET /v1/admin/serving` — section F, serving & product: API usage (F1),
//! alert latency percentiles with backfill-suppressed rows excluded and
//! counted separately (F2), delivery health + DLQ (F3). Per-request latency
//! is not recorded anywhere — the page states that, no fake numbers.
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
