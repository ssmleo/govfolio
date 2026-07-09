//! `GET /v1/admin/coverage` — section A, coverage & inventory: jurisdiction
//! phase matrix (A1), per-regime windows (A2), Bronze→Silver→Gold tier
//! counts via the bridge CTE (A3), regime × year × `record_type` heatmap (A4),
//! gap analysis (A5), entity inventory (A6).
//!
//! FAN-OUT DISCIPLINE: every aggregate below runs as its own statement over
//! exactly one fact table (or one bridge join), then the rollup is assembled
//! in Rust. Joining `filing` × `disclosure_record` × `raw_document` in one
//! statement would multiply counts (one filing carries many records); split
//! queries make each count exact by construction.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, NaiveDate, Utc};
use const_format::concatcp;
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

use super::BRIDGE_CTE;

// ------------------------------------------------------------- wire shapes --

/// One `coverage_phase` bucket of the jurisdiction registry (A1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminPhaseCount {
    /// The §5.8 state-machine phase (`stub` … `live` | `blocked`).
    pub phase: String,
    /// Jurisdictions currently in this phase.
    pub jurisdictions: i64,
}

/// One blocked jurisdiction with its recorded reason (A1/A5).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBlockedJurisdiction {
    /// Jurisdiction id.
    pub jurisdiction_id: String,
    /// Jurisdiction display name.
    pub name: String,
    /// Why the factory blocked it; `null` when no reason was recorded.
    pub blocked_reason: Option<String>,
}

/// Per-regime coverage rollup (A2/A3/A5): window, tier counts, gap flags.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminRegimeCoverage {
    /// Gold `disclosure_regime` id.
    pub regime_id: String,
    /// Regime body (e.g. `US House`).
    pub body: String,
    /// Owning jurisdiction id.
    pub jurisdiction_id: String,
    /// Owning jurisdiction name.
    pub jurisdiction_name: String,
    /// The jurisdiction's `coverage_phase`.
    pub coverage_phase: String,
    /// Adapter regime codes provenance bridged to this regime
    /// (`filing → raw_document → pipeline_run.adapter`); empty = unbridged
    /// (e.g. fixture-seeded documents with no fetch run).
    pub regime_codes: Vec<String>,
    /// Distinct politicians with at least one filing under this regime.
    pub politicians: i64,
    /// Filings under this regime.
    pub filings: i64,
    /// Gold `disclosure_record` rows under this regime.
    pub gold_records: i64,
    /// Earliest `filed_date` seen; `null` when no filing carries one.
    pub first_filed_date: Option<NaiveDate>,
    /// Latest `filed_date` seen; `null` when no filing carries one.
    pub last_filed_date: Option<NaiveDate>,
    /// Distinct Gold `record_type`s observed under this regime.
    pub record_types: Vec<String>,
    /// Bronze documents fetched by this regime's bridged adapter(s); `null`
    /// when the regime is unbridged (count not attributable, never guessed).
    pub bronze_documents: Option<i64>,
    /// Silver staging rows. Only `stg_us_house` and `stg_br` exist today, so
    /// this is `null` for every other regime — a missing staging table is
    /// reported as unavailable, not zero.
    pub silver_rows: Option<i64>,
    /// A5 gap flag: the jurisdiction reached `built`/`live` but the regime
    /// has zero Gold records — built, not yet backfilled.
    pub built_not_backfilled: bool,
}

/// One cell of the regime × year × `record_type` heatmap (A4).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminHeatmapCell {
    /// Gold `disclosure_regime` id.
    pub regime_id: String,
    /// Calendar year of `event_date`.
    pub year: i32,
    /// Gold `record_type`.
    pub record_type: String,
    /// Records in the cell.
    pub records: i64,
}

/// Entity inventory (A6): reconciliation coverage plus the invariant-3
/// never-guess backlog.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminEntityInventory {
    /// Politicians in the registry.
    pub politicians: i64,
    /// Politicians carrying a `wikidata_qid`.
    pub politicians_with_wikidata: i64,
    /// Percent of politicians with a `wikidata_qid`; `null` when there are
    /// no politicians (no denominator, no made-up 0%).
    pub politician_wikidata_pct: Option<f64>,
    /// Instruments in the registry.
    pub instruments: i64,
    /// Instruments carrying a ticker.
    pub instruments_with_ticker: i64,
    /// Instruments carrying an ISIN.
    pub instruments_with_isin: i64,
    /// Percent of instruments with a ticker; `null` when there are none.
    pub instrument_ticker_pct: Option<f64>,
    /// Percent of instruments with an ISIN; `null` when there are none.
    pub instrument_isin_pct: Option<f64>,
    /// All Gold `disclosure_record` rows.
    pub records_total: i64,
    /// Gold rows with NULL `instrument_id` — the invariant-3 backlog
    /// (below-threshold matches stay NULL, never guessed).
    pub records_null_instrument: i64,
}

/// `GET /v1/admin/coverage` — the full section-A payload, one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminCoverage {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// A1: jurisdictions per `coverage_phase`.
    pub phases: Vec<AdminPhaseCount>,
    /// A1/A5: blocked jurisdictions with reasons.
    pub blocked: Vec<AdminBlockedJurisdiction>,
    /// A2/A3/A5: per-regime rollup (every regime row, even zero-data ones).
    pub regimes: Vec<AdminRegimeCoverage>,
    /// A3 honesty bucket: Bronze documents attributable to NO regime via the
    /// bridge (no fetch run, or an adapter that never produced a bridged
    /// filing) — surfaced, never silently dropped.
    pub bronze_unbridged: i64,
    /// A4: regime × year × `record_type` cells (rows with an `event_date`).
    pub heatmap: Vec<AdminHeatmapCell>,
    /// A4 honesty bucket: Gold rows with NULL `event_date`, which no heatmap
    /// cell can carry.
    pub heatmap_missing_event_date: i64,
    /// A6: entity inventory.
    pub entities: AdminEntityInventory,
}

// -------------------------------------------------------------- statements --

const PHASES_SQL: &str = "select coverage_phase, count(*) from jurisdiction \
     group by coverage_phase order by count(*) desc, coverage_phase";

const BLOCKED_SQL: &str = "select id, name, blocked_reason from jurisdiction \
     where coverage_phase = 'blocked' order by name";

/// Regime skeleton: one row per regime whatever its data volume — a regime
/// with zero filings must still appear in the gap analysis.
const REGIME_BASE_SQL: &str = "select dr.id, dr.body, j.id, j.name, j.coverage_phase \
     from disclosure_regime dr \
     join jurisdiction j on j.id = dr.jurisdiction_id \
     order by dr.id";

/// Filing-side aggregates, `filing` only (no record join = no fan-out).
const FILING_STATS_SQL: &str = "select regime_id, count(*), count(distinct politician_id), \
        min(filed_date), max(filed_date) \
     from filing group by regime_id";

/// Gold-side aggregates, `disclosure_record` only.
const GOLD_STATS_SQL: &str = "select regime_id, count(*), \
        array_agg(distinct record_type order by record_type) \
     from disclosure_record group by regime_id";

/// The bridge pairs themselves — which adapter code(s) feed each regime.
const BRIDGE_PAIRS_SQL: &str = concatcp!(
    BRIDGE_CTE,
    " select regime_id, regime_code from bridge order by regime_id, regime_code"
);

/// Bronze per regime: documents fetched by the regime's bridged adapter(s).
/// `count(distinct rd.id)` so a document is never double-counted within one
/// regime even if the bridge carries several codes.
const BRONZE_PER_REGIME_SQL: &str = concatcp!(
    BRIDGE_CTE,
    " select b.regime_id, count(distinct rd.id) \
      from bridge b \
      join pipeline_run pr on pr.adapter = b.regime_code \
      join raw_document rd on rd.fetch_run_id = pr.id \
      group by b.regime_id"
);

/// Bronze documents no bridge row can claim: fetched with no run id, by an
/// unknown run, or by an adapter that never bridged to a regime.
const BRONZE_UNBRIDGED_SQL: &str = concatcp!(
    BRIDGE_CTE,
    " select count(*) from raw_document rd \
      where rd.fetch_run_id is null \
         or not exists (select 1 from pipeline_run pr \
                        join bridge b on b.regime_code = pr.adapter \
                        where pr.id = rd.fetch_run_id)"
);

const SILVER_US_HOUSE_SQL: &str = "select count(*) from stg_us_house";
const SILVER_BR_SQL: &str = "select count(*) from stg_br";

const HEATMAP_SQL: &str = "select regime_id, extract(year from event_date)::int, record_type, count(*) \
     from disclosure_record where event_date is not null \
     group by 1, 2, 3 order by 1, 2, 3";

const HEATMAP_MISSING_SQL: &str = "select count(*) from disclosure_record where event_date is null";

const POLITICIAN_INVENTORY_SQL: &str = "select count(*), count(wikidata_qid) from politician";
const INSTRUMENT_INVENTORY_SQL: &str =
    "select count(*), count(ticker), count(isin) from instrument";
const RECORD_INVENTORY_SQL: &str =
    "select count(*), count(*) filter (where instrument_id is null) from disclosure_record";

// ---------------------------------------------------------------- assembly --

/// Percent readout, `null` when the denominator is empty (no made-up 0%).
#[allow(clippy::cast_precision_loss)] // registry counts sit far below 2^52; exact for a % readout
fn pct(part: i64, total: i64) -> Option<f64> {
    if total <= 0 {
        return None;
    }
    Some(100.0 * part as f64 / total as f64)
}

async fn fetch_entities(state: &AppState) -> Result<AdminEntityInventory, ApiError> {
    let (politicians, with_wikidata): (i64, i64) = sqlx::query_as(POLITICIAN_INVENTORY_SQL)
        .fetch_one(&state.pool)
        .await?;
    let (instruments, with_ticker, with_isin): (i64, i64, i64) =
        sqlx::query_as(INSTRUMENT_INVENTORY_SQL)
            .fetch_one(&state.pool)
            .await?;
    let (records_total, records_null_instrument): (i64, i64) = sqlx::query_as(RECORD_INVENTORY_SQL)
        .fetch_one(&state.pool)
        .await?;
    Ok(AdminEntityInventory {
        politicians,
        politicians_with_wikidata: with_wikidata,
        politician_wikidata_pct: pct(with_wikidata, politicians),
        instruments,
        instruments_with_ticker: with_ticker,
        instruments_with_isin: with_isin,
        instrument_ticker_pct: pct(with_ticker, instruments),
        instrument_isin_pct: pct(with_isin, instruments),
        records_total,
        records_null_instrument,
    })
}

/// Assembles the per-regime rollup from the split aggregates.
async fn fetch_regimes(state: &AppState) -> Result<Vec<AdminRegimeCoverage>, ApiError> {
    let base: Vec<(String, String, String, String, String)> = sqlx::query_as(REGIME_BASE_SQL)
        .fetch_all(&state.pool)
        .await?;
    let filing_stats: HashMap<String, (i64, i64, Option<NaiveDate>, Option<NaiveDate>)> =
        sqlx::query_as::<_, (String, i64, i64, Option<NaiveDate>, Option<NaiveDate>)>(
            FILING_STATS_SQL,
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(regime_id, filings, politicians, min_filed, max_filed)| {
            (regime_id, (filings, politicians, min_filed, max_filed))
        })
        .collect();
    let gold_stats: HashMap<String, (i64, Vec<String>)> =
        sqlx::query_as::<_, (String, i64, Vec<String>)>(GOLD_STATS_SQL)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .map(|(regime_id, gold, record_types)| (regime_id, (gold, record_types)))
            .collect();
    let mut bridge_codes: HashMap<String, Vec<String>> = HashMap::new();
    let pairs: Vec<(String, String)> = sqlx::query_as(BRIDGE_PAIRS_SQL)
        .fetch_all(&state.pool)
        .await?;
    for (regime_id, regime_code) in pairs {
        bridge_codes.entry(regime_id).or_default().push(regime_code);
    }
    let bronze: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>(BRONZE_PER_REGIME_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect();
    let (silver_us_house,): (i64,) = sqlx::query_as(SILVER_US_HOUSE_SQL)
        .fetch_one(&state.pool)
        .await?;
    let (silver_br,): (i64,) = sqlx::query_as(SILVER_BR_SQL).fetch_one(&state.pool).await?;

    let regimes = base
        .into_iter()
        .map(
            |(regime_id, body, jurisdiction_id, jurisdiction_name, coverage_phase)| {
                let (filings, politicians, first_filed_date, last_filed_date) = filing_stats
                    .get(&regime_id)
                    .copied()
                    .unwrap_or((0, 0, None, None));
                let (gold_records, record_types) = gold_stats
                    .get(&regime_id)
                    .cloned()
                    .unwrap_or((0, Vec::new()));
                let regime_codes = bridge_codes.get(&regime_id).cloned().unwrap_or_default();
                // Silver rows only exist for the two staging tables shipped
                // today; every other regime honestly reports "unavailable".
                let mut silver_rows: Option<i64> = None;
                for code in &regime_codes {
                    let rows = match code.as_str() {
                        "us_house" => silver_us_house,
                        "br" => silver_br,
                        _ => continue,
                    };
                    silver_rows = Some(silver_rows.unwrap_or(0) + rows);
                }
                let bronze_documents = if regime_codes.is_empty() {
                    None
                } else {
                    Some(bronze.get(&regime_id).copied().unwrap_or(0))
                };
                let built_not_backfilled =
                    matches!(coverage_phase.as_str(), "built" | "live") && gold_records == 0;
                AdminRegimeCoverage {
                    regime_id,
                    body,
                    jurisdiction_id,
                    jurisdiction_name,
                    coverage_phase,
                    regime_codes,
                    politicians,
                    filings,
                    gold_records,
                    first_filed_date,
                    last_filed_date,
                    record_types,
                    bronze_documents,
                    silver_rows,
                    built_not_backfilled,
                }
            },
        )
        .collect();
    Ok(regimes)
}

// ----------------------------------------------------------------- handler --

/// Section-A coverage & inventory snapshot (admin dashboard `/admin/coverage`).
///
/// # Errors
/// `401` without a valid `X-Admin-Token` (enforced by the route-layer gate);
/// `500` on backend failure — all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/coverage",
    tag = "admin",
    responses(
        (status = 200, description = "Coverage & inventory: phases, blocked list, per-regime rollup, heatmap, entity inventory", body = AdminCoverage),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_coverage(
    State(state): State<AppState>,
) -> Result<Json<AdminCoverage>, ApiError> {
    let phases = sqlx::query_as::<_, (String, i64)>(PHASES_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(phase, jurisdictions)| AdminPhaseCount {
            phase,
            jurisdictions,
        })
        .collect();
    let blocked = sqlx::query_as::<_, (String, String, Option<String>)>(BLOCKED_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(
            |(jurisdiction_id, name, blocked_reason)| AdminBlockedJurisdiction {
                jurisdiction_id,
                name,
                blocked_reason,
            },
        )
        .collect();
    let regimes = fetch_regimes(&state).await?;
    let (bronze_unbridged,): (i64,) = sqlx::query_as(BRONZE_UNBRIDGED_SQL)
        .fetch_one(&state.pool)
        .await?;
    let heatmap = sqlx::query_as::<_, (String, i32, String, i64)>(HEATMAP_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(regime_id, year, record_type, records)| AdminHeatmapCell {
            regime_id,
            year,
            record_type,
            records,
        })
        .collect();
    let (heatmap_missing_event_date,): (i64,) = sqlx::query_as(HEATMAP_MISSING_SQL)
        .fetch_one(&state.pool)
        .await?;
    let entities = fetch_entities(&state).await?;
    Ok(Json(AdminCoverage {
        generated_at: Utc::now(),
        phases,
        blocked,
        regimes,
        bronze_unbridged,
        heatmap,
        heatmap_missing_event_date,
        entities,
    }))
}
