//! `POST /v1/stripe/webhook` — the inbound half of the Stripe seam (goal
//! 050, design §6.4). Signature-verified (`core::stripe`, canned-payload
//! tested; no Stripe credentials on hosts), then the minimal mirror:
//! `customer.subscription.*` events upsert the `subscription` row and keep
//! `user_account.tier` honest (metadata `govfolio_tier` names the tier the
//! founder's Stripe price is configured to grant; a lapsed subscription
//! downgrades to free and deactivates the user's alert rules — alerts are a
//! paid capability, design §6.2).
//!
//! Config-gated: without `STRIPE_WEBHOOK_SECRET` the endpoint is `503` (fail
//! closed — an unverifiable event is never acted on). Unknown customers and
//! event kinds are acknowledged and ignored (Stripe retries non-2xx
//! forever; a foreign-environment event is not an error).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use govfolio_core::domain::enums::Tier;
use govfolio_core::stripe::verify_and_parse;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

/// The slice of a Stripe subscription object the mirror needs.
#[derive(Debug, Deserialize)]
struct SubscriptionObject {
    /// `sub_...`
    id: String,
    /// `cus_...`
    customer: String,
    status: String,
    #[serde(default)]
    current_period_end: Option<i64>,
    #[serde(default)]
    metadata: serde_json::Value,
}

/// Receives Stripe webhook events (subscription lifecycle mirror).
///
/// # Errors
/// `503` when no webhook secret is configured; `400` on a missing/invalid
/// signature or an unparseable event; `500` on backend failure.
#[utoipa::path(
    post,
    path = "/v1/stripe/webhook",
    tag = "billing",
    request_body(content = String, description = "Raw Stripe event JSON (signature over the exact bytes)"),
    responses(
        (status = 200, description = "Event verified and processed (or acknowledged as not ours)"),
        (status = 400, description = "Missing/invalid Stripe-Signature or malformed event", body = ErrorBody),
        (status = 503, description = "Webhook secret not configured (fail closed)", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<StatusCode, ApiError> {
    let Some(secret) = state.config.stripe_webhook_secret.as_deref() else {
        return Err(ApiError::Unavailable {
            code: "stripe_unconfigured",
            message: "STRIPE_WEBHOOK_SECRET is not configured".to_owned(),
        });
    };
    let signature = headers
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::bad_request("invalid_signature", "Stripe-Signature header required")
        })?;
    let event = verify_and_parse(secret, signature, &body, Utc::now().timestamp())
        .map_err(|e| ApiError::bad_request("invalid_signature", e.to_string()))?;

    if !event.kind.starts_with("customer.subscription.") {
        return Ok(StatusCode::OK); // acknowledged, nothing to mirror
    }
    let object: SubscriptionObject = serde_json::from_value(event.data.object)
        .map_err(|e| ApiError::bad_request("invalid_event", format!("subscription object: {e}")))?;

    let user_id: Option<String> =
        sqlx::query_scalar("select id from user_account where stripe_customer_id = $1")
            .bind(&object.customer)
            .fetch_optional(&state.pool)
            .await?;
    let Some(user_id) = user_id else {
        // Not one of ours (other environment, deleted account): acknowledge
        // so Stripe stops retrying; nothing is mirrored.
        eprintln!(
            "stripe webhook {}: no account for customer {} — ignored",
            event.id, object.customer
        );
        return Ok(StatusCode::OK);
    };

    let period_end: Option<DateTime<Utc>> = object
        .current_period_end
        .and_then(|secs| DateTime::from_timestamp(secs, 0));
    sqlx::query(
        "insert into subscription \
           (id, user_id, stripe_subscription_id, status, current_period_end) \
         values ($1, $2, $3, $4, $5) \
         on conflict (stripe_subscription_id) do update \
           set status = excluded.status, \
               current_period_end = excluded.current_period_end, \
               updated_at = now()",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&user_id)
    .bind(&object.id)
    .bind(&object.status)
    .bind(period_end)
    .execute(&state.pool)
    .await?;

    let lapsed = event.kind == "customer.subscription.deleted"
        || matches!(
            object.status.as_str(),
            "canceled" | "unpaid" | "incomplete_expired"
        );
    if lapsed {
        sqlx::query("update user_account set tier = 'free' where id = $1")
            .bind(&user_id)
            .execute(&state.pool)
            .await?;
        // Alerts are a paid capability (design §6.2): a lapsed subscription
        // stops the fast path too, not just the API freshness.
        sqlx::query("update alert_rule set active = false, updated_at = now() where user_id = $1")
            .bind(&user_id)
            .execute(&state.pool)
            .await?;
    } else if matches!(object.status.as_str(), "active" | "trialing") {
        // The granted tier travels as subscription metadata (set when the
        // checkout session is created); only paid tiers are grantable.
        let granted = object
            .metadata
            .get("govfolio_tier")
            .and_then(serde_json::Value::as_str)
            .and_then(|token| {
                serde_json::from_value::<Tier>(serde_json::Value::String(token.to_owned())).ok()
            })
            .filter(|tier| tier.realtime());
        if let Some(tier) = granted {
            let token = govfolio_core::query::wire_token(&tier)
                .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
            sqlx::query("update user_account set tier = $2 where id = $1")
                .bind(&user_id)
                .bind(token)
                .execute(&state.pool)
                .await?;
        } else {
            eprintln!(
                "stripe webhook {}: subscription {} active without a valid govfolio_tier \
                 metadata value — tier left unchanged",
                event.id, object.id
            );
        }
    }
    Ok(StatusCode::OK)
}
