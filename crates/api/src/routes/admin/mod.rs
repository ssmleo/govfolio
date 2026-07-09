//! Admin observability endpoints (admin dashboard plan, sections A–H): nine
//! composite `GET /v1/admin/*` doors, one per dashboard page, each answering
//! in a single round trip.
//!
//! READ-ONLY BY CONTRACT: every handler in this subtree issues `SELECT`s
//! only — no writes, ever. (The one sanctioned write in the observability
//! design is worker instrumentation inserting into `backfill_run`, and that
//! lives in `crates/worker`, not here.)
//!
//! AUTH: the whole subtree is registered behind [`crate::auth::admin_gate`]
//! (`X-Admin-Token`; unset token = 401, fail closed) — same posture as the
//! reviewer surface.
//!
//! FAIL CLOSED / HONEST GAPS: metrics that cannot be observed in the current
//! environment are rendered as explicit unavailable states (e.g.
//! `/v1/admin/loop` answers 503 when [`crate::ApiConfig::repo_root`] is
//! `None`), never guessed, never faked.
//!
//! Pattern per handler module (copied from `routes/review.rs`): DTOs are
//! `Serialize + ToSchema` prefixed `Admin*`, `#[utoipa::path]` on each
//! handler, runtime `sqlx::query_as`, [`crate::error::ApiError`] envelope.

pub mod backfill;
pub mod coverage;
pub mod infra;
pub mod loop_meta;
pub mod overview;
pub mod pipeline;
pub mod quality;
pub mod serving;
pub mod storage;

/// The shared `regime_code` ↔ `regime_id` bridge CTE.
///
/// Gold rows carry `regime_id` (a ULID) while operational tables
/// (`pipeline_run`, `sentinel_watch`, `backfill_run`) speak `regime_code`
/// (the adapter id, e.g. `us_house`). No mapping table exists; the honest
/// join path is provenance itself:
/// `filing → raw_document → pipeline_run.adapter` via `fetch_run_id`.
///
/// Compose with `const_format::concatcp!` — either append a statement that
/// joins `bridge`, or extend the CTE list with `", extra as (...) select …"`.
/// Rows whose `regime_id` never appears in `bridge` (e.g. fixture-seeded
/// docs with no fetch run) must be surfaced as an explicit "unbridged"
/// bucket, never silently dropped or guessed.
pub(crate) const BRIDGE_CTE: &str = "with bridge as (select distinct pr.adapter as regime_code, f.regime_id \
     from filing f \
     join raw_document rd on rd.id = f.raw_document_id \
     join pipeline_run pr on pr.id = rd.fetch_run_id)";

#[cfg(test)]
mod tests {
    use super::BRIDGE_CTE;

    #[test]
    fn bridge_cte_is_a_composable_with_clause() {
        assert!(BRIDGE_CTE.starts_with("with bridge as ("));
        assert!(
            BRIDGE_CTE.ends_with(')'),
            "extendable via `, extra as (...)`"
        );
        assert!(BRIDGE_CTE.contains("pr.adapter as regime_code"));
        assert!(BRIDGE_CTE.contains("f.regime_id"));
    }
}
