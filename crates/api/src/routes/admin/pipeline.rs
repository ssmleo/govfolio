//! `GET /v1/admin/pipeline` — section C, pipeline health: adapter/freeze
//! board (C1), per-adapter stage funnel from `pipeline_run` + `PublishStats`
//! jsonb (C2), drift incidents by kind (C3), last 25 failed runs with error
//! text (C4), conformance run-locally note (C5, no fake status), supersede
//! activity (C6, invariant-1 evidence).
//!
//! The funnel's `PublishStats` sums are filtered to `stage = 'publish'` —
//! the five keys (`candidates`, `gold_inserted`, `outbox_written`,
//! `review_tasks`, `suppressed`) are exactly what
//! `pipeline::stages::publish::PublishStats` serializes into
//! `pipeline_run.stats`; other stages carry other payloads and answer `null`.

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// ------------------------------------------------------------ page captions --

/// C5: conformance results are not in the database — say so, never fake a
/// status light.
const CONFORMANCE_NOTE: &str = "Conformance results are not stored in the database; \
     run locally: `cargo run -p pipeline --bin conformance -- <adapter>`. \
     No status is shown because none is recorded.";

// ------------------------------------------------------------- wire shapes --

/// One row of the adapter/freeze board (C1): sentinel state joined to its
/// open drift reports.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminFreezeBoardRow {
    /// Adapter regime code (e.g. `us_house`).
    pub regime_code: String,
    /// Whether the sentinel froze this regime's publication (design §5.6).
    pub frozen: bool,
    /// Drift kind that froze it, when recorded.
    pub frozen_kind: Option<String>,
    /// When the freeze happened, when recorded.
    pub frozen_at: Option<DateTime<Utc>>,
    /// When the sentinel last probed this source.
    pub last_checked_at: DateTime<Utc>,
    /// Open `drift_report` rows for this regime.
    pub open_drift_count: i64,
    /// Kind of the highest-priority open drift, when any is open.
    pub worst_open_drift_kind: Option<String>,
}

/// One (adapter, stage) funnel cell (C2): run counts by status plus the
/// `PublishStats` rollup for publish-stage cells.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminFunnelRow {
    /// Adapter regime code.
    pub adapter: String,
    /// Pipeline stage (e.g. `fetch`, `publish`).
    pub stage: String,
    /// Total runs recorded for the cell.
    pub runs: i64,
    /// Runs that `succeeded`.
    pub succeeded: i64,
    /// Runs that `failed`.
    pub failed: i64,
    /// Runs still `running`.
    pub running: i64,
    /// Sum of `PublishStats.candidates`; `null` off the publish stage (the
    /// jsonb keys only exist there — never guessed for other stages).
    pub candidates: Option<i64>,
    /// Sum of `PublishStats.gold_inserted`; `null` off the publish stage.
    pub gold_inserted: Option<i64>,
    /// Sum of `PublishStats.outbox_written`; `null` off the publish stage.
    pub outbox_written: Option<i64>,
    /// Sum of `PublishStats.review_tasks`; `null` off the publish stage.
    pub review_tasks: Option<i64>,
    /// Sum of `PublishStats.suppressed`; `null` off the publish stage.
    pub suppressed: Option<i64>,
}

/// Drift incidents for one kind (C3), across all regimes.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminDriftKindRow {
    /// One of the six kinds (`layout_shift`, `count_zero`, `regime_change`,
    /// `status_error`, `probe_error`, `count_delta`).
    pub drift_kind: String,
    /// Reports currently `open`.
    pub open_count: i64,
    /// Reports `resolved`.
    pub resolved_count: i64,
    /// Reports `superseded`.
    pub superseded_count: i64,
    /// Total detections across all reports of this kind (re-detections bump
    /// the counter instead of duplicating rows).
    pub detections: i64,
}

/// One failed `pipeline_run` (C4) — raw error text, no fragile
/// classification.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminFailedRun {
    /// Run ULID (time-sortable).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Adapter regime code.
    pub adapter: String,
    /// Pipeline stage that failed.
    pub stage: String,
    /// Error text as recorded, verbatim.
    pub error: Option<String>,
    /// When the run started.
    pub started_at: DateTime<Utc>,
    /// When the run finished, when recorded.
    pub finished_at: Option<DateTime<Utc>>,
}

/// Supersede activity for one month (C6, invariant-1 evidence): corrections
/// insert superseding rows, never update.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminSupersedeMonth {
    /// Month label, `YYYY-MM` (UTC).
    pub month: String,
    /// `disclosure_record` rows inserted as supersessions that month (by
    /// `created_at`).
    pub superseding_records: i64,
    /// Amended filings discovered that month (`supersedes_filing_id` set, by
    /// `discovered_at`).
    pub amended_filings: i64,
}

/// Section C in one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminPipeline {
    /// When this snapshot was assembled (server clock, UTC).
    pub generated_at: DateTime<Utc>,
    /// Adapter/freeze board (C1), frozen regimes first.
    pub freeze_board: Vec<AdminFreezeBoardRow>,
    /// Stage funnel per adapter (C2).
    pub funnel: Vec<AdminFunnelRow>,
    /// Drift incidents by kind (C3).
    pub drift_by_kind: Vec<AdminDriftKindRow>,
    /// Last 25 failed runs, newest first (C4).
    pub recent_failures: Vec<AdminFailedRun>,
    /// C5 honesty caption: conformance is run locally, not recorded.
    pub conformance_note: String,
    /// Supersede activity per month, newest first (C6).
    pub supersede_activity: Vec<AdminSupersedeMonth>,
}

// -------------------------------------------------------------------- SQL --

const FREEZE_BOARD_SQL: &str = "select sw.regime_code, sw.frozen, sw.frozen_kind, \
     sw.frozen_at, sw.last_checked_at, \
     (select count(*) from drift_report dr \
        where dr.regime_code = sw.regime_code and dr.status = 'open') as open_drift_count, \
     (select dr.drift_kind from drift_report dr \
        where dr.regime_code = sw.regime_code and dr.status = 'open' \
        order by dr.priority_score desc, dr.id limit 1) as worst_open_drift_kind \
     from sentinel_watch sw \
     order by sw.frozen desc, sw.regime_code";

/// C2: counts per (adapter, stage) plus `PublishStats` jsonb sums, filtered
/// to `stage = 'publish'` (the keys are the `PublishStats` serialization;
/// `'{}'` stats on running/failed rows yield `null` per row, which `sum`
/// ignores — non-publish cells answer `null`, never zero-faked).
const FUNNEL_SQL: &str = "select adapter, stage, \
     count(*) as runs, \
     count(*) filter (where status = 'succeeded') as succeeded, \
     count(*) filter (where status = 'failed') as failed, \
     count(*) filter (where status = 'running') as running, \
     (sum((stats->>'candidates')::bigint) filter (where stage = 'publish'))::bigint as candidates, \
     (sum((stats->>'gold_inserted')::bigint) filter (where stage = 'publish'))::bigint as gold_inserted, \
     (sum((stats->>'outbox_written')::bigint) filter (where stage = 'publish'))::bigint as outbox_written, \
     (sum((stats->>'review_tasks')::bigint) filter (where stage = 'publish'))::bigint as review_tasks, \
     (sum((stats->>'suppressed')::bigint) filter (where stage = 'publish'))::bigint as suppressed \
     from pipeline_run \
     group by adapter, stage \
     order by adapter, stage";

const DRIFT_BY_KIND_SQL: &str = "select drift_kind, \
     count(*) filter (where status = 'open') as open_count, \
     count(*) filter (where status = 'resolved') as resolved_count, \
     count(*) filter (where status = 'superseded') as superseded_count, \
     coalesce(sum(detections), 0)::bigint as detections \
     from drift_report \
     group by drift_kind \
     order by drift_kind";

const RECENT_FAILURES_SQL: &str = "select id, adapter, stage, error, started_at, finished_at \
     from pipeline_run \
     where status = 'failed' \
     order by started_at desc \
     limit 25";

/// C6: superseding Gold rows by `created_at` month, amended filings by
/// `discovered_at` month (filings carry no `created_at`; discovery time is
/// the honest stamp we have) — FULL OUTER joined so a month active on only
/// one side still surfaces.
const SUPERSEDE_SQL: &str = "with rec as ( \
       select date_trunc('month', created_at) as month_start, \
              count(*) as superseding_records \
       from disclosure_record \
       where supersedes_record_id is not null \
       group by 1), \
     fil as ( \
       select date_trunc('month', discovered_at) as month_start, \
              count(*) as amended_filings \
       from filing \
       where supersedes_filing_id is not null \
       group by 1) \
     select to_char(coalesce(rec.month_start, fil.month_start) at time zone 'UTC', 'YYYY-MM') as month, \
            coalesce(rec.superseding_records, 0) as superseding_records, \
            coalesce(fil.amended_filings, 0) as amended_filings \
     from rec \
     full outer join fil on fil.month_start = rec.month_start \
     order by 1 desc \
     limit 48";

// ------------------------------------------------------------ the handler --

/// Section C — pipeline health: freeze board, stage funnel, drift by kind,
/// recent failures, conformance note, supersede activity — one round trip.
///
/// # Errors
/// `401` without a valid `X-Admin-Token` (the gate wraps the whole admin
/// subtree); `500` on backend failure — all in the consistent envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/pipeline",
    tag = "admin",
    responses(
        (status = 200, description = "The pipeline-health snapshot", body = AdminPipeline),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_pipeline(
    State(state): State<AppState>,
) -> Result<Json<AdminPipeline>, ApiError> {
    let freeze_board: Vec<AdminFreezeBoardRow> = sqlx::query_as(FREEZE_BOARD_SQL)
        .fetch_all(&state.pool)
        .await?;
    let funnel: Vec<AdminFunnelRow> = sqlx::query_as(FUNNEL_SQL).fetch_all(&state.pool).await?;
    let drift_by_kind: Vec<AdminDriftKindRow> = sqlx::query_as(DRIFT_BY_KIND_SQL)
        .fetch_all(&state.pool)
        .await?;
    let recent_failures: Vec<AdminFailedRun> = sqlx::query_as(RECENT_FAILURES_SQL)
        .fetch_all(&state.pool)
        .await?;
    let supersede_activity: Vec<AdminSupersedeMonth> =
        sqlx::query_as(SUPERSEDE_SQL).fetch_all(&state.pool).await?;
    Ok(Json(AdminPipeline {
        generated_at: Utc::now(),
        freeze_board,
        funnel,
        drift_by_kind,
        recent_failures,
        conformance_note: CONFORMANCE_NOTE.to_owned(),
        supersede_activity,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        DRIFT_BY_KIND_SQL, FREEZE_BOARD_SQL, FUNNEL_SQL, RECENT_FAILURES_SQL, SUPERSEDE_SQL,
    };

    /// READ-ONLY BY CONTRACT: every admin statement is a `select`/`with`.
    #[test]
    fn every_statement_is_read_only() {
        for sql in [
            FREEZE_BOARD_SQL,
            FUNNEL_SQL,
            DRIFT_BY_KIND_SQL,
            RECENT_FAILURES_SQL,
            SUPERSEDE_SQL,
        ] {
            assert!(
                sql.starts_with("select ") || sql.starts_with("with "),
                "not a select/with: {sql}"
            );
            for verb in ["insert ", "update ", "delete ", "truncate ", "drop "] {
                assert!(!sql.contains(verb), "write verb {verb:?} in: {sql}");
            }
        }
    }

    /// The funnel sums exactly the five `PublishStats` keys the publish
    /// stage serializes (crates/pipeline/src/stages/publish.rs) — a renamed
    /// key there must break this test, not silently null the dashboard.
    #[test]
    fn funnel_sums_the_publish_stats_keys() {
        for key in [
            "candidates",
            "gold_inserted",
            "outbox_written",
            "review_tasks",
            "suppressed",
        ] {
            assert!(
                FUNNEL_SQL.contains(&format!("stats->>'{key}'")),
                "missing PublishStats key {key}"
            );
        }
    }
}
