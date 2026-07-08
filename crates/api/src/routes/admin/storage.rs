//! `GET /v1/admin/storage` — section E, storage & tiers: Bronze doc counts
//! by mime and storage scheme (E1; no byte column exists → count-only,
//! stated), 30d Gold/filing growth (E2), Postgres size + top tables +
//! live/dead tuples from `pg_catalog` (E3).
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
