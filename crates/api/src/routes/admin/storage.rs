//! `GET /v1/admin/storage` — section E, storage & tiers: Bronze doc counts
//! by mime and storage scheme (E1; no byte column exists → count-only,
//! stated), 30d Gold/filing growth (E2), Postgres size + top tables +
//! live/dead tuples from `pg_catalog` (E3).
//!
//! The `pg_catalog` statements behave identically on local Postgres and
//! Cloud SQL — this page needs zero changes at cloud cutover.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// ------------------------------------------------------------- wire shapes --

/// Bronze documents sharing one mime type (E1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminMimeCount {
    /// The stored `mime_type`.
    pub mime_type: String,
    /// Documents of this type.
    pub documents: i64,
}

/// Bronze documents sharing one storage scheme (E1). Doubles as the
/// cloud-migration progress readout (`gs` vs `local`).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminSchemeCount {
    /// URI scheme of `storage_uri` (e.g. `gs`); plain paths report `local`.
    pub scheme: String,
    /// Documents stored under this scheme.
    pub documents: i64,
}

/// Exact row count of one key table.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminTableRowCount {
    /// Table name.
    pub table_name: String,
    /// Exact `count(*)` at snapshot time.
    pub row_count: i64,
}

/// One day of Gold/filing growth (E2).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminGrowthDay {
    /// UTC calendar day.
    pub day: NaiveDate,
    /// Gold `disclosure_record` rows created that day (`created_at`).
    pub gold_records: i64,
    /// Filings discovered that day (`discovered_at` — `filing` has no
    /// `created_at` column; discovery time is the honest ingestion clock).
    pub filings: i64,
}

/// One of the 25 largest tables by total relation size (E3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminPgTable {
    /// Table name.
    pub table_name: String,
    /// `pg_total_relation_size` — heap + indexes + TOAST, in bytes.
    pub total_bytes: i64,
    /// `pg_stat_user_tables.n_live_tup` (statistics estimate).
    pub live_tuples: i64,
    /// `pg_stat_user_tables.n_dead_tup` (statistics estimate).
    pub dead_tuples: i64,
}

/// Postgres physical stats (E3), straight from `pg_catalog`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminPgStats {
    /// `pg_database_size(current_database())`, in bytes.
    pub database_size_bytes: i64,
    /// Top 25 tables by total relation size, largest first.
    pub top_tables: Vec<AdminPgTable>,
}

/// `GET /v1/admin/storage` — the full section-E payload, one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminStorage {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// E1: Bronze documents by mime type, most common first.
    pub bronze_by_mime: Vec<AdminMimeCount>,
    /// E1: Bronze documents by storage scheme, most common first.
    pub bronze_by_scheme: Vec<AdminSchemeCount>,
    /// E1 caveat rendered verbatim on the page.
    pub bronze_note: String,
    /// Exact row counts of every schema table, largest first.
    pub table_rows: Vec<AdminTableRowCount>,
    /// E2: per-day growth over the last 30 days (days with zero activity on
    /// both clocks are absent, not fabricated).
    pub growth_30d: Vec<AdminGrowthDay>,
    /// E3: Postgres physical stats.
    pub pg: AdminPgStats,
}

// -------------------------------------------------------------- statements --

const BRONZE_MIME_SQL: &str = "select mime_type, count(*) from raw_document \
     group by mime_type order by count(*) desc, mime_type";

/// Scheme = the part before `://`; a `storage_uri` without one is a plain
/// filesystem path and reports `local`. No forced gs/local binary — whatever
/// schemes actually exist are reported.
const BRONZE_SCHEME_SQL: &str = "select case when position('://' in storage_uri) > 0 \
        then split_part(storage_uri, '://', 1) else 'local' end, count(*) \
     from raw_document group by 1 order by count(*) desc, 1";

const BRONZE_NOTE: &str = "Counts only: raw_document stores no byte-size column, so Bronze \
     volume in bytes is unavailable (not estimated). The gs-vs-local scheme split doubles \
     as cloud-migration progress.";

/// Exact `count(*)` per key table — every table the migrations create. Exact
/// counts (not `n_live_tup` estimates) are affordable at current scale; if a
/// table crosses ~10x today's volume, swap its arm for the `pg_stat` estimate.
const TABLE_ROWS_SQL: &str = "select table_name, row_count from ( \
        select 'jurisdiction'::text as table_name, count(*)::bigint as row_count from jurisdiction \
        union all select 'disclosure_regime', count(*) from disclosure_regime \
        union all select 'politician', count(*) from politician \
        union all select 'politician_alias', count(*) from politician_alias \
        union all select 'mandate', count(*) from mandate \
        union all select 'instrument', count(*) from instrument \
        union all select 'instrument_alias', count(*) from instrument_alias \
        union all select 'raw_document', count(*) from raw_document \
        union all select 'filing', count(*) from filing \
        union all select 'disclosure_record', count(*) from disclosure_record \
        union all select 'review_task', count(*) from review_task \
        union all select 'pipeline_run', count(*) from pipeline_run \
        union all select 'outbox_event', count(*) from outbox_event \
        union all select 'stg_us_house', count(*) from stg_us_house \
        union all select 'stg_br', count(*) from stg_br \
        union all select 'stg_meta', count(*) from stg_meta \
        union all select 'extraction_cache', count(*) from extraction_cache \
        union all select 'alert_rule', count(*) from alert_rule \
        union all select 'delivery', count(*) from delivery \
        union all select 'review_audit', count(*) from review_audit \
        union all select 'user_account', count(*) from user_account \
        union all select 'api_key', count(*) from api_key \
        union all select 'usage_report', count(*) from usage_report \
        union all select 'usage_event', count(*) from usage_event \
        union all select 'subscription', count(*) from subscription \
        union all select 'sentinel_watch', count(*) from sentinel_watch \
        union all select 'drift_report', count(*) from drift_report \
        union all select 'sample_audit', count(*) from sample_audit \
        union all select 'backfill_run', count(*) from backfill_run \
     ) counts order by row_count desc, table_name";

const GOLD_GROWTH_SQL: &str = "select (created_at at time zone 'utc')::date, count(*) \
     from disclosure_record where created_at >= now() - interval '30 days' \
     group by 1 order by 1";

const FILING_GROWTH_SQL: &str = "select (discovered_at at time zone 'utc')::date, count(*) \
     from filing where discovered_at >= now() - interval '30 days' \
     group by 1 order by 1";

const DB_SIZE_SQL: &str = "select pg_database_size(current_database())";

/// `coalesce` because `pg_stat` columns are nullable in the view definition
/// (e.g. right after a stats reset), and a NULL would be a decode error, not
/// a zero.
const PG_TOP_TABLES_SQL: &str = "select relname::text, \
        coalesce(pg_total_relation_size(relid), 0), \
        coalesce(n_live_tup, 0), coalesce(n_dead_tup, 0) \
     from pg_stat_user_tables \
     order by pg_total_relation_size(relid) desc, relname limit 25";

// ---------------------------------------------------------------- assembly --

/// Merges the two per-day aggregates on their (UTC) day; a day missing from
/// one side contributes 0 on that side only.
async fn fetch_growth(state: &AppState) -> Result<Vec<AdminGrowthDay>, ApiError> {
    let gold: Vec<(NaiveDate, i64)> = sqlx::query_as(GOLD_GROWTH_SQL)
        .fetch_all(&state.pool)
        .await?;
    let filings: Vec<(NaiveDate, i64)> = sqlx::query_as(FILING_GROWTH_SQL)
        .fetch_all(&state.pool)
        .await?;
    let mut by_day: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    for (day, count) in gold {
        by_day.entry(day).or_default().0 = count;
    }
    for (day, count) in filings {
        by_day.entry(day).or_default().1 = count;
    }
    Ok(by_day
        .into_iter()
        .map(|(day, (gold_records, filings))| AdminGrowthDay {
            day,
            gold_records,
            filings,
        })
        .collect())
}

async fn fetch_pg_stats(state: &AppState) -> Result<AdminPgStats, ApiError> {
    let (database_size_bytes,): (i64,) = sqlx::query_as(DB_SIZE_SQL).fetch_one(&state.pool).await?;
    let top_tables = sqlx::query_as::<_, (String, i64, i64, i64)>(PG_TOP_TABLES_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(
            |(table_name, total_bytes, live_tuples, dead_tuples)| AdminPgTable {
                table_name,
                total_bytes,
                live_tuples,
                dead_tuples,
            },
        )
        .collect();
    Ok(AdminPgStats {
        database_size_bytes,
        top_tables,
    })
}

// ----------------------------------------------------------------- handler --

/// Section-E storage & tiers snapshot (admin dashboard `/admin/storage`).
///
/// # Errors
/// `401` without a valid `X-Admin-Token` (enforced by the route-layer gate);
/// `500` on backend failure — all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/storage",
    tag = "admin",
    responses(
        (status = 200, description = "Storage & tiers: Bronze counts by mime/scheme, exact table rows, 30d growth, Postgres physical stats", body = AdminStorage),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_storage(State(state): State<AppState>) -> Result<Json<AdminStorage>, ApiError> {
    let bronze_by_mime = sqlx::query_as::<_, (String, i64)>(BRONZE_MIME_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(mime_type, documents)| AdminMimeCount {
            mime_type,
            documents,
        })
        .collect();
    let bronze_by_scheme = sqlx::query_as::<_, (String, i64)>(BRONZE_SCHEME_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(scheme, documents)| AdminSchemeCount { scheme, documents })
        .collect();
    let table_rows = sqlx::query_as::<_, (String, i64)>(TABLE_ROWS_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(table_name, row_count)| AdminTableRowCount {
            table_name,
            row_count,
        })
        .collect();
    let growth_30d = fetch_growth(&state).await?;
    let pg = fetch_pg_stats(&state).await?;
    Ok(Json(AdminStorage {
        generated_at: Utc::now(),
        bronze_by_mime,
        bronze_by_scheme,
        bronze_note: BRONZE_NOTE.to_owned(),
        table_rows,
        growth_30d,
        pg,
    }))
}
