//! Read-only admin ops observability surface (goal 090): `/healthz` plus the
//! eight `/v1/admin/ops/*` endpoints behind the `admin_gate` route layer —
//! backfill progress, pipeline runs, freezes/drift, review-queue health,
//! alert delivery, and LLM extraction cost vs the monthly HARD CAP.
//!
//! READ ONLY by construction: this module contains no INSERT/UPDATE/DELETE.
//! Data truth is Postgres (`pipeline_run`, `review_task`, `sentinel_watch`,
//! `drift_report`, `sample_audit`, `delivery`, `outbox_event`, Bronze/Silver/
//! Gold); worker `RunReport` counters are stdout-only and never persisted, so
//! nothing here depends on them.
//!
//! Regime code ↔ ULID mapping: `pipeline_run.adapter`, `sentinel_watch` and
//! `drift_report` speak the adapter REGIME CODE (`us_house`, `br`, ...);
//! `filing.regime_id` / `disclosure_record.regime_id` speak the seeded regime
//! ULID. No `regime_code` column exists, so the mapping is resolved from (in
//! order): the registry seed's `disclosure_regime.details->>'regime_code'`
//! (the eight live launch regimes), `govfolio_core::seed::LIVE_REGIMES`
//! (same eight, belt and suspenders), and the data-derived publish linkage
//! (`pipeline_run.stats->>'filing_id'` → `filing.regime_id`, which is how
//! adapter-seeded regimes like `br` — two regime rows, one adapter code —
//! get attributed).

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive as _;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::AppState;
use crate::dto::validate_page_params;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;

/// LLM extraction monthly HARD CAP in USD — founder decision 2026-07-08,
/// goal 021 Phase 2 (automation-policy billing lane: auto within the cap,
/// halt over it). Serialized as the decimal STRING `"200.00"` (invariant 7);
/// enforcement lives in the pipeline (goal 021), this surface only reports
/// utilization against it.
pub const LLM_MONTHLY_HARD_CAP: Decimal = Decimal::from_parts(20_000, 0, 0, false, 2);

/// Money → decimal string with exactly two fraction digits (invariant 7).
fn usd(amount: Decimal) -> String {
    format!("{:.2}", amount.round_dp(2))
}

/// Resolves an adapter regime code for a seeded regime row from the static
/// sources (registry-seed `details.regime_code`, then the core seed consts).
/// `None` falls through to the data-derived publish linkage (module docs).
fn static_regime_code(regime_id: &str, details: &serde_json::Value) -> Option<String> {
    if let Some(code) = details.get("regime_code").and_then(|v| v.as_str()) {
        return Some(code.to_owned());
    }
    govfolio_core::seed::LIVE_REGIMES
        .iter()
        .find(|regime| regime.regime_id == regime_id)
        .map(|regime| regime.code.to_owned())
}

// -------------------------------------------------------- param validation --

/// Validates the runs `status` filter: `running` | `succeeded` | `failed`
/// (the `pipeline_run.status` CHECK vocabulary); absent = no filter.
fn validate_run_status(status: Option<String>) -> Result<Option<String>, ApiError> {
    match status {
        None => Ok(None),
        Some(status) if matches!(status.as_str(), "running" | "succeeded" | "failed") => {
            Ok(Some(status))
        }
        Some(other) => Err(ApiError::bad_request(
            "invalid_status",
            format!("status must be running|succeeded|failed, got {other:?}"),
        )),
    }
}

/// Validates the summary window: `1..=720` hours (30 days), default 24.
fn validate_hours(hours: Option<u32>) -> Result<i32, ApiError> {
    let hours = hours.unwrap_or(24);
    if !(1..=720).contains(&hours) {
        return Err(ApiError::bad_request(
            "invalid_hours",
            format!("hours must be within 1..=720, got {hours}"),
        ));
    }
    Ok(i32::try_from(hours).map_err(|e| anyhow::anyhow!("hours fits i32: {e}"))?)
}

/// Validates the summary bucket: `hour` (default) | `day` — the only two
/// `date_trunc` units the series chart needs.
fn validate_bucket(bucket: Option<String>) -> Result<String, ApiError> {
    let bucket = bucket.unwrap_or_else(|| "hour".to_owned());
    if !matches!(bucket.as_str(), "hour" | "day") {
        return Err(ApiError::bad_request(
            "invalid_bucket",
            format!("bucket must be hour|day, got {bucket:?}"),
        ));
    }
    Ok(bucket)
}

/// Validates the cost window: `1..=24` months, default 3.
fn validate_months(months: Option<u32>) -> Result<i32, ApiError> {
    let months = months.unwrap_or(3);
    if !(1..=24).contains(&months) {
        return Err(ApiError::bad_request(
            "invalid_months",
            format!("months must be within 1..=24, got {months}"),
        ));
    }
    Ok(i32::try_from(months).map_err(|e| anyhow::anyhow!("months fits i32: {e}"))?)
}

/// Validates the freezes scope: `open` (default) | `all`; returns whether
/// resolved/superseded drift reports are included.
fn validate_freeze_scope(status: Option<String>) -> Result<bool, ApiError> {
    let status = status.unwrap_or_else(|| "open".to_owned());
    match status.as_str() {
        "open" => Ok(false),
        "all" => Ok(true),
        other => Err(ApiError::bad_request(
            "invalid_status",
            format!("status must be open|all, got {other:?}"),
        )),
    }
}

// ------------------------------------------------------------------ healthz --

/// Liveness probe body: process is up; `db` reports pool reachability.
/// Deliberately carries NOTHING sensitive (served without auth).
#[derive(Debug, Serialize, ToSchema)]
pub struct Healthz {
    /// `ok` when the process and its database both answer; `degraded` when
    /// the database does not.
    pub status: String,
    /// `ok` | `error` — result of `select 1` on the pool.
    pub db: String,
}

/// Liveness probe. Unauthenticated and un-ETagged by mounting position
/// (added AFTER the middleware layers in `lib.rs` — axum layers only wrap
/// routes added before them). Always `200`; database trouble is reported in
/// the body, not as a 5xx, so the probe distinguishes "process dead" (no
/// answer) from "db unreachable" (degraded).
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "health",
    responses(
        (status = 200, description = "Process liveness + database reachability", body = Healthz),
    ),
)]
pub async fn healthz(State(state): State<AppState>) -> Json<Healthz> {
    let db_ok = sqlx::query_scalar::<_, i32>("select 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    Json(Healthz {
        status: if db_ok { "ok" } else { "degraded" }.to_owned(),
        db: if db_ok { "ok" } else { "error" }.to_owned(),
    })
}

// ----------------------------------------------------------------- overview --

/// Whole-system totals (design §4.2 tables, one count each).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct OpsTotals {
    /// Bronze `raw_document` rows (sha256-addressed, immutable).
    pub bronze_documents: i64,
    /// Silver staging rows across the per-regime `stg_*` tables.
    pub silver_rows: i64,
    /// `filing` rows.
    pub filings: i64,
    /// Gold `disclosure_record` rows.
    pub gold_records: i64,
    /// Gold rows still `unverified` (design §7.1 two-stage publication).
    pub gold_unverified: i64,
    /// `politician` rows.
    pub politicians: i64,
    /// Open review tasks.
    pub review_open: i64,
    /// Regimes currently frozen by the sentinel (design §5.6 fail closed).
    pub frozen_regimes: i64,
    /// Open drift reports.
    pub drift_open: i64,
    /// Dead-letter deliveries (design §6.3 DLQ).
    pub deliveries_dead: i64,
    /// Outbox events not yet dispatched.
    pub outbox_undispatched: i64,
}

/// Pipeline-run pulse over the trailing 24 hours (running counts are global,
/// not window-bounded — a run started days ago and still `running` is stale
/// wherever it started).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct Runs24h {
    /// Runs started in the window.
    pub started: i64,
    /// Of those, finished `succeeded`.
    pub succeeded: i64,
    /// Of those, finished `failed`.
    pub failed: i64,
    /// Runs currently `running` (any age).
    pub running_now: i64,
    /// Runs `running` for over an hour — likely crashed before finish.
    pub stale_running: i64,
}

/// Current-month LLM extraction spend vs the HARD CAP. Zeros until goal 021
/// Phase 2 starts writing `pipeline_run.stats.extraction` (see
/// [`extraction_costs`] for the key contract).
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtractionMonth {
    /// Month label, `YYYY-MM` (UTC).
    pub month: String,
    /// Input tokens consumed this month.
    pub tokens_in: i64,
    /// Output tokens produced this month.
    pub tokens_out: i64,
    /// Estimated spend, decimal string (invariant 7).
    pub estimated_cost_usd: String,
    /// The monthly HARD CAP, decimal string — always `"200.00"`.
    pub hard_cap_usd: String,
    /// Spend as a percentage of the cap (derived display number, not money).
    pub cap_utilization_pct: f64,
}

/// Last-seen timestamps — the "is anything alive" pulse row.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct LastActivity {
    /// Latest `pipeline_run.started_at`.
    pub last_run_started_at: Option<DateTime<Utc>>,
    /// Latest successful publish finish.
    pub last_publish_succeeded_at: Option<DateTime<Utc>>,
    /// Latest sentinel probe (`sentinel_watch.last_checked_at`).
    pub last_sentinel_check_at: Option<DateTime<Utc>>,
    /// Latest outbox dispatch.
    pub last_outbox_dispatch_at: Option<DateTime<Utc>>,
    /// Latest Gold row creation.
    pub last_gold_created_at: Option<DateTime<Utc>>,
}

/// The composite overview panel — one cheap body per poll (5–15s cadence),
/// high `ETag` 304 hit-rate when nothing moved.
#[derive(Debug, Serialize, ToSchema)]
pub struct OpsOverview {
    /// When this body was computed (UTC).
    pub generated_at: DateTime<Utc>,
    /// Whole-system totals.
    pub totals: OpsTotals,
    /// Trailing-24h run pulse.
    pub runs_24h: Runs24h,
    /// Current-month extraction spend vs the HARD CAP.
    pub extraction_month: ExtractionMonth,
    /// Last-seen activity timestamps.
    pub last_activity: LastActivity,
}

/// Silver is per-regime staging tables (`stg_<regime>`, design §4.2) — the
/// overview sums the ones that exist today; a new adapter's table joins this
/// list with its migration. Mechanically gated: `tests/ops.rs`
/// `silver_union_covers_every_stg_table` enumerates `information_schema`
/// `stg_%` tables and fails when one is missing from this union (or from
/// [`BACKFILL_SILVER_SQL`]) — `pub` so that gate can read the SQL.
pub const TOTALS_SQL: &str = "select \
   (select count(*) from raw_document) as bronze_documents, \
   (select (select count(*) from stg_us_house) + (select count(*) from stg_br)) as silver_rows, \
   (select count(*) from filing) as filings, \
   (select count(*) from disclosure_record) as gold_records, \
   (select count(*) from disclosure_record where verification_state = 'unverified') as gold_unverified, \
   (select count(*) from politician) as politicians, \
   (select count(*) from review_task where status = 'open') as review_open, \
   (select count(*) from sentinel_watch where frozen) as frozen_regimes, \
   (select count(*) from drift_report where status = 'open') as drift_open, \
   (select count(*) from delivery where status = 'dead') as deliveries_dead, \
   (select count(*) from outbox_event where dispatched_at is null) as outbox_undispatched";

const RUNS_24H_SQL: &str = "select \
   count(*) filter (where started_at >= now() - interval '24 hours') as started, \
   count(*) filter (where started_at >= now() - interval '24 hours' \
                      and status = 'succeeded') as succeeded, \
   count(*) filter (where started_at >= now() - interval '24 hours' \
                      and status = 'failed') as failed, \
   count(*) filter (where status = 'running') as running_now, \
   count(*) filter (where status = 'running' \
                      and started_at < now() - interval '1 hour') as stale_running \
 from pipeline_run";

const LAST_ACTIVITY_SQL: &str = "select \
   (select max(started_at) from pipeline_run) as last_run_started_at, \
   (select max(finished_at) from pipeline_run \
     where stage = 'publish' and status = 'succeeded') as last_publish_succeeded_at, \
   (select max(last_checked_at) from sentinel_watch) as last_sentinel_check_at, \
   (select max(dispatched_at) from outbox_event) as last_outbox_dispatch_at, \
   (select max(created_at) from disclosure_record) as last_gold_created_at";

/// Current-month extraction rollup. Null-tolerant by regex-guarded casts:
/// `stats.extraction` does not exist until goal 021 Phase 2 lands it, and a
/// malformed value must degrade to zero, never 500 the ops surface.
const EXTRACTION_MONTH_SQL: &str = "select \
   coalesce(sum(case when stats->'extraction'->>'tokens_in' ~ '^[0-9]+$' \
                     then (stats->'extraction'->>'tokens_in')::bigint else 0 end), 0)::bigint \
     as tokens_in, \
   coalesce(sum(case when stats->'extraction'->>'tokens_out' ~ '^[0-9]+$' \
                     then (stats->'extraction'->>'tokens_out')::bigint else 0 end), 0)::bigint \
     as tokens_out, \
   coalesce(sum(case when stats->'extraction'->>'estimated_cost_usd' ~ '^[0-9]+(\\.[0-9]+)?$' \
                     then (stats->'extraction'->>'estimated_cost_usd')::numeric else 0 end), 0) \
     as estimated_cost_usd \
 from pipeline_run \
 where stage = 'parse' and started_at >= date_trunc('month', now())";

#[derive(Debug, sqlx::FromRow)]
struct ExtractionMonthRow {
    tokens_in: i64,
    tokens_out: i64,
    estimated_cost_usd: Decimal,
}

/// Builds the current-month spend block (shared by overview and unit tests).
fn extraction_month(now: DateTime<Utc>, row: &ExtractionMonthRow) -> ExtractionMonth {
    let pct = (row.estimated_cost_usd / LLM_MONTHLY_HARD_CAP * Decimal::ONE_HUNDRED)
        .round_dp(2)
        .to_f64()
        .unwrap_or(0.0);
    ExtractionMonth {
        month: now.format("%Y-%m").to_string(),
        tokens_in: row.tokens_in,
        tokens_out: row.tokens_out,
        estimated_cost_usd: usd(row.estimated_cost_usd),
        hard_cap_usd: usd(LLM_MONTHLY_HARD_CAP),
        cap_utilization_pct: pct,
    }
}

/// The composite ops overview: totals, 24h run pulse, current-month
/// extraction spend vs the HARD CAP, and last-activity timestamps.
///
/// # Errors
/// `401` outside the admin gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/overview",
    tag = "ops",
    responses(
        (status = 200, description = "Whole-system ops overview", body = OpsOverview),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn overview(State(state): State<AppState>) -> Result<Json<OpsOverview>, ApiError> {
    let totals: OpsTotals = sqlx::query_as(TOTALS_SQL).fetch_one(&state.pool).await?;
    let runs_24h: Runs24h = sqlx::query_as(RUNS_24H_SQL).fetch_one(&state.pool).await?;
    let month_row: ExtractionMonthRow = sqlx::query_as(EXTRACTION_MONTH_SQL)
        .fetch_one(&state.pool)
        .await?;
    let last_activity: LastActivity = sqlx::query_as(LAST_ACTIVITY_SQL)
        .fetch_one(&state.pool)
        .await?;
    let now = Utc::now();
    Ok(Json(OpsOverview {
        generated_at: now,
        totals,
        runs_24h,
        extraction_month: extraction_month(now, &month_row),
        last_activity,
    }))
}

// --------------------------------------------------------------------- runs --

/// One `pipeline_run` row (design §5.2 audit trail).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct PipelineRun {
    /// Run ULID, minted at claim — time-ordered, the pagination cursor.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Adapter regime code, e.g. `us_house`.
    pub adapter: String,
    /// Stage name, e.g. `fetch` | `parse` | `normalize` | `publish`.
    pub stage: String,
    /// `running` | `succeeded` | `failed`.
    pub status: String,
    /// Deterministic stage-unit key (crash-safe replay short-circuit).
    pub idempotency_key: String,
    /// Stage audit stats (e.g. `PublishStats`; `{}` until finish).
    #[schema(value_type = Object)]
    pub stats: serde_json::Value,
    /// Failure message, for `failed` runs.
    pub error: Option<String>,
    /// When the run was claimed.
    pub started_at: DateTime<Utc>,
    /// When it finished; `null` while running.
    pub finished_at: Option<DateTime<Utc>>,
}

/// One page of runs, newest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineRunPage {
    /// Runs in descending id (= claim-time) order.
    pub items: Vec<PipelineRun>,
    /// Pass back as `cursor` for the next (older) page; `null` at the end.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub next_cursor: Option<String>,
}

/// Query parameters of `GET /v1/admin/ops/runs`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OpsRunsParams {
    /// Adapter regime-code filter, e.g. `us_house`.
    pub adapter: Option<String>,
    /// Stage filter, e.g. `publish`.
    pub stage: Option<String>,
    /// Status filter: `running` | `succeeded` | `failed`.
    pub status: Option<String>,
    /// Only runs started at/after this instant (RFC 3339).
    pub since: Option<DateTime<Utc>>,
    /// Only runs started before this instant (RFC 3339).
    pub until: Option<DateTime<Utc>>,
    /// Pagination cursor: the run id of the last item on the previous page.
    #[param(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub cursor: Option<String>,
    /// Page size, `1..=200`; defaults to 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// Keyset by `id desc` (run ULIDs are minted at claim, so id order is claim
/// time; a verified retry re-uses its row without resetting `started_at`,
/// `pipeline_run.rs:75-79`). Filters arrive as nullable binds.
const RUNS_SQL: &str = "select id, adapter, stage, status, idempotency_key, stats, error, \
        started_at, finished_at \
 from pipeline_run \
 where ($1::text is null or adapter = $1) \
   and ($2::text is null or stage = $2) \
   and ($3::text is null or status = $3) \
   and ($4::timestamptz is null or started_at >= $4) \
   and ($5::timestamptz is null or started_at < $5) \
   and ($6::text is null or id < $6) \
 order by id desc \
 limit $7";

/// Lists pipeline runs, newest first, with adapter/stage/status/time filters.
///
/// # Errors
/// `400` on a malformed status, cursor or limit; `401` outside the admin
/// gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/runs",
    tag = "ops",
    params(OpsRunsParams),
    responses(
        (status = 200, description = "One page of pipeline runs, newest first", body = PipelineRunPage),
        (status = 400, description = "Malformed status, cursor or limit", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_runs(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<OpsRunsParams>,
) -> Result<Json<PipelineRunPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let status = validate_run_status(params.status)?;
    let rows: Vec<PipelineRun> = sqlx::query_as(RUNS_SQL)
        .bind(&params.adapter)
        .bind(&params.stage)
        .bind(&status)
        .bind(params.since)
        .bind(params.until)
        .bind(&cursor)
        .bind(limit + 1)
        .fetch_all(&state.pool)
        .await?;
    // limit+1 sentinel, same as dto::build_page: the extra row only proves
    // more data exists; next_cursor = last RETURNED id.
    let page_len = usize::try_from(limit).map_err(|e| anyhow::anyhow!("limit fits usize: {e}"))?;
    let has_more = rows.len() > page_len;
    let mut items = rows;
    items.truncate(page_len);
    let next_cursor = if has_more {
        items.last().map(|run| run.id.clone())
    } else {
        None
    };
    Ok(Json(PipelineRunPage { items, next_cursor }))
}

// ------------------------------------------------------------- runs summary --

/// One (adapter, stage, status) rollup over the window.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct RunGroup {
    /// Adapter regime code.
    pub adapter: String,
    /// Stage name.
    pub stage: String,
    /// `running` | `succeeded` | `failed`.
    pub status: String,
    /// Runs in the group.
    pub runs: i64,
    /// Median run duration in seconds (finished runs only; `null` when none
    /// finished — `percentile_cont` ignores null `finished_at` durations).
    pub p50_seconds: Option<f64>,
    /// 95th-percentile run duration in seconds (finished runs only).
    pub p95_seconds: Option<f64>,
}

/// One (bucket, adapter) point of the throughput series.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct RunSeriesPoint {
    /// Bucket start (UTC, `date_trunc` of the requested bucket).
    pub bucket_start: DateTime<Utc>,
    /// Adapter regime code.
    pub adapter: String,
    /// Runs finished `succeeded` in the bucket (by start time).
    pub succeeded: i64,
    /// Runs finished `failed` in the bucket (by start time).
    pub failed: i64,
    /// Gold rows inserted (summed from publish-run `stats.gold_inserted`).
    pub gold_inserted: i64,
    /// Review tasks opened (summed from publish-run `stats.review_tasks`).
    pub review_tasks: i64,
}

/// Windowed run rollup: per-(adapter, stage, status) groups with duration
/// percentiles, plus a bucketed throughput series.
#[derive(Debug, Serialize, ToSchema)]
pub struct RunsSummary {
    /// The trailing window the rollup covers, in hours.
    pub window_hours: i32,
    /// Series bucket unit: `hour` | `day`.
    pub bucket: String,
    /// Per-(adapter, stage, status) rollup.
    pub groups: Vec<RunGroup>,
    /// Bucketed throughput series.
    pub series: Vec<RunSeriesPoint>,
}

/// Query parameters of `GET /v1/admin/ops/runs/summary`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RunsSummaryParams {
    /// Trailing window in hours, `1..=720`; defaults to 24.
    #[param(minimum = 1, maximum = 720)]
    pub hours: Option<u32>,
    /// Series bucket: `hour` (default) | `day`.
    pub bucket: Option<String>,
}

const RUN_GROUPS_SQL: &str = "select adapter, stage, status, count(*) as runs, \
   percentile_cont(0.5) within group \
     (order by extract(epoch from finished_at - started_at)::double precision) as p50_seconds, \
   percentile_cont(0.95) within group \
     (order by extract(epoch from finished_at - started_at)::double precision) as p95_seconds \
 from pipeline_run \
 where started_at >= now() - make_interval(hours => $1) \
 group by adapter, stage, status \
 order by adapter, stage, status";

/// `gold_inserted`/`review_tasks` are summed from `PublishStats` keys on
/// publish rows (`publish.rs`); regex-guarded casts keep malformed stats at
/// zero instead of 500.
const RUN_SERIES_SQL: &str = "select date_trunc($2, started_at) as bucket_start, adapter, \
   count(*) filter (where status = 'succeeded') as succeeded, \
   count(*) filter (where status = 'failed') as failed, \
   coalesce(sum(case when stage = 'publish' and stats->>'gold_inserted' ~ '^[0-9]+$' \
                     then (stats->>'gold_inserted')::bigint else 0 end), 0)::bigint \
     as gold_inserted, \
   coalesce(sum(case when stage = 'publish' and stats->>'review_tasks' ~ '^[0-9]+$' \
                     then (stats->>'review_tasks')::bigint else 0 end), 0)::bigint \
     as review_tasks \
 from pipeline_run \
 where started_at >= now() - make_interval(hours => $1) \
 group by 1, adapter \
 order by 1, adapter";

/// Windowed run rollup with duration percentiles and a throughput series.
///
/// # Errors
/// `400` on out-of-range `hours` or a bad `bucket`; `401` outside the admin
/// gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/runs/summary",
    tag = "ops",
    params(RunsSummaryParams),
    responses(
        (status = 200, description = "Windowed run rollup + throughput series", body = RunsSummary),
        (status = 400, description = "Malformed hours or bucket", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn runs_summary(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<RunsSummaryParams>,
) -> Result<Json<RunsSummary>, ApiError> {
    let hours = validate_hours(params.hours)?;
    let bucket = validate_bucket(params.bucket)?;
    let groups: Vec<RunGroup> = sqlx::query_as(RUN_GROUPS_SQL)
        .bind(hours)
        .fetch_all(&state.pool)
        .await?;
    let series: Vec<RunSeriesPoint> = sqlx::query_as(RUN_SERIES_SQL)
        .bind(hours)
        .bind(&bucket)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(RunsSummary {
        window_hours: hours,
        bucket,
        groups,
        series,
    }))
}

// ----------------------------------------------------------------- backfill --

/// Per-regime whole-history totals.
#[derive(Debug, Serialize, ToSchema)]
pub struct RegimeTotals {
    /// Distinct Bronze documents that produced filings for this regime.
    pub bronze_documents: i64,
    /// Silver staging rows, attributed per DOCUMENT: a document shared by
    /// filings of more than one regime (e.g. one br CSV feeding both
    /// chambers) reports its full row count under each — Silver rows carry
    /// no regime linkage of their own.
    pub silver_rows: i64,
    /// Filings.
    pub filings: i64,
    /// Gold records.
    pub gold_records: i64,
    /// Gold records still `unverified`.
    pub gold_unverified: i64,
    /// Open review tasks attributed to this regime (three target kinds:
    /// record → `regime_id` join, bare filing id → `filing` join, and
    /// code-keyed `regime`/`filing` targets — code-keyed counts repeat on
    /// every regime row sharing the adapter code, e.g. both br chambers).
    pub review_open: i64,
}

/// Stage progress for the regime's adapter code. Failure attribution is
/// regime+stage level only — run idempotency keys carry `sha`/`external_id`, no
/// year (plan "Non-obvious query decisions").
#[derive(Debug, Serialize, ToSchema)]
pub struct StageProgress {
    /// Stage name.
    pub stage: String,
    /// Runs `succeeded`.
    pub succeeded: i64,
    /// Runs `failed`.
    pub failed: i64,
    /// Runs `running`.
    pub running: i64,
}

/// One filing-year bucket. `year` is `extract(year from filing.filed_date)`;
/// a filing without a `filed_date` lands in the honest `year: null` bucket —
/// `external_id` is NEVER parsed for a year (its shape is regime-specific).
#[derive(Debug, Serialize, ToSchema)]
pub struct YearProgress {
    /// Filing year; `null` = filings whose `filed_date` is unknown.
    pub year: Option<i32>,
    /// Filings filed that year.
    pub filings: i64,
    /// Distinct Bronze documents behind those filings.
    pub documents: i64,
    /// Gold records from those filings.
    pub gold_records: i64,
    /// Of those, still `unverified`.
    pub gold_unverified: i64,
    /// Distinct politicians with at least one filing that year.
    pub politicians_with_filings: i64,
    /// Roster denominator: distinct politicians holding a mandate on the
    /// regime's `(jurisdiction_id, body)` during that year (`mandate.body`
    /// values are written from the same adapter constants as
    /// `disclosure_regime.body`, so the join is exact). `null` for the
    /// unknown-year bucket.
    pub roster_members: Option<i64>,
}

/// Backfill progress of one regime.
#[derive(Debug, Serialize, ToSchema)]
pub struct RegimeBackfill {
    /// Adapter regime code (`us_house`, `br`, ...); `null` when no static or
    /// publish-derived mapping exists for the regime row (module docs).
    pub regime_code: Option<String>,
    /// Seeded `disclosure_regime.id`. Deliberately un-patterned:
    /// `disclosure_regime.id` is unconstrained text (registry- and
    /// adapter-seeded ids both exist), so publishing a ULID pattern here
    /// would be a lie and `.*` would be dead weight.
    pub regime_id: String,
    /// Owning `jurisdiction.id`.
    pub jurisdiction_id: String,
    /// Whole-history totals.
    pub totals: RegimeTotals,
    /// Per-stage run progress for the regime's adapter code (shared across
    /// regime rows of a multi-body adapter, e.g. both br chambers).
    pub stages: Vec<StageProgress>,
    /// Year-by-year progress, oldest first; the `null` year bucket last.
    pub years: Vec<YearProgress>,
}

/// Backfill progress across every regime that has at least one filing.
#[derive(Debug, Serialize, ToSchema)]
pub struct BackfillProgress {
    /// Per-regime progress, ordered by regime id.
    pub regimes: Vec<RegimeBackfill>,
}

/// Query parameters of `GET /v1/admin/ops/backfill`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BackfillParams {
    /// Adapter regime-code filter, e.g. `us_house` (matches the resolved
    /// `regime_code`; a multi-body adapter returns all its regime rows).
    pub adapter: Option<String>,
}

const BACKFILL_REGIMES_SQL: &str = "select r.id as regime_id, r.jurisdiction_id, r.details \
 from disclosure_regime r \
 where exists (select 1 from filing f where f.regime_id = r.id) \
 order by r.id";

const BACKFILL_FILING_TOTALS_SQL: &str = "select regime_id, \
   count(distinct raw_document_id) as bronze_documents, count(*) as filings \
 from filing group by regime_id";

const BACKFILL_GOLD_TOTALS_SQL: &str = "select regime_id, count(*) as gold_records, \
   count(*) filter (where verification_state = 'unverified') as gold_unverified \
 from disclosure_record group by regime_id";

/// Silver rows attributed per document (see [`RegimeTotals::silver_rows`]).
/// The `stg_%` union is mechanically gated together with [`TOTALS_SQL`]
/// (`tests/ops.rs` `silver_union_covers_every_stg_table`) — `pub` for that
/// gate.
pub const BACKFILL_SILVER_SQL: &str = "select fr.regime_id, sum(s.silver_rows)::bigint as silver_rows \
 from ( \
   select raw_document_id, count(*) as silver_rows from stg_us_house group by raw_document_id \
   union all \
   select raw_document_id, count(*) as silver_rows from stg_br group by raw_document_id \
 ) s \
 join (select distinct raw_document_id, regime_id from filing) fr \
   using (raw_document_id) \
 group by fr.regime_id";

/// Review-task attribution, kind 1 of 3: `disclosure_record` targets join
/// Gold for their `regime_id`.
const BACKFILL_REVIEW_RECORD_SQL: &str = "select r.regime_id, count(*) as review_open \
 from review_task rt \
 join disclosure_record r on r.id = rt.target_id \
 where rt.status = 'open' and rt.target_kind = 'disclosure_record' \
 group by r.regime_id";

/// Kind 2: `filing` targets carrying a bare filing id (publish-suppression
/// tasks, `publish.rs::insert_filing_review_task`) join `filing`.
const BACKFILL_REVIEW_FILING_SQL: &str = "select f.regime_id, count(*) as review_open \
 from review_task rt \
 join filing f on f.id = rt.target_id \
 where rt.status = 'open' and rt.target_kind = 'filing' \
 group by f.regime_id";

/// Kind 3: code-keyed targets — `regime` targets ARE the code
/// (`publish_blocked_frozen`), and `filing` targets in the
/// `<code>:<external_id>` shape (`run.rs:534` `unresolved_filer`) yield it
/// via `split_part`.
const BACKFILL_REVIEW_CODE_SQL: &str = "select case when rt.target_kind = 'regime' then rt.target_id \
             else split_part(rt.target_id, ':', 1) end as code, \
        count(*) as review_open \
 from review_task rt \
 where rt.status = 'open' \
   and (rt.target_kind = 'regime' \
        or (rt.target_kind = 'filing' and position(':' in rt.target_id) > 0)) \
 group by 1";

const BACKFILL_STAGES_SQL: &str = "select adapter, stage, \
   count(*) filter (where status = 'succeeded') as succeeded, \
   count(*) filter (where status = 'failed') as failed, \
   count(*) filter (where status = 'running') as running \
 from pipeline_run group by adapter, stage order by adapter, stage";

/// Year bucketing on `filed_date` ONLY (verified: both live bindings always
/// populate it; `published_at` is never written) — `null` year is the honest
/// unknown bucket.
const BACKFILL_YEARS_SQL: &str = "select regime_id, extract(year from filed_date)::int as year, \
   count(*) as filings, count(distinct raw_document_id) as documents, \
   count(distinct politician_id) as politicians_with_filings \
 from filing group by regime_id, 2";

const BACKFILL_YEAR_GOLD_SQL: &str = "select f.regime_id, extract(year from f.filed_date)::int as year, \
   count(*) as gold_records, \
   count(*) filter (where r.verification_state = 'unverified') as gold_unverified \
 from disclosure_record r \
 join filing f on f.id = r.filing_id \
 group by f.regime_id, 2";

/// Roster denominator: `disclosure_regime ⋈ mandate` on `(jurisdiction_id,
/// body)`, expanded over each mandate's active years (open-ended mandates run
/// through the current year).
const BACKFILL_ROSTER_SQL: &str = "select r.id as regime_id, y.year, \
   count(distinct m.politician_id) as roster_members \
 from disclosure_regime r \
 join mandate m on m.jurisdiction_id = r.jurisdiction_id and m.body = r.body \
 join lateral generate_series( \
        extract(year from m.start_date)::int, \
        coalesce(extract(year from m.end_date)::int, extract(year from now())::int) \
      ) as y(year) on true \
 group by r.id, y.year";

/// Data-derived adapter-code ↔ regime-ULID linkage: publish runs record their
/// filing in `stats.filing_id` (`PublishStats`), and the filing knows its
/// regime. This is how adapter-seeded regimes (`br`) resolve their code.
const BACKFILL_ADAPTER_REGIME_SQL: &str = "select distinct pr.adapter, f.regime_id \
 from pipeline_run pr \
 join filing f on f.id = pr.stats->>'filing_id' \
 where pr.stage = 'publish'";

#[derive(Debug, sqlx::FromRow)]
struct BackfillRegimeRow {
    regime_id: String,
    jurisdiction_id: String,
    details: serde_json::Value,
}

#[derive(Debug, sqlx::FromRow)]
struct FilingTotalsRow {
    regime_id: String,
    bronze_documents: i64,
    filings: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct GoldTotalsRow {
    regime_id: String,
    gold_records: i64,
    gold_unverified: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct StageRow {
    adapter: String,
    stage: String,
    succeeded: i64,
    failed: i64,
    running: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct YearRow {
    regime_id: String,
    year: Option<i32>,
    filings: i64,
    documents: i64,
    politicians_with_filings: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct YearGoldRow {
    regime_id: String,
    year: Option<i32>,
    gold_records: i64,
    gold_unverified: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct RosterYearRow {
    regime_id: String,
    year: i32,
    roster_members: i64,
}

/// Everything the backfill assembly needs, fetched in one pass.
struct BackfillData {
    regimes: Vec<BackfillRegimeRow>,
    filing_totals: HashMap<String, (i64, i64)>,
    gold_totals: HashMap<String, (i64, i64)>,
    silver: HashMap<String, i64>,
    review_by_regime: HashMap<String, i64>,
    review_by_code: HashMap<String, i64>,
    stages: Vec<StageRow>,
    years: Vec<YearRow>,
    year_gold: HashMap<(String, Option<i32>), (i64, i64)>,
    roster: HashMap<(String, i32), i64>,
    adapter_regime: Vec<(String, String)>,
}

async fn fetch_backfill_data(state: &AppState) -> Result<BackfillData, ApiError> {
    let regimes: Vec<BackfillRegimeRow> = sqlx::query_as(BACKFILL_REGIMES_SQL)
        .fetch_all(&state.pool)
        .await?;
    let filing_totals: Vec<FilingTotalsRow> = sqlx::query_as(BACKFILL_FILING_TOTALS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let gold_totals: Vec<GoldTotalsRow> = sqlx::query_as(BACKFILL_GOLD_TOTALS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let silver: Vec<(String, i64)> = sqlx::query_as(BACKFILL_SILVER_SQL)
        .fetch_all(&state.pool)
        .await?;
    let review_record: Vec<(String, i64)> = sqlx::query_as(BACKFILL_REVIEW_RECORD_SQL)
        .fetch_all(&state.pool)
        .await?;
    let review_filing: Vec<(String, i64)> = sqlx::query_as(BACKFILL_REVIEW_FILING_SQL)
        .fetch_all(&state.pool)
        .await?;
    let review_code: Vec<(String, i64)> = sqlx::query_as(BACKFILL_REVIEW_CODE_SQL)
        .fetch_all(&state.pool)
        .await?;
    let stages: Vec<StageRow> = sqlx::query_as(BACKFILL_STAGES_SQL)
        .fetch_all(&state.pool)
        .await?;
    let years: Vec<YearRow> = sqlx::query_as(BACKFILL_YEARS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let year_gold: Vec<YearGoldRow> = sqlx::query_as(BACKFILL_YEAR_GOLD_SQL)
        .fetch_all(&state.pool)
        .await?;
    let roster: Vec<RosterYearRow> = sqlx::query_as(BACKFILL_ROSTER_SQL)
        .fetch_all(&state.pool)
        .await?;
    let adapter_regime: Vec<(String, String)> = sqlx::query_as(BACKFILL_ADAPTER_REGIME_SQL)
        .fetch_all(&state.pool)
        .await?;

    let mut review_by_regime: HashMap<String, i64> = HashMap::new();
    for (regime_id, open) in review_record.into_iter().chain(review_filing) {
        *review_by_regime.entry(regime_id).or_insert(0) += open;
    }
    Ok(BackfillData {
        regimes,
        filing_totals: filing_totals
            .into_iter()
            .map(|r| (r.regime_id, (r.bronze_documents, r.filings)))
            .collect(),
        gold_totals: gold_totals
            .into_iter()
            .map(|r| (r.regime_id, (r.gold_records, r.gold_unverified)))
            .collect(),
        silver: silver.into_iter().collect(),
        review_by_regime,
        review_by_code: review_code.into_iter().collect(),
        stages,
        years,
        year_gold: year_gold
            .into_iter()
            .map(|r| ((r.regime_id, r.year), (r.gold_records, r.gold_unverified)))
            .collect(),
        roster: roster
            .into_iter()
            .map(|r| ((r.regime_id, r.year), r.roster_members))
            .collect(),
        adapter_regime,
    })
}

/// Assembles one regime's backfill entry from the fetched maps.
fn assemble_regime(data: &BackfillData, row: &BackfillRegimeRow) -> RegimeBackfill {
    let regime_code = static_regime_code(&row.regime_id, &row.details).or_else(|| {
        data.adapter_regime
            .iter()
            .find(|(_, regime_id)| regime_id == &row.regime_id)
            .map(|(adapter, _)| adapter.clone())
    });
    let (bronze_documents, filings) = data
        .filing_totals
        .get(&row.regime_id)
        .copied()
        .unwrap_or((0, 0));
    let (gold_records, gold_unverified) = data
        .gold_totals
        .get(&row.regime_id)
        .copied()
        .unwrap_or((0, 0));
    let review_open = data
        .review_by_regime
        .get(&row.regime_id)
        .copied()
        .unwrap_or(0)
        + regime_code
            .as_ref()
            .and_then(|code| data.review_by_code.get(code))
            .copied()
            .unwrap_or(0);
    let stages = regime_code
        .as_ref()
        .map(|code| {
            data.stages
                .iter()
                .filter(|s| &s.adapter == code)
                .map(|s| StageProgress {
                    stage: s.stage.clone(),
                    succeeded: s.succeeded,
                    failed: s.failed,
                    running: s.running,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut years: Vec<YearProgress> = data
        .years
        .iter()
        .filter(|y| y.regime_id == row.regime_id)
        .map(|y| {
            let (gold_records, gold_unverified) = data
                .year_gold
                .get(&(row.regime_id.clone(), y.year))
                .copied()
                .unwrap_or((0, 0));
            let roster_members = y
                .year
                .and_then(|year| data.roster.get(&(row.regime_id.clone(), year)).copied());
            YearProgress {
                year: y.year,
                filings: y.filings,
                documents: y.documents,
                gold_records,
                gold_unverified,
                politicians_with_filings: y.politicians_with_filings,
                roster_members,
            }
        })
        .collect();
    // Oldest year first; the honest unknown-year bucket sorts last.
    years.sort_by_key(|y| (y.year.is_none(), y.year));
    RegimeBackfill {
        regime_code,
        regime_id: row.regime_id.clone(),
        jurisdiction_id: row.jurisdiction_id.clone(),
        totals: RegimeTotals {
            bronze_documents,
            silver_rows: data.silver.get(&row.regime_id).copied().unwrap_or(0),
            filings,
            gold_records,
            gold_unverified,
            review_open,
        },
        stages,
        years,
    }
}

/// Per-regime, per-year backfill progress: filings/documents/gold split by
/// filing year, politicians-vs-roster coverage, stage-level run outcomes.
///
/// # Errors
/// `401` outside the admin gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/backfill",
    tag = "ops",
    params(BackfillParams),
    responses(
        (status = 200, description = "Per-regime, per-year backfill progress", body = BackfillProgress),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn backfill(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<BackfillParams>,
) -> Result<Json<BackfillProgress>, ApiError> {
    let data = fetch_backfill_data(&state).await?;
    let regimes = data
        .regimes
        .iter()
        .map(|row| assemble_regime(&data, row))
        .filter(|entry| match &params.adapter {
            None => true,
            Some(adapter) => entry.regime_code.as_deref() == Some(adapter.as_str()),
        })
        .collect();
    Ok(Json(BackfillProgress { regimes }))
}

// ------------------------------------------------------------------ freezes --

/// One sentinel per-source baseline row (`sentinel_watch`, design §5.6/§5.8).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SentinelSource {
    /// Adapter regime code the sentinel watches.
    pub regime_code: String,
    /// Last observed HTTP status.
    pub last_status: Option<i32>,
    /// Last structural layout hash of the listing markup.
    pub last_layout_hash: Option<String>,
    /// Last discoverable filing count.
    pub last_count: Option<i64>,
    /// Last observed `ETag`.
    pub last_etag: Option<String>,
    /// Last observed `Last-Modified`.
    pub last_modified: Option<String>,
    /// Whether the regime's publication is currently frozen (fail closed).
    pub frozen: bool,
    /// The drift kind that froze it.
    pub frozen_kind: Option<String>,
    /// When it froze.
    pub frozen_at: Option<DateTime<Utc>>,
    /// When the sentinel last probed the source.
    pub last_checked_at: DateTime<Utc>,
}

/// One ranked drift anomaly (`drift_report`).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DriftReportEntry {
    /// Report ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Adapter regime code.
    pub regime_code: String,
    /// Anomaly kind, e.g. `layout_shift`.
    pub drift_kind: String,
    /// Severity rank — the orchestrator picks the worst first.
    pub priority_score: f32,
    /// Whether this anomaly froze publication.
    pub freezes_publication: bool,
    /// Dedup key (`regime_code:kind:signature`).
    pub dedup_key: String,
    /// Anomaly detail payload.
    #[schema(value_type = Object)]
    pub detail: serde_json::Value,
    /// `open` | `resolved` | `superseded`.
    pub status: String,
    /// The auto-filed review task, when one was opened.
    pub review_task_id: Option<String>,
    /// How many times the same open anomaly re-detected.
    pub detections: i32,
    /// First detection.
    pub first_detected_at: DateTime<Utc>,
    /// Most recent detection.
    pub last_detected_at: DateTime<Utc>,
}

/// Sentinel baselines + ranked drift reports.
#[derive(Debug, Serialize, ToSchema)]
pub struct FreezeStatus {
    /// Every watched source's baseline (freeze state included).
    pub sources: Vec<SentinelSource>,
    /// Drift reports, worst first.
    pub drift: Vec<DriftReportEntry>,
}

/// Query parameters of `GET /v1/admin/ops/freezes`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FreezesParams {
    /// Drift-report scope: `open` (default) | `all`.
    pub status: Option<String>,
}

const SENTINEL_SQL: &str = "select regime_code, last_status, last_layout_hash, last_count, \
        last_etag, last_modified, frozen, frozen_kind, frozen_at, last_checked_at \
 from sentinel_watch order by regime_code";

const DRIFT_SQL: &str = "select id, regime_code, drift_kind, priority_score, freezes_publication, \
        dedup_key, detail, status, review_task_id, detections, \
        first_detected_at, last_detected_at \
 from drift_report \
 where ($1 or status = 'open') \
 order by priority_score desc, id";

/// Sentinel freeze state + ranked drift reports (design §5.6/§5.8).
///
/// # Errors
/// `400` on a bad `status`; `401` outside the admin gate; `500` on backend
/// failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/freezes",
    tag = "ops",
    params(FreezesParams),
    responses(
        (status = 200, description = "Sentinel baselines + drift reports, worst first", body = FreezeStatus),
        (status = 400, description = "Malformed status", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn freezes(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<FreezesParams>,
) -> Result<Json<FreezeStatus>, ApiError> {
    let include_all = validate_freeze_scope(params.status)?;
    let sources: Vec<SentinelSource> = sqlx::query_as(SENTINEL_SQL).fetch_all(&state.pool).await?;
    let drift: Vec<DriftReportEntry> = sqlx::query_as(DRIFT_SQL)
        .bind(include_all)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(FreezeStatus { sources, drift }))
}

// ------------------------------------------------------------ review health --

/// Task counts per status.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ReviewStatusCounts {
    /// `open` tasks.
    pub open: i64,
    /// `resolved` tasks.
    pub resolved: i64,
    /// `dismissed` tasks.
    pub dismissed: i64,
}

/// One open-task rollup per reason.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct OpenReason {
    /// Task reason, e.g. `unresolved_filer`.
    pub reason: String,
    /// Open tasks with this reason.
    pub open: i64,
    /// Oldest open task's creation time.
    pub oldest_created_at: DateTime<Utc>,
    /// Highest priority among them.
    pub max_priority: f32,
}

/// Tasks resolved per UTC day (any terminal status — `resolved_at` is
/// stamped for dismissals too).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ResolvedDay {
    /// Day start (UTC).
    pub day: DateTime<Utc>,
    /// Tasks resolved that day.
    pub resolved: i64,
}

/// One month × regime slice of the sampling audit (design §7.4 precision).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SampleAuditSlice {
    /// Batch label, `YYYY-MM`.
    pub sample_month: String,
    /// Gold regime the sample was drawn from.
    pub regime_id: String,
    /// Drawn records awaiting a verdict.
    pub pending: i64,
    /// Confirmed correct.
    pub confirmed: i64,
    /// Found discrepant.
    pub discrepancy: i64,
}

/// Review-queue health: status counts, open-by-reason, 14-day resolution
/// series, and the monthly sampling-audit precision source.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewHealth {
    /// Task counts per status.
    pub by_status: ReviewStatusCounts,
    /// Open tasks rolled up by reason, largest first.
    pub open_by_reason: Vec<OpenReason>,
    /// Tasks resolved per UTC day over the trailing 14 days.
    pub resolved_by_day: Vec<ResolvedDay>,
    /// Creation time of the oldest still-open task.
    pub oldest_open_at: Option<DateTime<Utc>>,
    /// Sampling-audit slices, newest month first.
    pub sample_audit: Vec<SampleAuditSlice>,
}

const REVIEW_STATUS_SQL: &str = "select \
   count(*) filter (where status = 'open') as open, \
   count(*) filter (where status = 'resolved') as resolved, \
   count(*) filter (where status = 'dismissed') as dismissed \
 from review_task";

const REVIEW_REASONS_SQL: &str = "select reason, count(*) as open, min(created_at) as oldest_created_at, \
        max(priority_score) as max_priority \
 from review_task where status = 'open' \
 group by reason order by open desc, reason";

const REVIEW_RESOLVED_SQL: &str = "select date_trunc('day', resolved_at) as day, count(*) as resolved \
 from review_task \
 where resolved_at is not null and resolved_at >= now() - interval '14 days' \
 group by 1 order by 1";

const REVIEW_OLDEST_SQL: &str = "select min(created_at) from review_task where status = 'open'";

const SAMPLE_AUDIT_SQL: &str = "select sample_month, regime_id, \
   count(*) filter (where status = 'pending') as pending, \
   count(*) filter (where status = 'confirmed') as confirmed, \
   count(*) filter (where status = 'discrepancy') as discrepancy \
 from sample_audit \
 group by sample_month, regime_id \
 order by sample_month desc, regime_id";

/// Review-queue health (design §7.1–7.2, §7.4).
///
/// # Errors
/// `401` outside the admin gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/review-health",
    tag = "ops",
    responses(
        (status = 200, description = "Review-queue health + sampling-audit precision source", body = ReviewHealth),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn review_health(State(state): State<AppState>) -> Result<Json<ReviewHealth>, ApiError> {
    let by_status: ReviewStatusCounts = sqlx::query_as(REVIEW_STATUS_SQL)
        .fetch_one(&state.pool)
        .await?;
    let open_by_reason: Vec<OpenReason> = sqlx::query_as(REVIEW_REASONS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let resolved_by_day: Vec<ResolvedDay> = sqlx::query_as(REVIEW_RESOLVED_SQL)
        .fetch_all(&state.pool)
        .await?;
    let oldest_open_at: Option<DateTime<Utc>> = sqlx::query_scalar(REVIEW_OLDEST_SQL)
        .fetch_one(&state.pool)
        .await?;
    let sample_audit: Vec<SampleAuditSlice> = sqlx::query_as(SAMPLE_AUDIT_SQL)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(ReviewHealth {
        by_status,
        open_by_reason,
        resolved_by_day,
        oldest_open_at,
        sample_audit,
    }))
}

// --------------------------------------------------------------- deliveries --

/// Delivery counts per status (design §6.3 ledger).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DeliveryStatusCounts {
    /// Awaiting dispatch.
    pub pending: i64,
    /// Held for the digest window.
    pub pending_digest: i64,
    /// Delivered.
    pub sent: i64,
    /// Dead-lettered after exhausted retries.
    pub dead: i64,
}

/// One dead-lettered delivery.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DeadDelivery {
    /// Delivery ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// The alert rule it belonged to.
    pub alert_rule_id: String,
    /// `email` | `webhook`.
    pub channel: String,
    /// Attempts made before dead-lettering.
    pub attempts: i32,
    /// The final failure.
    pub last_error: Option<String>,
    /// When it dead-lettered.
    pub updated_at: DateTime<Utc>,
}

/// Alert-delivery health: ledger counts, 24h throughput, outbox backlog and
/// the recent DLQ.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeliveryHealth {
    /// Delivery counts per status.
    pub by_status: DeliveryStatusCounts,
    /// Deliveries sent in the trailing 24 hours.
    pub sent_24h: i64,
    /// Outbox events awaiting dispatch.
    pub outbox_undispatched: i64,
    /// Oldest undispatched event's creation time (backlog age).
    pub oldest_undispatched_at: Option<DateTime<Utc>>,
    /// The 20 most recently dead-lettered deliveries.
    pub dead_recent: Vec<DeadDelivery>,
}

#[derive(Debug, sqlx::FromRow)]
struct DeliveryHealthRow {
    pending: i64,
    pending_digest: i64,
    sent: i64,
    dead: i64,
    sent_24h: i64,
    outbox_undispatched: i64,
    oldest_undispatched_at: Option<DateTime<Utc>>,
}

const DELIVERY_HEALTH_SQL: &str = "select \
   (select count(*) from delivery where status = 'pending') as pending, \
   (select count(*) from delivery where status = 'pending_digest') as pending_digest, \
   (select count(*) from delivery where status = 'sent') as sent, \
   (select count(*) from delivery where status = 'dead') as dead, \
   (select count(*) from delivery \
     where status = 'sent' and updated_at >= now() - interval '24 hours') as sent_24h, \
   (select count(*) from outbox_event where dispatched_at is null) as outbox_undispatched, \
   (select min(created_at) from outbox_event where dispatched_at is null) \
     as oldest_undispatched_at";

const DEAD_RECENT_SQL: &str = "select id, alert_rule_id, channel, attempts, last_error, updated_at \
 from delivery where status = 'dead' \
 order by updated_at desc limit 20";

/// Alert-delivery health (design §6.3): ledger counts, outbox backlog, DLQ.
///
/// # Errors
/// `401` outside the admin gate; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/deliveries",
    tag = "ops",
    responses(
        (status = 200, description = "Delivery ledger counts, outbox backlog and recent DLQ", body = DeliveryHealth),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn deliveries(State(state): State<AppState>) -> Result<Json<DeliveryHealth>, ApiError> {
    let row: DeliveryHealthRow = sqlx::query_as(DELIVERY_HEALTH_SQL)
        .fetch_one(&state.pool)
        .await?;
    let dead_recent: Vec<DeadDelivery> = sqlx::query_as(DEAD_RECENT_SQL)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(DeliveryHealth {
        by_status: DeliveryStatusCounts {
            pending: row.pending,
            pending_digest: row.pending_digest,
            sent: row.sent,
            dead: row.dead,
        },
        sent_24h: row.sent_24h,
        outbox_undispatched: row.outbox_undispatched,
        oldest_undispatched_at: row.oldest_undispatched_at,
        dead_recent,
    }))
}

// --------------------------------------------------------- extraction costs --

/// One month of LLM extraction spend.
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtractionCostMonth {
    /// Month label, `YYYY-MM` (UTC).
    pub month: String,
    /// Input tokens consumed.
    pub tokens_in: i64,
    /// Output tokens produced.
    pub tokens_out: i64,
    /// Estimated spend, decimal string (invariant 7).
    pub estimated_cost_usd: String,
    /// Parse-stage runs that recorded a `stats.extraction` block.
    pub extraction_runs: i64,
    /// Extraction-cache entries created that month.
    pub cache_entries_created: i64,
}

/// Monthly LLM extraction spend vs the HARD CAP.
///
/// COST INTERFACE CONTRACT with goal 021 Phase 2: parse-stage
/// `pipeline_run.stats.extraction` is expected to carry
/// `{tokens_in, tokens_out, estimated_cost_usd (decimal string), passes}`.
/// Nothing writes that block yet — every month reports zeros until 021 lands
/// it (the SQL is null-tolerant by design). 021 owners must adopt these key
/// names or update [`EXTRACTION_COSTS_SQL`].
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtractionCostReport {
    /// The monthly HARD CAP, decimal string — always `"200.00"`.
    pub hard_cap_usd: String,
    /// Months, oldest first — every requested month present, zeros when
    /// nothing was spent.
    pub months: Vec<ExtractionCostMonth>,
}

/// Query parameters of `GET /v1/admin/ops/extraction-costs`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExtractionCostsParams {
    /// Trailing months to report (current month included), `1..=24`;
    /// defaults to 3.
    #[param(minimum = 1, maximum = 24)]
    pub months: Option<u32>,
}

/// `generate_series` guarantees a row per requested month; the LEFT JOINs
/// keep absent data at NULL (mapped to zeros), and regex-guarded casts keep
/// malformed stats from 500-ing the surface.
const EXTRACTION_COSTS_SQL: &str = "with months as ( \
   select generate_series( \
            date_trunc('month', now()) - make_interval(months => $1 - 1), \
            date_trunc('month', now()), \
            interval '1 month') as month \
 ), parse_runs as ( \
   select date_trunc('month', started_at) as month, \
     sum(case when stats->'extraction'->>'tokens_in' ~ '^[0-9]+$' \
              then (stats->'extraction'->>'tokens_in')::bigint else 0 end)::bigint as tokens_in, \
     sum(case when stats->'extraction'->>'tokens_out' ~ '^[0-9]+$' \
              then (stats->'extraction'->>'tokens_out')::bigint else 0 end)::bigint as tokens_out, \
     sum(case when stats->'extraction'->>'estimated_cost_usd' ~ '^[0-9]+(\\.[0-9]+)?$' \
              then (stats->'extraction'->>'estimated_cost_usd')::numeric else 0 end) \
       as estimated_cost_usd, \
     count(*) filter (where (stats -> 'extraction') is not null) as extraction_runs \
   from pipeline_run where stage = 'parse' group by 1 \
 ), cache as ( \
   select date_trunc('month', created_at) as month, count(*) as cache_entries_created \
   from extraction_cache group by 1 \
 ) \
 select m.month, p.tokens_in, p.tokens_out, p.estimated_cost_usd, p.extraction_runs, \
        c.cache_entries_created \
 from months m \
 left join parse_runs p using (month) \
 left join cache c using (month) \
 order by m.month";

#[derive(Debug, sqlx::FromRow)]
struct CostMonthRow {
    month: DateTime<Utc>,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    estimated_cost_usd: Option<Decimal>,
    extraction_runs: Option<i64>,
    cache_entries_created: Option<i64>,
}

/// Monthly LLM extraction cost rollup vs the USD 200.00 HARD CAP.
///
/// # Errors
/// `400` on out-of-range `months`; `401` outside the admin gate; `500` on
/// backend failure.
#[utoipa::path(
    get,
    path = "/v1/admin/ops/extraction-costs",
    tag = "ops",
    params(ExtractionCostsParams),
    responses(
        (status = 200, description = "Monthly extraction spend vs the HARD CAP", body = ExtractionCostReport),
        (status = 400, description = "Malformed months", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn extraction_costs(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ExtractionCostsParams>,
) -> Result<Json<ExtractionCostReport>, ApiError> {
    let months = validate_months(params.months)?;
    let rows: Vec<CostMonthRow> = sqlx::query_as(EXTRACTION_COSTS_SQL)
        .bind(months)
        .fetch_all(&state.pool)
        .await?;
    let months = rows
        .into_iter()
        .map(|row| ExtractionCostMonth {
            month: row.month.format("%Y-%m").to_string(),
            tokens_in: row.tokens_in.unwrap_or(0),
            tokens_out: row.tokens_out.unwrap_or(0),
            estimated_cost_usd: usd(row.estimated_cost_usd.unwrap_or(Decimal::ZERO)),
            extraction_runs: row.extraction_runs.unwrap_or(0),
            cache_entries_created: row.cache_entries_created.unwrap_or(0),
        })
        .collect();
    Ok(Json(ExtractionCostReport {
        hard_cap_usd: usd(LLM_MONTHLY_HARD_CAP),
        months,
    }))
}

// -------------------------------------------------------------------- tests --

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn hard_cap_serializes_exactly_200_00() {
        // Invariant 7: money is a decimal STRING, and the founder-set cap
        // (2026-07-08, goal 021 Phase 2) is exactly USD 200.00.
        assert_eq!(usd(LLM_MONTHLY_HARD_CAP), "200.00");
        assert_eq!(
            serde_json::to_string(&LLM_MONTHLY_HARD_CAP).unwrap(),
            "\"200.00\""
        );
    }

    #[test]
    fn usd_always_carries_two_fraction_digits() {
        assert_eq!(usd(Decimal::ZERO), "0.00");
        assert_eq!(usd(Decimal::new(1284, 2)), "12.84");
        assert_eq!(usd(Decimal::new(5, 1)), "0.50");
        assert_eq!(usd(Decimal::new(123_456, 4)), "12.35", "rounds to cents");
    }

    #[test]
    fn extraction_month_reports_cap_utilization() {
        let now = Utc::now();
        let block = extraction_month(
            now,
            &ExtractionMonthRow {
                tokens_in: 10,
                tokens_out: 20,
                estimated_cost_usd: Decimal::new(1284, 2), // 12.84
            },
        );
        assert_eq!(block.month, now.format("%Y-%m").to_string());
        assert_eq!(block.estimated_cost_usd, "12.84");
        assert_eq!(block.hard_cap_usd, "200.00");
        assert!((block.cap_utilization_pct - 6.42).abs() < 1e-9);
    }

    #[test]
    fn run_status_rejects_unknown_tokens() {
        assert_eq!(validate_run_status(None).unwrap(), None);
        for ok in ["running", "succeeded", "failed"] {
            assert_eq!(
                validate_run_status(Some(ok.to_owned())).unwrap().as_deref(),
                Some(ok)
            );
        }
        assert!(validate_run_status(Some("done".to_owned())).is_err());
    }

    #[test]
    fn hours_bounds_are_1_to_720_default_24() {
        assert_eq!(validate_hours(None).unwrap(), 24);
        assert_eq!(validate_hours(Some(1)).unwrap(), 1);
        assert_eq!(validate_hours(Some(720)).unwrap(), 720);
        assert!(validate_hours(Some(0)).is_err());
        assert!(validate_hours(Some(721)).is_err());
    }

    #[test]
    fn bucket_is_hour_or_day_default_hour() {
        assert_eq!(validate_bucket(None).unwrap(), "hour");
        assert_eq!(validate_bucket(Some("day".to_owned())).unwrap(), "day");
        assert!(validate_bucket(Some("week".to_owned())).is_err());
    }

    #[test]
    fn months_bounds_are_1_to_24_default_3() {
        assert_eq!(validate_months(None).unwrap(), 3);
        assert_eq!(validate_months(Some(1)).unwrap(), 1);
        assert_eq!(validate_months(Some(24)).unwrap(), 24);
        assert!(validate_months(Some(0)).is_err());
        assert!(validate_months(Some(25)).is_err());
    }

    #[test]
    fn freeze_scope_is_open_or_all_default_open() {
        assert!(!validate_freeze_scope(None).unwrap());
        assert!(!validate_freeze_scope(Some("open".to_owned())).unwrap());
        assert!(validate_freeze_scope(Some("all".to_owned())).unwrap());
        assert!(validate_freeze_scope(Some("frozen".to_owned())).is_err());
    }

    #[test]
    fn static_regime_code_prefers_details_then_core_seed() {
        // Registry-seeded live regime: details carry the code.
        let details = serde_json::json!({ "regime_code": "us_house" });
        assert_eq!(
            static_regime_code("whatever", &details).as_deref(),
            Some("us_house")
        );
        // Adapter-pinned live regime id without details resolves via the
        // core seed consts.
        let empty = serde_json::json!({});
        assert_eq!(
            static_regime_code("0HSEREG0000000000000000001", &empty).as_deref(),
            Some("us_house")
        );
        // Adapter-seeded regime (br) is unknown statically — the publish
        // linkage resolves it at query time.
        assert_eq!(
            static_regime_code("0BRAREG0000000000000000001", &empty),
            None
        );
    }
}
