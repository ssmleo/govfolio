//! `/v1/alert-rules` — minimal CRUD over alert rules (design §6.3).
//!
//! The `filter` field speaks the ONE shared grammar ([`RecordFilter`]) and is
//! validated STRICTLY against the committed JSON Schema at the door (the
//! schema forbids unknown keys, so a typo'd filter fails instead of silently
//! matching everything — invariant-5 discipline applied to rule config).
//!
//! AUTH (goal 050): alerts are the paid fast path (design §6.2 — free tier
//! has no alerts, so an open rules door would tunnel under the 24h delay).
//! Every endpoint requires a pro/data-tier API key; rules belong to the
//! authenticated account and are invisible across accounts.

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use govfolio_core::alerts::AlertChannel;
use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::auth::{AuthContext, Principal};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiJson;

/// Alert rules require an authenticated paid tier (see module docs).
fn require_paid(auth: &AuthContext) -> Result<&Principal, ApiError> {
    let principal = auth
        .principal
        .as_ref()
        .ok_or_else(|| ApiError::unauthorized("key_required", "authenticate with an API key"))?;
    if !auth.tier.realtime() {
        return Err(ApiError::forbidden(
            "tier_required",
            "alert rules require the pro or data tier",
        ));
    }
    Ok(principal)
}

/// One alert rule.
#[derive(Debug, Serialize, ToSchema)]
pub struct AlertRule {
    /// Rule ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Owning account (`user_account.id` — always the authenticated caller).
    pub user_id: String,
    /// Filter in the shared record grammar — identical to `/v1/records`
    /// query parameters (design §6.3: one grammar, learned once).
    pub filter: RecordFilter,
    /// Delivery channels; at most one per channel type.
    pub channels: Vec<AlertChannel>,
    /// Digest mode: matches accumulate and ship as one periodic summary
    /// instead of instant per-record sends.
    pub digest: bool,
    /// Inactive rules never match.
    pub active: bool,
    /// When the rule was created.
    pub created_at: DateTime<Utc>,
    /// When the rule was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Create/replace body for an alert rule (`POST` and `PUT` share it). The
/// owner is always the authenticated account — never caller-supplied.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AlertRuleSpec {
    /// Filter in the shared record grammar; unknown keys are rejected.
    #[schema(value_type = RecordFilter)]
    pub filter: serde_json::Value,
    /// Delivery channels; at least one, at most one per channel type.
    pub channels: Vec<AlertChannel>,
    /// Digest mode (defaults to instant).
    #[serde(default)]
    pub digest: bool,
    /// Whether the rule matches at all (defaults to active).
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

/// Raw `alert_rule` row; conversion re-types the jsonb columns through the
/// core contracts.
#[derive(Debug, sqlx::FromRow)]
struct AlertRuleRow {
    id: String,
    user_id: String,
    filter: serde_json::Value,
    channels: serde_json::Value,
    digest: bool,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<AlertRuleRow> for AlertRule {
    type Error = ApiError;

    fn try_from(row: AlertRuleRow) -> Result<Self, Self::Error> {
        let filter: RecordFilter = serde_json::from_value(row.filter)
            .map_err(|e| anyhow::anyhow!("stored filter is outside the grammar: {e}"))?;
        let channels: Vec<AlertChannel> = serde_json::from_value(row.channels)
            .map_err(|e| anyhow::anyhow!("stored channels are outside the contract: {e}"))?;
        Ok(Self {
            id: row.id,
            user_id: row.user_id,
            filter,
            channels,
            digest: row.digest,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// The shared row projection.
const RULE_COLUMNS: &str = "id, user_id, filter, channels, digest, active, created_at, updated_at";

/// Validates a spec and returns the CANONICAL jsonb payloads to store
/// (normalized through the typed grammar — no unknown keys, no nulls).
///
/// # Errors
/// `400` with a stable code on any contract violation.
fn validate_spec(spec: &AlertRuleSpec) -> Result<(serde_json::Value, serde_json::Value), ApiError> {
    // Strict grammar check against the committed schema (unknown keys reject).
    let schema = serde_json::to_value(govfolio_core::schemas::record_filter())
        .map_err(|e| anyhow::anyhow!("rendering the filter schema: {e}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|e| anyhow::anyhow!("compiling the filter schema: {e}"))?;
    let violations: Vec<String> = validator
        .iter_errors(&spec.filter)
        .map(|e| e.to_string())
        .collect();
    if !violations.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_filter",
            format!(
                "filter violates the record grammar: {}",
                violations.join("; ")
            ),
        ));
    }
    let filter: RecordFilter = serde_json::from_value(spec.filter.clone())
        .map_err(|e| ApiError::bad_request("invalid_filter", format!("filter: {e}")))?;
    if spec.channels.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_channels",
            "at least one delivery channel is required",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for channel in &spec.channels {
        if !seen.insert(channel.kind()) {
            return Err(ApiError::bad_request(
                "invalid_channels",
                format!(
                    "at most one {} channel per rule (create a second rule instead)",
                    channel.kind()
                ),
            ));
        }
        let empty = match channel {
            AlertChannel::Email { to } => to.trim().is_empty(),
            AlertChannel::Webhook { url, secret } => {
                url.trim().is_empty() || secret.trim().is_empty()
            }
        };
        if empty {
            return Err(ApiError::bad_request(
                "invalid_channels",
                format!("{} channel fields must be non-empty", channel.kind()),
            ));
        }
    }
    let filter = serde_json::to_value(&filter)
        .map_err(|e| anyhow::anyhow!("re-serializing the filter: {e}"))?;
    let channels = serde_json::to_value(&spec.channels)
        .map_err(|e| anyhow::anyhow!("re-serializing the channels: {e}"))?;
    Ok((filter, channels))
}

/// Creates an alert rule owned by the authenticated account.
///
/// # Errors
/// `401`/`403` without a pro/data key; `400` on a body outside the
/// contracts; `500` on backend failure.
#[utoipa::path(
    post,
    path = "/v1/alert-rules",
    tag = "alert-rules",
    request_body = AlertRuleSpec,
    responses(
        (status = 201, description = "The created rule", body = AlertRule),
        (status = 400, description = "Body violates the filter/channel contracts", body = ErrorBody),
        (status = 401, description = "No valid API key", body = ErrorBody),
        (status = 403, description = "Tier does not include alerts", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn create_alert_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    ApiJson(spec): ApiJson<AlertRuleSpec>,
) -> Result<(StatusCode, Json<AlertRule>), ApiError> {
    let principal = require_paid(&auth)?;
    let (filter, channels) = validate_spec(&spec)?;
    let row: AlertRuleRow = sqlx::query_as(const_format::concatcp!(
        "insert into alert_rule (id, user_id, filter, channels, digest, active) \
         values ($1, $2, $3, $4, $5, $6) returning ",
        RULE_COLUMNS
    ))
    .bind(ulid::Ulid::new().to_string())
    .bind(&principal.user_id)
    .bind(filter)
    .bind(channels)
    .bind(spec.digest)
    .bind(spec.active)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(row.try_into()?)))
}

/// Lists the authenticated account's alert rules.
///
/// # Errors
/// `401`/`403` without a pro/data key; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/alert-rules",
    tag = "alert-rules",
    responses(
        (status = 200, description = "The caller's rules", body = [AlertRule]),
        (status = 401, description = "No valid API key", body = ErrorBody),
        (status = 403, description = "Tier does not include alerts", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_alert_rules(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<AlertRule>>, ApiError> {
    let principal = require_paid(&auth)?;
    let rows: Vec<AlertRuleRow> = sqlx::query_as(const_format::concatcp!(
        "select ",
        RULE_COLUMNS,
        " from alert_rule where user_id = $1 order by id"
    ))
    .bind(&principal.user_id)
    .fetch_all(&state.pool)
    .await?;
    let rules = rows
        .into_iter()
        .map(AlertRule::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(rules))
}

/// Replaces one of the authenticated account's alert rules.
///
/// # Errors
/// `401`/`403` without a pro/data key; `400` on a body outside the
/// contracts; `404` for a rule that is unknown or not the caller's; `500`
/// on backend failure.
#[utoipa::path(
    put,
    path = "/v1/alert-rules/{id}",
    tag = "alert-rules",
    params(("id" = String, Path, description = "Rule ULID")),
    request_body = AlertRuleSpec,
    responses(
        (status = 200, description = "The updated rule", body = AlertRule),
        (status = 400, description = "Body violates the filter/channel contracts", body = ErrorBody),
        (status = 401, description = "No valid API key", body = ErrorBody),
        (status = 403, description = "Tier does not include alerts", body = ErrorBody),
        (status = 404, description = "Unknown (or not owned) rule", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn update_alert_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    ApiJson(spec): ApiJson<AlertRuleSpec>,
) -> Result<Json<AlertRule>, ApiError> {
    let principal = require_paid(&auth)?;
    let (filter, channels) = validate_spec(&spec)?;
    let row: Option<AlertRuleRow> = sqlx::query_as(const_format::concatcp!(
        "update alert_rule \
         set filter = $3, channels = $4, digest = $5, active = $6, \
             updated_at = now() \
         where id = $1 and user_id = $2 returning ",
        RULE_COLUMNS
    ))
    .bind(&id)
    .bind(&principal.user_id)
    .bind(filter)
    .bind(channels)
    .bind(spec.digest)
    .bind(spec.active)
    .fetch_optional(&state.pool)
    .await?;
    match row {
        Some(row) => Ok(Json(row.try_into()?)),
        None => Err(ApiError::NotFound {
            message: format!("alert rule {id} not found"),
        }),
    }
}

/// Deletes one of the authenticated account's alert rules (its delivery
/// ledger rows cascade with it).
///
/// # Errors
/// `401`/`403` without a pro/data key; `404` for a rule that is unknown or
/// not the caller's; `500` on backend failure.
#[utoipa::path(
    delete,
    path = "/v1/alert-rules/{id}",
    tag = "alert-rules",
    params(("id" = String, Path, description = "Rule ULID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "No valid API key", body = ErrorBody),
        (status = 403, description = "Tier does not include alerts", body = ErrorBody),
        (status = 404, description = "Unknown (or not owned) rule", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn delete_alert_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let principal = require_paid(&auth)?;
    let result = sqlx::query("delete from alert_rule where id = $1 and user_id = $2")
        .bind(&id)
        .bind(&principal.user_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound {
            message: format!("alert rule {id} not found"),
        });
    }
    Ok(StatusCode::NO_CONTENT)
}
