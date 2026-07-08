//! `GET /v1/admin/overview` — the status strip: every queue depth (outbox
//! undispatched, review open, drift open, sample pending, delivery DLQ,
//! unbilled, `pipeline_run` running/failed), 24h run counts, frozen regimes.
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
