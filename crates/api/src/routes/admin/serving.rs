//! `GET /v1/admin/serving` — section F, serving & product: API usage (F1),
//! alert latency percentiles with backfill-suppressed rows excluded and
//! counted separately (F2), delivery health + DLQ (F3). Per-request latency
//! is not recorded anywhere — the page states that, no fake numbers.
//!
//! READ-ONLY: `SELECT`s only, per the `super` contract.

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// ------------------------------------------------------------- wire shapes --

/// Authenticated API requests on one day (F1; `usage_event` rows).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUsageDay {
    /// The day (UTC date of `occurred_at`).
    pub day: NaiveDate,
    /// Authenticated requests metered that day.
    pub requests: i64,
}

/// Request count for one endpoint (F1 top-endpoints board).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminEndpointCount {
    /// Endpoint label as metered (`usage_event.endpoint`).
    pub endpoint: String,
    /// Requests over the window.
    pub requests: i64,
}

/// Account count for one tier.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminTierCount {
    /// `free` | `pro` | `data`.
    pub tier: String,
    /// Accounts on that tier.
    pub users: i64,
}

/// Accounts, keys and subscriptions at a glance (F1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminAccounts {
    /// Accounts per tier.
    pub users_by_tier: Vec<AdminTierCount>,
    /// API keys not revoked.
    pub active_keys: i64,
    /// API keys with `revoked_at` set (history is kept, never deleted).
    pub revoked_keys: i64,
    /// Stripe-mirrored subscriptions in a paying state (`active` or
    /// `trialing` — the same statuses the webhook treats as paid).
    pub active_subscriptions: i64,
}

/// Alert pipeline latency (F2): outbox created→dispatched percentiles plus
/// delivery created→sent percentiles. All figures are seconds.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminAlertLatency {
    /// p50 of `dispatched_at - created_at` in seconds, real dispatches only;
    /// `null` when no qualifying row exists.
    pub dispatch_p50_seconds: Option<f64>,
    /// p90 of the same distribution.
    pub dispatch_p90_seconds: Option<f64>,
    /// p99 of the same distribution.
    pub dispatch_p99_seconds: Option<f64>,
    /// Dispatched events included in the percentiles (delta >= 1 second).
    pub dispatched_count: i64,
    /// Dispatched events EXCLUDED from the percentiles because their
    /// created→dispatched delta is under 1 second — see `note`.
    pub pre_dispatched_count: i64,
    /// Why sub-1-second rows are excluded (documented approximation).
    pub note: String,
    /// p50 of `updated_at - created_at` in seconds over sent deliveries;
    /// `null` when nothing was sent yet.
    pub send_p50_seconds: Option<f64>,
    /// p90 of the same distribution.
    pub send_p90_seconds: Option<f64>,
    /// Deliveries with status `sent` backing the send percentiles.
    pub sent_count: i64,
}

/// Delivery count for one (status, channel) cell (F3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminDeliveryStatusChannel {
    /// `pending` | `pending_digest` | `sent` | `dead`.
    pub status: String,
    /// `email` | `webhook`.
    pub channel: String,
    /// Deliveries in that cell.
    pub count: i64,
}

/// One bar of the delivery attempts histogram (F3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminAttemptsBucket {
    /// Attempt count.
    pub attempts: i32,
    /// Deliveries with exactly that many attempts.
    pub count: i64,
}

/// One dead-lettered delivery (F3 DLQ surface).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminDeadDelivery {
    /// Delivery ULID.
    pub id: String,
    /// The alert rule that fanned out.
    pub alert_rule_id: String,
    /// The outbox event that triggered it.
    pub outbox_event_id: String,
    /// `email` | `webhook`.
    pub channel: String,
    /// Attempts made before dead-lettering.
    pub attempts: i32,
    /// Last error recorded by the dispatcher, verbatim.
    pub last_error: Option<String>,
    /// When the delivery last changed (the dead-lettering moment).
    pub updated_at: DateTime<Utc>,
}

/// Delivery health (F3): status×channel grid, attempts histogram, recent DLQ.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminDeliveries {
    /// Deliveries per (status, channel) cell.
    pub by_status_channel: Vec<AdminDeliveryStatusChannel>,
    /// Deliveries per attempt count.
    pub attempts_histogram: Vec<AdminAttemptsBucket>,
    /// The 10 most recently dead-lettered deliveries with their errors.
    pub recent_dead: Vec<AdminDeadDelivery>,
}

/// Section F — serving & product, one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminServing {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// Authenticated requests per day, last 14 days (days with zero metered
    /// requests are absent).
    pub usage_by_day: Vec<AdminUsageDay>,
    /// Top 10 endpoints by request count, last 7 days.
    pub top_endpoints_7d: Vec<AdminEndpointCount>,
    /// Accounts, keys and subscriptions.
    pub accounts: AdminAccounts,
    /// Alert pipeline latency percentiles.
    pub alert_latency: AdminAlertLatency,
    /// Delivery health and DLQ.
    pub deliveries: AdminDeliveries,
    /// Honest gap: per-request API latency is not recorded anywhere.
    pub latency_note: String,
}

// --------------------------------------------------------------------- SQL --

const USAGE_BY_DAY_SQL: &str = "select occurred_at::date as day, count(*) as requests \
     from usage_event \
     where occurred_at >= now() - interval '14 days' \
     group by 1 order by 1";

const TOP_ENDPOINTS_SQL: &str = "select endpoint, count(*) as requests \
     from usage_event \
     where occurred_at >= now() - interval '7 days' \
     group by endpoint order by count(*) desc, endpoint limit 10";

const USERS_BY_TIER_SQL: &str =
    "select tier, count(*) from user_account group by tier order by tier";

const KEY_COUNTS_SQL: &str = "select count(*) filter (where revoked_at is null) as active, \
            count(*) filter (where revoked_at is not null) as revoked \
     from api_key";

/// Same paying statuses the Stripe webhook mirror treats as paid
/// (`routes/stripe.rs`: `active` | `trialing`).
const ACTIVE_SUBSCRIPTIONS_SQL: &str =
    "select count(*) from subscription where status in ('active', 'trialing')";

/// Outbox created→dispatched percentiles. Rows whose delta is under 1 second
/// are pre-dispatched (backfill alert suppression writes `dispatched_at` in
/// the insert transaction) — excluded from the percentiles, counted apart.
const DISPATCH_LATENCY_SQL: &str = "select \
       percentile_cont(0.5) within group \
         (order by extract(epoch from dispatched_at - created_at)::double precision) \
         filter (where dispatched_at - created_at >= interval '1 second') as p50, \
       percentile_cont(0.9) within group \
         (order by extract(epoch from dispatched_at - created_at)::double precision) \
         filter (where dispatched_at - created_at >= interval '1 second') as p90, \
       percentile_cont(0.99) within group \
         (order by extract(epoch from dispatched_at - created_at)::double precision) \
         filter (where dispatched_at - created_at >= interval '1 second') as p99, \
       count(*) filter (where dispatched_at - created_at >= interval '1 second') as dispatched, \
       count(*) filter (where dispatched_at - created_at < interval '1 second') as pre_dispatched \
     from outbox_event where dispatched_at is not null";

/// Delivery created→sent percentiles over sent rows (`updated_at` is the
/// send moment for status 'sent').
const SEND_LATENCY_SQL: &str = "select \
       percentile_cont(0.5) within group \
         (order by extract(epoch from updated_at - created_at)::double precision) as p50, \
       percentile_cont(0.9) within group \
         (order by extract(epoch from updated_at - created_at)::double precision) as p90, \
       count(*) as sent \
     from delivery where status = 'sent'";

const STATUS_CHANNEL_SQL: &str = "select status, channel, count(*) \
     from delivery group by status, channel order by status, channel";

const ATTEMPTS_HISTOGRAM_SQL: &str =
    "select attempts, count(*) from delivery group by attempts order by attempts";

const RECENT_DEAD_SQL: &str = "select id, alert_rule_id, outbox_event_id, channel, attempts, last_error, updated_at \
     from delivery where status = 'dead' order by updated_at desc limit 10";

// --------------------------------------------------------------- raw rows --

#[derive(Debug, sqlx::FromRow)]
struct DispatchLatencyRow {
    p50: Option<f64>,
    p90: Option<f64>,
    p99: Option<f64>,
    dispatched: i64,
    pre_dispatched: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct SendLatencyRow {
    p50: Option<f64>,
    p90: Option<f64>,
    sent: i64,
}

// ------------------------------------------------------------- the handler --

/// Serving & product observability (section F) in one round trip: API usage,
/// accounts, alert latency percentiles and delivery health.
///
/// # Errors
/// `500` on backend failure — consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/serving",
    tag = "admin",
    responses(
        (status = 200, description = "Serving & product snapshot", body = AdminServing),
        (status = 401, description = "Missing or invalid admin token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn admin_serving(State(state): State<AppState>) -> Result<Json<AdminServing>, ApiError> {
    let usage_rows: Vec<(NaiveDate, i64)> = sqlx::query_as(USAGE_BY_DAY_SQL)
        .fetch_all(&state.pool)
        .await?;
    let endpoint_rows: Vec<(String, i64)> = sqlx::query_as(TOP_ENDPOINTS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let tier_rows: Vec<(String, i64)> = sqlx::query_as(USERS_BY_TIER_SQL)
        .fetch_all(&state.pool)
        .await?;
    let (active_keys, revoked_keys): (i64, i64) = sqlx::query_as(KEY_COUNTS_SQL)
        .fetch_one(&state.pool)
        .await?;
    let active_subscriptions: i64 = sqlx::query_scalar(ACTIVE_SUBSCRIPTIONS_SQL)
        .fetch_one(&state.pool)
        .await?;
    let dispatch: DispatchLatencyRow = sqlx::query_as(DISPATCH_LATENCY_SQL)
        .fetch_one(&state.pool)
        .await?;
    let send: SendLatencyRow = sqlx::query_as(SEND_LATENCY_SQL)
        .fetch_one(&state.pool)
        .await?;
    let status_channel_rows: Vec<(String, String, i64)> = sqlx::query_as(STATUS_CHANNEL_SQL)
        .fetch_all(&state.pool)
        .await?;
    let attempts_rows: Vec<(i32, i64)> = sqlx::query_as(ATTEMPTS_HISTOGRAM_SQL)
        .fetch_all(&state.pool)
        .await?;
    let recent_dead: Vec<AdminDeadDelivery> = sqlx::query_as(RECENT_DEAD_SQL)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(AdminServing {
        generated_at: Utc::now(),
        usage_by_day: usage_rows
            .into_iter()
            .map(|(day, requests)| AdminUsageDay { day, requests })
            .collect(),
        top_endpoints_7d: endpoint_rows
            .into_iter()
            .map(|(endpoint, requests)| AdminEndpointCount { endpoint, requests })
            .collect(),
        accounts: AdminAccounts {
            users_by_tier: tier_rows
                .into_iter()
                .map(|(tier, users)| AdminTierCount { tier, users })
                .collect(),
            active_keys,
            revoked_keys,
            active_subscriptions,
        },
        alert_latency: AdminAlertLatency {
            dispatch_p50_seconds: dispatch.p50,
            dispatch_p90_seconds: dispatch.p90,
            dispatch_p99_seconds: dispatch.p99,
            dispatched_count: dispatch.dispatched,
            pre_dispatched_count: dispatch.pre_dispatched,
            note: "Backfill alert suppression writes dispatched_at inside the insert \
                   transaction, so a created-to-dispatched delta under 1 second marks a \
                   pre-dispatched (suppressed) event rather than a real dispatch; such rows \
                   are excluded from the percentiles and counted as pre_dispatched_count. \
                   Documented approximation: a genuinely sub-second live dispatch would be \
                   miscounted as pre-dispatched."
                .to_owned(),
            send_p50_seconds: send.p50,
            send_p90_seconds: send.p90,
            sent_count: send.sent,
        },
        deliveries: AdminDeliveries {
            by_status_channel: status_channel_rows
                .into_iter()
                .map(|(status, channel, count)| AdminDeliveryStatusChannel {
                    status,
                    channel,
                    count,
                })
                .collect(),
            attempts_histogram: attempts_rows
                .into_iter()
                .map(|(attempts, count)| AdminAttemptsBucket { attempts, count })
                .collect(),
            recent_dead,
        },
        latency_note: "Per-request API latency is not recorded anywhere (usage_event has no \
                       latency column); only request counts are observable. Future: a latency \
                       column on usage_event."
            .to_owned(),
    }))
}
