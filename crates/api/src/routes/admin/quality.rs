//! `GET /v1/admin/quality?sweep=br` — section D, data quality & review ops:
//! review-queue analytics (D1), unmatched-entity counts (D2), opt-in br CPF
//! collision sweep reusing the check-bin's SELECT verbatim (D3), idempotency
//! evidence from `backfill_run` (D4), raw retention spot check (D5),
//! `sample_audit` precision estimates (D6).
//!
//! The collision sweep is OPT-IN (`?sweep=br`) because it is a whole-dataset
//! scan over `stg_br`; the default payload never pays for it. Any other
//! `sweep` value is a 400 — unknown sweeps fail closed, they do not silently
//! no-op.

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::AppState;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;

// ------------------------------------------------------------- wire shapes --

/// Open review tasks sharing one `reason` (D1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminReasonCount {
    /// Why the tasks were opened (e.g. `ptr_amendment_unlinked`).
    pub reason: String,
    /// Open tasks with this reason.
    pub tasks: i64,
}

/// Open review tasks sharing one `target_kind` (D1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminTargetKindCount {
    /// What the tasks target (e.g. `disclosure_record`).
    pub target_kind: String,
    /// Open tasks with this target kind.
    pub tasks: i64,
}

/// Age distribution of the open review queue (D1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminAgeBuckets {
    /// Open tasks younger than 1 day.
    pub under_1d: i64,
    /// Open tasks aged 1–7 days.
    pub d1_to_7: i64,
    /// Open tasks aged 7–30 days.
    pub d7_to_30: i64,
    /// Open tasks older than 30 days.
    pub over_30d: i64,
}

/// Applied verdicts of one kind in the last 30 days (D1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminVerdictCount {
    /// `confirm` | `edit` | `reject`.
    pub verdict: String,
    /// Applied attempts carrying this verdict.
    pub attempts: i64,
}

/// 30-day resolution throughput (D1), from `review_audit` applied attempts
/// joined to their tasks' `resolved_at`.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminResolution30d {
    /// Distinct tasks resolved via an applied attempt in the last 30 days.
    pub resolved_tasks: i64,
    /// Median hours from task creation to resolution (`percentile_cont`);
    /// `null` when nothing resolved in the window.
    pub median_hours_to_resolve: Option<f64>,
    /// Applied verdicts by kind in the window.
    pub verdicts: Vec<AdminVerdictCount>,
}

/// NULL-instrument Gold rows (D2) — the invariant-3 never-guess backlog.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUnlinkedInstruments {
    /// All Gold rows with NULL `instrument_id`.
    pub total: i64,
    /// The listed subset (asset classes equity/bond/fund/option) — the rows
    /// a reference-data match could plausibly link.
    pub listed: i64,
    /// Listed breakdown: equity.
    pub equity: i64,
    /// Listed breakdown: bond.
    pub bond: i64,
    /// Listed breakdown: fund.
    pub fund: i64,
    /// Listed breakdown: option.
    pub option: i64,
}

/// One regime's precision estimate in the current sampling batch (D6).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminRegimePrecision {
    /// Gold `disclosure_regime` id.
    pub regime_id: String,
    /// Regime body (e.g. `US House`).
    pub body: String,
    /// Records drawn this month.
    pub sampled: i64,
    /// Draws already audited.
    pub audited: i64,
    /// Audited draws found discrepant.
    pub discrepancies: i64,
    /// `(audited - discrepancies) / audited`; `null` until something is
    /// audited (no estimate is invented from zero evidence).
    pub precision_estimate: Option<f64>,
}

/// Current-month sampling-audit precision (D6, design §7.4).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminPrecisionMonth {
    /// The batch label, `YYYY-MM` (the database's current month).
    pub sample_month: String,
    /// Per-regime estimates; empty when no batch was drawn this month.
    pub regimes: Vec<AdminRegimePrecision>,
}

/// Idempotency evidence from `backfill_run` (D4, invariant 4).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminIdempotency {
    /// `backfill_run` rows counted.
    pub runs: i64,
    /// Sum of `replayed` — already-published filings re-runs left untouched.
    pub replayed_total: i64,
    /// Scope caveat rendered verbatim on the page.
    pub note: String,
}

/// Raw-retention spot check (D5, invariant 2).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminRawRetention {
    /// All Bronze `raw_document` rows.
    pub raw_documents: i64,
    /// Distinct documents referenced by at least one filing.
    pub linked_to_filing: i64,
    /// Documents no filing references (fetched but not yet promoted, or
    /// evidence-only captures) — retained either way, Bronze is immutable.
    pub orphaned: i64,
}

/// One politician whose `stg_br` rows carry more than one distinct CPF (D3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBrCpfCollision {
    /// Politician id.
    pub politician_id: String,
    /// Canonical name.
    pub canonical_name: String,
    /// Distinct CPFs seen across the politician's filings.
    pub distinct_cpfs: i64,
    /// The CPF values themselves.
    pub cpfs: Vec<String>,
}

/// br CPF collision sweep result (D3) — report-only, exactly like the
/// `check-br-identity-collisions` bin: zero rows = PASS, rows = investigate.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBrCollisionSweep {
    /// `true` when zero collisions were found.
    pub pass: bool,
    /// The collision rows, when any.
    pub collisions: Vec<AdminBrCpfCollision>,
}

/// `GET /v1/admin/quality` — the full section-D payload, one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminQuality {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// D1: open tasks by reason, most common first.
    pub open_by_reason: Vec<AdminReasonCount>,
    /// D1: open tasks by target kind, most common first.
    pub open_by_target_kind: Vec<AdminTargetKindCount>,
    /// D1: open-queue age distribution.
    pub open_age_buckets: AdminAgeBuckets,
    /// D1: 30-day resolution throughput + verdict mix.
    pub resolution_30d: AdminResolution30d,
    /// D2: NULL-instrument backlog.
    pub unlinked_instruments: AdminUnlinkedInstruments,
    /// D6: current-month precision estimates.
    pub precision_current_month: AdminPrecisionMonth,
    /// D4: replay counters from `backfill_run`.
    pub idempotency: AdminIdempotency,
    /// D5: raw-retention spot check.
    pub raw_retention: AdminRawRetention,
    /// D3: only present when the request opted in with `?sweep=br`.
    pub collision_sweep: Option<AdminBrCollisionSweep>,
}

/// Query parameters of `GET /v1/admin/quality`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AdminQualityParams {
    /// Opt-in expensive sweep. The only supported value is `br` (the CPF
    /// collision sweep); anything else is a 400.
    pub sweep: Option<String>,
}

// -------------------------------------------------------------- statements --

const OPEN_BY_REASON_SQL: &str = "select reason, count(*) from review_task \
     where status = 'open' group by reason order by count(*) desc, reason";

const OPEN_BY_TARGET_KIND_SQL: &str = "select target_kind, count(*) from review_task \
     where status = 'open' group by target_kind order by count(*) desc, target_kind";

const AGE_BUCKETS_SQL: &str = "select \
        count(*) filter (where created_at >= now() - interval '1 day'), \
        count(*) filter (where created_at < now() - interval '1 day' \
                           and created_at >= now() - interval '7 days'), \
        count(*) filter (where created_at < now() - interval '7 days' \
                           and created_at >= now() - interval '30 days'), \
        count(*) filter (where created_at < now() - interval '30 days') \
     from review_task where status = 'open'";

/// Applied attempts joined to their tasks; deduplicated per task before the
/// median so a (theoretical) second applied row cannot skew it.
const RESOLUTION_30D_SQL: &str = "select count(*), \
        percentile_cont(0.5) within group (order by hours) \
     from (select distinct rt.id, \
                  (extract(epoch from (rt.resolved_at - rt.created_at)) / 3600.0)::double precision as hours \
           from review_audit ra \
           join review_task rt on rt.id = ra.review_task_id \
           where ra.outcome = 'applied' \
             and rt.resolved_at is not null \
             and rt.resolved_at >= now() - interval '30 days') resolved";

const VERDICTS_30D_SQL: &str = "select verdict, count(*) from review_audit \
     where outcome = 'applied' and created_at >= now() - interval '30 days' \
     group by verdict order by verdict";

const UNLINKED_SQL: &str = "select count(*), \
        count(*) filter (where asset_class in ('equity','bond','fund','option')), \
        count(*) filter (where asset_class = 'equity'), \
        count(*) filter (where asset_class = 'bond'), \
        count(*) filter (where asset_class = 'fund'), \
        count(*) filter (where asset_class = 'option') \
     from disclosure_record where instrument_id is null";

/// The batch label the precision query filters on — read from the database
/// so the label and the filter can never disagree.
const SAMPLE_MONTH_SQL: &str = "select to_char(now(), 'YYYY-MM')";

/// D6 — this SELECT is `crates/worker/src/sampler.rs` `precision_report`'s
/// query verbatim, with the `$1` month bind replaced by
/// `to_char(now(), 'YYYY-MM')` (the current batch).
const PRECISION_SQL: &str = "select sa.regime_id, dr.body, \
                count(*) as sampled, \
                count(*) filter (where sa.status <> 'pending') as audited, \
                count(*) filter (where sa.status = 'discrepancy') as discrepancies \
         from sample_audit sa \
         join disclosure_regime dr on dr.id = sa.regime_id \
         where sa.sample_month = to_char(now(), 'YYYY-MM') \
         group by sa.regime_id, dr.body \
         order by sa.regime_id";

/// `sum(bigint)` is numeric in Postgres; cast back to bigint for the wire.
const IDEMPOTENCY_SQL: &str =
    "select count(*), coalesce(sum(replayed), 0)::bigint from backfill_run";

const IDEMPOTENCY_NOTE: &str = "replayed = already-published filings a re-run left untouched \
     (invariant-4 evidence). Counters exist only for runs recorded since migration 0011; \
     earlier history is log-only and NOT represented here.";

const RAW_RETENTION_SQL: &str = "select (select count(*) from raw_document), \
        (select count(distinct raw_document_id) from filing)";

/// D3 — copied VERBATIM from
/// `crates/worker/src/bin/check-br-identity-collisions.rs` (`SWEEP_SQL`),
/// which itself copies the sweep query in
/// `docs/decisions/br-identity-collision-remediation.md` §2. Report-only:
/// this endpoint never fixes anything, exactly like the bin.
const SWEEP_SQL: &str = "select p.id as politician_id, p.canonical_name, \
     count(distinct s.nr_cpf_candidato) as distinct_cpfs, \
     array_agg(distinct s.nr_cpf_candidato) as cpfs \
     from politician p \
     join filing f on f.politician_id = p.id \
     join raw_document rd on rd.id = f.raw_document_id \
     join stg_br s on s.raw_document_id = rd.id \
     where s.nr_cpf_candidato is not null \
     group by p.id, p.canonical_name \
     having count(distinct s.nr_cpf_candidato) > 1";

// ---------------------------------------------------------------- assembly --

/// `(audited - discrepancies) / audited`, or `None` when nothing is audited
/// yet — same formula as `worker::sampler::precision_estimate`.
#[allow(clippy::cast_precision_loss)] // audit counts are tiny; f64 is exact here
fn precision_estimate(audited: i64, discrepancies: i64) -> Option<f64> {
    if audited <= 0 {
        return None;
    }
    Some((audited - discrepancies) as f64 / audited as f64)
}

async fn fetch_precision(state: &AppState) -> Result<AdminPrecisionMonth, ApiError> {
    let (sample_month,): (String,) = sqlx::query_as(SAMPLE_MONTH_SQL)
        .fetch_one(&state.pool)
        .await?;
    let regimes = sqlx::query_as::<_, (String, String, i64, i64, i64)>(PRECISION_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(
            |(regime_id, body, sampled, audited, discrepancies)| AdminRegimePrecision {
                regime_id,
                body,
                sampled,
                audited,
                discrepancies,
                precision_estimate: precision_estimate(audited, discrepancies),
            },
        )
        .collect();
    Ok(AdminPrecisionMonth {
        sample_month,
        regimes,
    })
}

async fn fetch_resolution_30d(state: &AppState) -> Result<AdminResolution30d, ApiError> {
    let (resolved_tasks, median_hours_to_resolve): (i64, Option<f64>) =
        sqlx::query_as(RESOLUTION_30D_SQL)
            .fetch_one(&state.pool)
            .await?;
    let verdicts = sqlx::query_as::<_, (String, i64)>(VERDICTS_30D_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(verdict, attempts)| AdminVerdictCount { verdict, attempts })
        .collect();
    Ok(AdminResolution30d {
        resolved_tasks,
        median_hours_to_resolve,
        verdicts,
    })
}

async fn fetch_collision_sweep(state: &AppState) -> Result<AdminBrCollisionSweep, ApiError> {
    let collisions: Vec<AdminBrCpfCollision> =
        sqlx::query_as::<_, (String, String, i64, Vec<String>)>(SWEEP_SQL)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .map(
                |(politician_id, canonical_name, distinct_cpfs, cpfs)| AdminBrCpfCollision {
                    politician_id,
                    canonical_name,
                    distinct_cpfs,
                    cpfs,
                },
            )
            .collect();
    Ok(AdminBrCollisionSweep {
        pass: collisions.is_empty(),
        collisions,
    })
}

// ----------------------------------------------------------------- handler --

/// Section-D data-quality & review-ops snapshot (admin dashboard
/// `/admin/quality`).
///
/// # Errors
/// `400` on an unsupported `sweep` value; `401` without a valid
/// `X-Admin-Token` (enforced by the route-layer gate); `500` on backend
/// failure — all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/quality",
    tag = "admin",
    params(AdminQualityParams),
    responses(
        (status = 200, description = "Data quality & review ops: queue analytics, unlinked backlog, precision, idempotency, raw retention, optional br sweep", body = AdminQuality),
        (status = 400, description = "Unsupported sweep value", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_quality(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<AdminQualityParams>,
) -> Result<Json<AdminQuality>, ApiError> {
    // Validate the opt-in FIRST: an unknown sweep must fail closed before
    // any work happens, never degrade into "quality without the sweep".
    let run_sweep = match params.sweep.as_deref() {
        None => false,
        Some("br") => true,
        Some(other) => {
            return Err(ApiError::bad_request(
                "invalid_sweep",
                format!("sweep must be \"br\" (the only sweep implemented), got {other:?}"),
            ));
        }
    };
    let open_by_reason = sqlx::query_as::<_, (String, i64)>(OPEN_BY_REASON_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(reason, tasks)| AdminReasonCount { reason, tasks })
        .collect();
    let open_by_target_kind = sqlx::query_as::<_, (String, i64)>(OPEN_BY_TARGET_KIND_SQL)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|(target_kind, tasks)| AdminTargetKindCount { target_kind, tasks })
        .collect();
    let (under_1d, d1_to_7, d7_to_30, over_30d): (i64, i64, i64, i64) =
        sqlx::query_as(AGE_BUCKETS_SQL)
            .fetch_one(&state.pool)
            .await?;
    let resolution_30d = fetch_resolution_30d(&state).await?;
    let (total, listed, equity, bond, fund, option): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(UNLINKED_SQL).fetch_one(&state.pool).await?;
    let precision_current_month = fetch_precision(&state).await?;
    let (runs, replayed_total): (i64, i64) = sqlx::query_as(IDEMPOTENCY_SQL)
        .fetch_one(&state.pool)
        .await?;
    let (raw_documents, linked_to_filing): (i64, i64) = sqlx::query_as(RAW_RETENTION_SQL)
        .fetch_one(&state.pool)
        .await?;
    let collision_sweep = if run_sweep {
        Some(fetch_collision_sweep(&state).await?)
    } else {
        None
    };
    Ok(Json(AdminQuality {
        generated_at: Utc::now(),
        open_by_reason,
        open_by_target_kind,
        open_age_buckets: AdminAgeBuckets {
            under_1d,
            d1_to_7,
            d7_to_30,
            over_30d,
        },
        resolution_30d,
        unlinked_instruments: AdminUnlinkedInstruments {
            total,
            listed,
            equity,
            bond,
            fund,
            option,
        },
        precision_current_month,
        idempotency: AdminIdempotency {
            runs,
            replayed_total,
            note: IDEMPOTENCY_NOTE.to_owned(),
        },
        raw_retention: AdminRawRetention {
            raw_documents,
            linked_to_filing,
            orphaned: raw_documents - linked_to_filing,
        },
        collision_sweep,
    }))
}
