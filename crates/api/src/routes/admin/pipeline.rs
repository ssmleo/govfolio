//! `GET /v1/admin/pipeline` — section C, pipeline health: adapter/freeze
//! board (C1), per-adapter stage funnel from `pipeline_run` + `PublishStats`
//! jsonb (C2), drift incidents by kind (C3), last 25 failed runs with error
//! text (C4), conformance run-locally note (C5, no fake status), supersede
//! activity (C6).
//!
//! Handlers land in the P3 fill-in pass; see `super` for the shared pattern.
