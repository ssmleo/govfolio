//! `/v1/alert-rules` — minimal CRUD over alert rules (design §6.3).
//!
//! The `filter` field speaks the ONE shared grammar ([`RecordFilter`]) and is
//! validated STRICTLY against the committed JSON Schema at the door (the
//! schema forbids unknown keys, so a typo'd filter fails instead of silently
//! matching everything — invariant-5 discipline applied to rule config).
//!
//! AUTH: none yet — accounts and tier enforcement land in goal 050; until
//! then `user_id` is caller-supplied free text (schema already has room).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use govfolio_core::alerts::AlertChannel;
use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ApiQuery};

/// One alert rule.
#[derive(Debug, Serialize, ToSchema)]
pub struct AlertRule {
    /// Rule ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Owning user (free text until accounts land — goal 050).
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

/// Create/replace body for an alert rule (`POST` and `PUT` share it).
#[derive(Debug, Deserialize, ToSchema)]
pub struct AlertRuleSpec {
    /// Owning user (free text until accounts land — goal 050).
    pub user_id: String,
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

/// Query parameters of `GET /v1/alert-rules`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAlertRulesParams {
    /// Only rules owned by this user.
    pub user_id: Option<String>,
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
    if spec.user_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_user_id",
            "user_id must be non-empty",
        ));
    }
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

/// Creates an alert rule.
///
/// # Errors
/// `400` on a body outside the contracts; `500` on backend failure.
#[utoipa::path(
    post,
    path = "/v1/alert-rules",
    tag = "alert-rules",
    request_body = AlertRuleSpec,
    responses(
        (status = 201, description = "The created rule", body = AlertRule),
        (status = 400, description = "Body violates the filter/channel contracts", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn create_alert_rule(
    State(state): State<AppState>,
    ApiJson(spec): ApiJson<AlertRuleSpec>,
) -> Result<(StatusCode, Json<AlertRule>), ApiError> {
    let (filter, channels) = validate_spec(&spec)?;
    let row: AlertRuleRow = sqlx::query_as(const_format::concatcp!(
        "insert into alert_rule (id, user_id, filter, channels, digest, active) \
         values ($1, $2, $3, $4, $5, $6) returning ",
        RULE_COLUMNS
    ))
    .bind(ulid::Ulid::new().to_string())
    .bind(&spec.user_id)
    .bind(filter)
    .bind(channels)
    .bind(spec.digest)
    .bind(spec.active)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(row.try_into()?)))
}

/// Lists alert rules (optionally one user's).
///
/// # Errors
/// `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/alert-rules",
    tag = "alert-rules",
    params(ListAlertRulesParams),
    responses(
        (status = 200, description = "All matching rules", body = [AlertRule]),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_alert_rules(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ListAlertRulesParams>,
) -> Result<Json<Vec<AlertRule>>, ApiError> {
    let rows: Vec<AlertRuleRow> = sqlx::query_as(const_format::concatcp!(
        "select ",
        RULE_COLUMNS,
        " from alert_rule where ($1::text is null or user_id = $1) order by id"
    ))
    .bind(&params.user_id)
    .fetch_all(&state.pool)
    .await?;
    let rules = rows
        .into_iter()
        .map(AlertRule::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(rules))
}

/// Replaces an alert rule.
///
/// # Errors
/// `400` on a body outside the contracts; `404` for an unknown rule; `500`
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
        (status = 404, description = "Unknown rule", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn update_alert_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ApiJson(spec): ApiJson<AlertRuleSpec>,
) -> Result<Json<AlertRule>, ApiError> {
    let (filter, channels) = validate_spec(&spec)?;
    let row: Option<AlertRuleRow> = sqlx::query_as(const_format::concatcp!(
        "update alert_rule \
         set user_id = $2, filter = $3, channels = $4, digest = $5, active = $6, \
             updated_at = now() \
         where id = $1 returning ",
        RULE_COLUMNS
    ))
    .bind(&id)
    .bind(&spec.user_id)
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

/// Deletes an alert rule (its delivery ledger rows cascade with it).
///
/// # Errors
/// `404` for an unknown rule; `500` on backend failure.
#[utoipa::path(
    delete,
    path = "/v1/alert-rules/{id}",
    tag = "alert-rules",
    params(("id" = String, Path, description = "Rule ULID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown rule", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn delete_alert_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query("delete from alert_rule where id = $1")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound {
            message: format!("alert rule {id} not found"),
        });
    }
    Ok(StatusCode::NO_CONTENT)
}
