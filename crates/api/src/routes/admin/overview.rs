//! `GET /v1/admin/overview` — the status strip: every queue depth (outbox
//! undispatched, review open, drift open, sample pending, delivery DLQ,
//! unbilled usage, `pipeline_run` running/failed), 24h run counts by status,
//! frozen regimes and the last sentinel check.
//!
//! Cheap indexed counts only — the web strip polls this endpoint every 15s,
//! so every statement is a `count(*)` answered by a partial index or a small
//! operational table; nothing scans Gold (its size ships as the planner's
//! `reltuples` estimate, never a count).

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// ------------------------------------------------------------- wire shapes --

/// Every operational queue depth on one row (plan B5). Shared verbatim with
/// `/v1/admin/backfill` so the two surfaces can never disagree on shape.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminQueueDepths {
    /// `outbox_event` rows not yet dispatched.
    pub outbox_undispatched: i64,
    /// Open `review_task` rows.
    pub review_open: i64,
    /// Open `drift_report` rows.
    pub drift_open: i64,
    /// `sample_audit` rows still awaiting an audit verdict.
    pub sample_pending: i64,
    /// Dead-letter deliveries (`delivery.status = 'dead'`).
    pub delivery_dlq: i64,
    /// `usage_event` rows not yet folded into a `usage_report`.
    pub usage_unbilled: i64,
    /// `pipeline_run` rows still `running`.
    pub pipeline_running: i64,
    /// `pipeline_run` rows that ended `failed`.
    pub pipeline_failed: i64,
}

/// `pipeline_run` counts by status over the trailing 24 hours (by
/// `started_at`).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminRuns24h {
    /// Runs still `running`.
    pub running: i64,
    /// Runs that `succeeded`.
    pub succeeded: i64,
    /// Runs that `failed`.
    pub failed: i64,
}

/// One regime the sentinel froze (design §5.6 fail-closed publication gate).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminFrozenRegime {
    /// Adapter regime code (e.g. `us_house`).
    pub regime_code: String,
    /// Drift kind that froze it (e.g. `layout_shift`), when recorded.
    pub frozen_kind: Option<String>,
    /// When the freeze happened, when recorded.
    pub frozen_at: Option<DateTime<Utc>>,
}

/// The whole status strip in one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminOverview {
    /// When this snapshot was assembled (server clock, UTC).
    pub generated_at: DateTime<Utc>,
    /// Every operational queue depth (plan B5).
    pub queue_depths: AdminQueueDepths,
    /// `pipeline_run` counts by status, trailing 24 hours.
    pub runs_24h: AdminRuns24h,
    /// Regimes currently frozen by the sentinel; empty = nothing frozen.
    pub frozen_regimes: Vec<AdminFrozenRegime>,
    /// Latest `sentinel_watch.last_checked_at` across all regimes; `null`
    /// when the sentinel has never run (honest absence, not zero).
    pub last_sentinel_check: Option<DateTime<Utc>>,
    /// Planner ESTIMATE of Gold `disclosure_record` rows, straight from
    /// `pg_class.reltuples` — free at any table size, never a `count(*)`
    /// (this strip polls every 15s and nothing here may scan Gold). Kept
    /// fresh by (auto)vacuum/analyze, so it drifts from the exact count;
    /// `null` when postgres has never analyzed the table (honest absence,
    /// not zero).
    pub gold_records_estimate: Option<i64>,
}

// -------------------------------------------------------------------- SQL --

/// Eight scalar subqueries, one row. Each is a `count(*)` over a partial
/// index (`outbox_undispatched`, `sample_audit_pending`, `delivery_dlq`,
/// `usage_event_unbilled`) or a small operational table.
const QUEUE_DEPTHS_SQL: &str = "select \
     (select count(*) from outbox_event where dispatched_at is null) as outbox_undispatched, \
     (select count(*) from review_task where status = 'open') as review_open, \
     (select count(*) from drift_report where status = 'open') as drift_open, \
     (select count(*) from sample_audit where status = 'pending') as sample_pending, \
     (select count(*) from delivery where status = 'dead') as delivery_dlq, \
     (select count(*) from usage_event where report_id is null) as usage_unbilled, \
     (select count(*) from pipeline_run where status = 'running') as pipeline_running, \
     (select count(*) from pipeline_run where status = 'failed') as pipeline_failed";

const RUNS_24H_SQL: &str = "select \
     count(*) filter (where status = 'running') as running, \
     count(*) filter (where status = 'succeeded') as succeeded, \
     count(*) filter (where status = 'failed') as failed \
     from pipeline_run where started_at > now() - interval '24 hours'";

const FROZEN_SQL: &str = "select regime_code, frozen_kind, frozen_at \
     from sentinel_watch where frozen order by regime_code";

const LAST_SENTINEL_SQL: &str = "select max(last_checked_at) from sentinel_watch";

/// The planner's row estimate for Gold — `reltuples` is maintained by
/// (auto)vacuum/analyze and reads in O(1); `-1` is postgres's "never
/// analyzed" sentinel, mapped to `null` here. The `relkind`/`relnamespace`
/// filters keep an index or other-schema object named `disclosure_record`
/// from ever shadowing the table.
const GOLD_ESTIMATE_SQL: &str = "select case when reltuples < 0 then null \
     else reltuples::bigint end from pg_class \
     where relname = 'disclosure_record' and relkind = 'r' \
     and relnamespace = 'public'::regnamespace";

/// Fetches the queue depths — shared with `/v1/admin/backfill` (plan B5).
pub(crate) async fn fetch_queue_depths(pool: &PgPool) -> Result<AdminQueueDepths, ApiError> {
    Ok(sqlx::query_as(QUEUE_DEPTHS_SQL).fetch_one(pool).await?)
}

// ------------------------------------------------------------ the handler --

/// The admin status strip: queue depths, 24h run counts, frozen regimes,
/// last sentinel check — one round trip, cheap indexed counts only.
///
/// # Errors
/// `401` without a valid `X-Admin-Token` (the gate wraps the whole admin
/// subtree); `500` on backend failure — all in the consistent envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/overview",
    tag = "admin",
    responses(
        (status = 200, description = "The status strip snapshot", body = AdminOverview),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_overview(
    State(state): State<AppState>,
) -> Result<Json<AdminOverview>, ApiError> {
    let queue_depths = fetch_queue_depths(&state.pool).await?;
    let runs_24h: AdminRuns24h = sqlx::query_as(RUNS_24H_SQL).fetch_one(&state.pool).await?;
    let frozen_regimes: Vec<AdminFrozenRegime> =
        sqlx::query_as(FROZEN_SQL).fetch_all(&state.pool).await?;
    let last_sentinel_check: Option<DateTime<Utc>> = sqlx::query_scalar(LAST_SENTINEL_SQL)
        .fetch_one(&state.pool)
        .await?;
    // fetch_optional + flatten: a missing pg_class row (wrong database)
    // degrades this one figure to null instead of 500-ing the whole strip.
    let gold_records_estimate: Option<i64> = sqlx::query_scalar(GOLD_ESTIMATE_SQL)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    Ok(Json(AdminOverview {
        generated_at: Utc::now(),
        queue_depths,
        runs_24h,
        frozen_regimes,
        last_sentinel_check,
        gold_records_estimate,
    }))
}

#[cfg(test)]
mod tests {
    use super::{FROZEN_SQL, GOLD_ESTIMATE_SQL, LAST_SENTINEL_SQL, QUEUE_DEPTHS_SQL, RUNS_24H_SQL};

    /// READ-ONLY BY CONTRACT: every admin statement is a `select`.
    #[test]
    fn every_statement_is_a_select() {
        for sql in [
            QUEUE_DEPTHS_SQL,
            RUNS_24H_SQL,
            FROZEN_SQL,
            LAST_SENTINEL_SQL,
            GOLD_ESTIMATE_SQL,
        ] {
            assert!(sql.starts_with("select "), "not a select: {sql}");
            for verb in ["insert ", "update ", "delete ", "truncate ", "drop "] {
                assert!(!sql.contains(verb), "write verb {verb:?} in: {sql}");
            }
        }
    }
}
