//! `GET /v1/admin/backfill` — section B, backfill & ingestion: `backfill_run`
//! rows (B1; pre-migration history is log-only and the page says so),
//! historical completion vs declared targets (B2), per-source freshness +
//! filing lag percentiles (B3), the politeness proxy (B4), queue depths (B5).
//!
//! HONEST FALLBACKS, spelled out on the wire: `backfill_run` bookkeeping
//! starts at migration 0011 (`history_note`); target years are DECLARED v1
//! targets, not discovered facts (`targets_note`); no fetch-log table exists,
//! so fetch density is a `raw_document.fetched_at` proxy
//! (`fetch_density_note`); Cloud Tasks depths are a static note v1
//! (`cloud_tasks_note`).

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use const_format::concatcp;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use utoipa::ToSchema;

use super::BRIDGE_CTE;
use super::overview::{AdminQueueDepths, fetch_queue_depths};
use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// -------------------------------------------------------- declared targets --

/// Declared v1 target years for the US congressional archives (plan B2).
/// These are DECLARED TARGETS — what goals 080/081 set out to cover — not
/// discovered facts; completion is measured against them honestly.
const US_TARGET_YEARS: std::ops::RangeInclusive<i32> = 2012..=2026;

/// Declared v1 target cycles for `br` federal elections (plan B2): the
/// minimum set per data present (2022 nationwide proven; 2014/2018 declared).
const BR_TARGET_YEARS: [i32; 3] = [2014, 2018, 2022];

/// The declared v1 target years for a regime; empty for regimes with no
/// declared target (the page shows an empty list, never a guessed one).
fn declared_target_years(regime_code: &str) -> Vec<i32> {
    match regime_code {
        "us_house" | "us_senate" => US_TARGET_YEARS.collect(),
        "br" => BR_TARGET_YEARS.to_vec(),
        _ => Vec::new(),
    }
}

/// Regimes that always get a completion row, even before any data lands —
/// the "what's left" view must show the declared targets from day zero.
const DECLARED_REGIMES: [&str; 3] = ["us_house", "us_senate", "br"];

// ------------------------------------------------------------ page captions --

/// B1 honesty caption: no backdated rows are fabricated.
const HISTORY_NOTE: &str = "backfill_run bookkeeping begins at migration 0011; \
     earlier backfills exist only in worker logs — no backdated rows are fabricated.";

/// B2 honesty caption: targets are declared, not discovered.
const TARGETS_NOTE: &str = "target_years are DECLARED v1 targets: us_house/us_senate \
     annual archives 2012..=2026; br federal election cycles 2014, 2018, 2022 (minimum, \
     per data present). Regimes without a declared target show an empty target list.";

/// B4 honesty caption: density is a proxy, not a fetch log.
const FETCH_DENSITY_NOTE: &str = "No fetch-log table exists; density is an honest proxy \
     over raw_document.fetched_at joined to its fetch pipeline_run, last 48 hours by hour.";

/// B5 Cloud Tasks caption: static note in v1, no live poll.
const CLOUD_TASKS_NOTE: &str = "Cloud Tasks queue depths are not surfaced in v1 — \
     inspect with `gcloud tasks queues list` (static note, no live poll, no faked numbers).";

// ------------------------------------------------------------- wire shapes --

/// One `backfill_run` row (migration 0011): what a worker bin did for one
/// year — status, counters, and what the budget gate decided.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminBackfillRun {
    /// Run ULID (time-sortable).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Adapter regime code, e.g. `us_house`, `br`.
    pub regime_code: String,
    /// Archive / election year the run covered.
    pub year: i32,
    /// `backfill` | `seed`.
    pub kind: String,
    /// Worker bin that wrote the row, e.g. `backfill-real-br`.
    pub bin: String,
    /// Optional scope note, e.g. `--uf SP` or `nationwide`.
    pub scope: Option<String>,
    /// `succeeded` | `skipped_budget` | `failed`.
    pub status: String,
    /// Filings seen for the year.
    pub filings: i64,
    /// Filings published (adds + supersessions + changes).
    pub published: i64,
    /// Already-published filings left untouched (invariant-4 evidence).
    pub replayed: i64,
    /// Gold rows actually inserted.
    pub gold_inserted: i64,
    /// `outbox_event` rows written.
    pub outbox_written: i64,
    /// `review_task` rows opened.
    pub review_tasks: i64,
    /// Per-filing failures (the year continued).
    pub failed_count: i64,
    /// Dry-run Gold-row delta the budget gate compared.
    pub record_delta: i64,
    /// `BACKFILL_BUDGET` in force; `null` when no budget gate applied.
    pub budget: Option<i64>,
    /// Year-level error, when `status = 'failed'`.
    pub error: Option<String>,
    /// When the year's processing began.
    pub started_at: DateTime<Utc>,
    /// When the row was written.
    pub finished_at: DateTime<Utc>,
}

/// Historical completion for one regime (plan B2): what the data says vs the
/// declared v1 target.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminRegimeCompletion {
    /// Adapter regime code.
    pub regime_code: String,
    /// Declared v1 target years (see `targets_note`); empty = no declared
    /// target for this regime.
    pub target_years: Vec<i32>,
    /// Years with at least one filing on record (by `filing.filed_date` via
    /// the provenance bridge; filings without a `filed_date` are not counted).
    pub years_with_data: Vec<i32>,
    /// Years with at least one succeeded `backfill_run` (kind `backfill`);
    /// pre-0011 history is log-only, so early years may show data without a
    /// run row.
    pub years_succeeded: Vec<i32>,
    /// Target years with no filing data yet — the "what's left" list.
    pub missing_years: Vec<i32>,
}

/// Freshness of one source (plan B3): sentinel check, last fetch, last
/// discovery, and the filing lag percentiles.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminRegimeFreshness {
    /// Adapter regime code.
    pub regime_code: String,
    /// `sentinel_watch.last_checked_at`; `null` = the sentinel has never
    /// probed this source.
    pub sentinel_last_checked_at: Option<DateTime<Utc>>,
    /// Latest `raw_document.fetched_at` for this source's fetch runs.
    pub last_fetched_at: Option<DateTime<Utc>>,
    /// Latest `filing.discovered_at` (our latency, honestly).
    pub last_discovered_at: Option<DateTime<Utc>>,
    /// p50 of `discovered_at - published_at` in seconds; `null` when no
    /// filing carries a `published_at`.
    pub lag_p50_seconds: Option<f64>,
    /// p90 of `discovered_at - published_at` in seconds; `null` when no
    /// filing carries a `published_at`.
    pub lag_p90_seconds: Option<f64>,
}

/// One (regime, hour) fetch-density bucket (plan B4 politeness proxy).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminFetchDensityBucket {
    /// Adapter regime code.
    pub regime_code: String,
    /// Hour bucket start (UTC, `date_trunc('hour', fetched_at)`).
    pub hour_start: DateTime<Utc>,
    /// Documents fetched in the bucket.
    pub fetched: i64,
}

/// Section B in one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBackfill {
    /// When this snapshot was assembled (server clock, UTC).
    pub generated_at: DateTime<Utc>,
    /// B1 honesty caption: pre-0011 history is log-only.
    pub history_note: String,
    /// Last 100 `backfill_run` rows, newest first (B1).
    pub runs: Vec<AdminBackfillRun>,
    /// B2 honesty caption: targets are declared, not discovered.
    pub targets_note: String,
    /// Historical completion per regime vs declared targets (B2).
    pub completion: Vec<AdminRegimeCompletion>,
    /// Freshness + filing lag percentiles per source (B3).
    pub freshness: Vec<AdminRegimeFreshness>,
    /// B4 honesty caption: density is a proxy, not a fetch log.
    pub fetch_density_note: String,
    /// Fetch density, last 48h by hour per source (B4).
    pub fetch_density: Vec<AdminFetchDensityBucket>,
    /// Queue depths (B5) — same shape as `/v1/admin/overview`.
    pub queue_depths: AdminQueueDepths,
    /// B5 Cloud Tasks caption: static note in v1.
    pub cloud_tasks_note: String,
}

// -------------------------------------------------------------------- SQL --

const RUNS_SQL: &str = "select id, regime_code, year, kind, bin, scope, status, \
     filings, published, replayed, gold_inserted, outbox_written, review_tasks, \
     failed_count, record_delta, budget, error, started_at, finished_at \
     from backfill_run order by finished_at desc, id desc limit 100";

/// Years with at least one filing, per regime, via the provenance bridge
/// (`regime_id` → `regime_code`). Filings without a `filed_date` cannot be
/// assigned a year and are honestly excluded (never guessed).
const DATA_YEARS_SQL: &str = concatcp!(
    BRIDGE_CTE,
    " select b.regime_code, extract(year from f.filed_date)::int as year \
     from filing f \
     join bridge b on b.regime_id = f.regime_id \
     where f.filed_date is not null \
     group by 1, 2 order by 1, 2"
);

const SUCCEEDED_YEARS_SQL: &str = "select regime_code, year from backfill_run \
     where kind = 'backfill' and status = 'succeeded' \
     group by regime_code, year order by regime_code, year";

/// Per-source freshness (B3): filing stats via the bridge, fetch stats via
/// `raw_document` → its fetch `pipeline_run`, sentinel state — FULL OUTER
/// joined so a source missing from any leg still surfaces (with `null`s,
/// never dropped). `percentile_cont` ignores rows whose lag is `null`
/// (filings without a `published_at`).
const FRESHNESS_SQL: &str = concatcp!(
    BRIDGE_CTE,
    ", filing_stats as ( \
       select b.regime_code, \
              max(f.discovered_at) as last_discovered_at, \
              percentile_cont(0.5) within group \
                (order by extract(epoch from (f.discovered_at - f.published_at))::double precision) \
                as lag_p50_seconds, \
              percentile_cont(0.9) within group \
                (order by extract(epoch from (f.discovered_at - f.published_at))::double precision) \
                as lag_p90_seconds \
       from filing f \
       join bridge b on b.regime_id = f.regime_id \
       group by b.regime_code), \
     fetch_stats as ( \
       select pr.adapter as regime_code, max(rd.fetched_at) as last_fetched_at \
       from raw_document rd \
       join pipeline_run pr on pr.id = rd.fetch_run_id \
       group by pr.adapter) \
     select coalesce(fst.regime_code, fes.regime_code, sw.regime_code) as regime_code, \
            sw.last_checked_at as sentinel_last_checked_at, \
            fes.last_fetched_at, \
            fst.last_discovered_at, \
            fst.lag_p50_seconds, \
            fst.lag_p90_seconds \
     from filing_stats fst \
     full outer join fetch_stats fes on fes.regime_code = fst.regime_code \
     full outer join sentinel_watch sw \
       on sw.regime_code = coalesce(fst.regime_code, fes.regime_code) \
     order by 1"
);

const DENSITY_SQL: &str = "select pr.adapter as regime_code, \
     date_trunc('hour', rd.fetched_at) as hour_start, \
     count(*) as fetched \
     from raw_document rd \
     join pipeline_run pr on pr.id = rd.fetch_run_id \
     where rd.fetched_at > now() - interval '48 hours' \
     group by 1, 2 order by 1, 2";

// ----------------------------------------------------------------- helpers --

/// Assembles B2: declared regimes always get a row; regimes discovered in
/// data or run history get one too (empty declared target, honest).
async fn fetch_completion(pool: &PgPool) -> Result<Vec<AdminRegimeCompletion>, ApiError> {
    let data_years: Vec<(String, i32)> = sqlx::query_as(DATA_YEARS_SQL).fetch_all(pool).await?;
    let succeeded_years: Vec<(String, i32)> =
        sqlx::query_as(SUCCEEDED_YEARS_SQL).fetch_all(pool).await?;

    let mut regimes: BTreeMap<String, (BTreeSet<i32>, BTreeSet<i32>)> = BTreeMap::new();
    for code in DECLARED_REGIMES {
        regimes.entry(code.to_owned()).or_default();
    }
    for (code, year) in data_years {
        regimes.entry(code).or_default().0.insert(year);
    }
    for (code, year) in succeeded_years {
        regimes.entry(code).or_default().1.insert(year);
    }
    Ok(regimes
        .into_iter()
        .map(|(regime_code, (with_data, succeeded))| {
            let target_years = declared_target_years(&regime_code);
            let missing_years = target_years
                .iter()
                .copied()
                .filter(|year| !with_data.contains(year))
                .collect();
            AdminRegimeCompletion {
                regime_code,
                target_years,
                years_with_data: with_data.into_iter().collect(),
                years_succeeded: succeeded.into_iter().collect(),
                missing_years,
            }
        })
        .collect())
}

// ------------------------------------------------------------ the handler --

/// Section B — backfill & ingestion: run history, completion vs declared
/// targets, per-source freshness + lag percentiles, the fetch-density
/// politeness proxy, and queue depths — one round trip.
///
/// # Errors
/// `401` without a valid `X-Admin-Token` (the gate wraps the whole admin
/// subtree); `500` on backend failure — all in the consistent envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/backfill",
    tag = "admin",
    responses(
        (status = 200, description = "The backfill & ingestion snapshot", body = AdminBackfill),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_backfill(
    State(state): State<AppState>,
) -> Result<Json<AdminBackfill>, ApiError> {
    let runs: Vec<AdminBackfillRun> = sqlx::query_as(RUNS_SQL).fetch_all(&state.pool).await?;
    let completion = fetch_completion(&state.pool).await?;
    let freshness: Vec<AdminRegimeFreshness> =
        sqlx::query_as(FRESHNESS_SQL).fetch_all(&state.pool).await?;
    let fetch_density: Vec<AdminFetchDensityBucket> =
        sqlx::query_as(DENSITY_SQL).fetch_all(&state.pool).await?;
    let queue_depths = fetch_queue_depths(&state.pool).await?;
    Ok(Json(AdminBackfill {
        generated_at: Utc::now(),
        history_note: HISTORY_NOTE.to_owned(),
        runs,
        targets_note: TARGETS_NOTE.to_owned(),
        completion,
        freshness,
        fetch_density_note: FETCH_DENSITY_NOTE.to_owned(),
        fetch_density,
        queue_depths,
        cloud_tasks_note: CLOUD_TASKS_NOTE.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        DATA_YEARS_SQL, DENSITY_SQL, FRESHNESS_SQL, RUNS_SQL, SUCCEEDED_YEARS_SQL,
        declared_target_years,
    };

    /// READ-ONLY BY CONTRACT: every admin statement is a `select` (or a
    /// bridge-CTE `with … select`).
    #[test]
    fn every_statement_is_read_only() {
        for sql in [
            RUNS_SQL,
            DATA_YEARS_SQL,
            SUCCEEDED_YEARS_SQL,
            FRESHNESS_SQL,
            DENSITY_SQL,
        ] {
            assert!(
                sql.starts_with("select ") || sql.starts_with("with bridge as ("),
                "not a select/with: {sql}"
            );
            for verb in ["insert ", "update ", "delete ", "truncate ", "drop "] {
                assert!(!sql.contains(verb), "write verb {verb:?} in: {sql}");
            }
        }
    }

    #[test]
    fn declared_targets_match_the_v1_plan() {
        assert_eq!(
            declared_target_years("us_house"),
            (2012..=2026).collect::<Vec<_>>()
        );
        assert_eq!(
            declared_target_years("us_senate"),
            (2012..=2026).collect::<Vec<_>>()
        );
        assert_eq!(declared_target_years("br"), vec![2014, 2018, 2022]);
        assert!(declared_target_years("uk_commons").is_empty());
    }
}
