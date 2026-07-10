//! Bearer API-key auth, tier resolution and metering (goal 050, design
//! §6.2/§6.4).
//!
//! - Keys are `gfk_<64 hex>` bearer tokens; at rest only their SHA-256 hex
//!   lives in `api_key.key_hash` (plaintext exists exactly once, in the
//!   creation response). Revocation is immediate: resolution requires
//!   `revoked_at is null`.
//! - Every request gets an [`AuthContext`] extension: `key -> user -> tier`.
//!   No key = the free tier (public browse stays open, design §6.2); an
//!   invalid or revoked key is `401`, never silently free (fail closed).
//! - THE 24h delay: [`AuthContext::apply`] / [`AuthContext::filter`] are the
//!   only producers of record filters in this crate — they stamp the
//!   internal `max_discovered_at` bound (`core::query`, the ONE evaluator)
//!   from the tier, so every record-serving route enforces the freemium
//!   boundary by construction.
//! - Metering: one `usage_event` row per authenticated request (Postgres
//!   counters at launch, design §6.4; Redis is the documented, unbuilt
//!   upgrade path). The same rows are the daily-quota counter AND the
//!   billing-sync aggregation source. Over quota = `429` in the envelope,
//!   no row inserted (quota rows == served requests).
//! - Unauthenticated traffic gets a light per-IP, per-minute in-memory
//!   limiter only. Reality check: authoritative anonymous limiting happens
//!   at the CDN edge (design §6.4 "coarse limits at Cloudflare"); this
//!   backstop is per-instance, resets on deploy, and the ceiling is generous
//!   because the SSR origin funnels many visitors through one IP. Requests
//!   carrying a VALID `X-Admin-Token` skip the backstop entirely (goal 095):
//!   the operator dashboard polls every 15s through that same SSR funnel.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest as _, Sha256};

use govfolio_core::domain::enums::Tier;
use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::error::ApiError;

/// Free-tier daily request quota (design §6.2 freemium table: "60 req/day,
/// delayed").
pub const FREE_DAILY_REQUESTS: i64 = 60;
/// Pro-tier daily request quota. TODO(founder): product knob — the final
/// ceiling is a pricing decision (human lane); this is a working default.
pub const PRO_DAILY_REQUESTS: i64 = 5_000;
/// Data-tier daily ceiling. Usage is metered (billed per request via
/// `usage_event` -> Stripe), so this is an abuse backstop, not a quota.
/// TODO(founder): product knob.
pub const DATA_DAILY_REQUESTS: i64 = 100_000;

/// The free tier's freshness delay (design §6.2: THE monetization lever).
pub const FREE_DELAY_HOURS: i64 = 24;

/// How the caller authenticated: the resolved key and its owner.
#[derive(Debug, Clone)]
pub struct Principal {
    /// Owning `user_account.id`.
    pub user_id: String,
    /// The presenting `api_key.id`.
    pub api_key_id: String,
}

/// Per-request identity: tier (always) + principal (when a key was
/// presented). Inserted into request extensions by [`authenticate`].
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Effective tier — `Free` for anonymous traffic.
    pub tier: Tier,
    /// The authenticated key, if any.
    pub principal: Option<Principal>,
}

impl AuthContext {
    /// Anonymous = the free tier (design §6.2: everything is free; immediacy
    /// is the product).
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            tier: Tier::Free,
            principal: None,
        }
    }

    /// Stamps the tier's visibility bound onto a filter — the ONE door every
    /// record-serving query in this crate goes through (grep-proof: routes
    /// never build a record filter any other way).
    #[must_use]
    pub fn apply(&self, filter: RecordFilter) -> RecordFilter {
        filter.with_max_discovered_at(visibility_bound(self.tier, Utc::now()))
    }

    /// [`Self::apply`] over an empty filter — for routes without caller
    /// filters (detail, timeline, summaries).
    #[must_use]
    pub fn filter(&self) -> RecordFilter {
        self.apply(RecordFilter::default())
    }
}

/// The tier's record-visibility bound: free sees only filings we discovered
/// at least [`FREE_DELAY_HOURS`] ago (`filing.discovered_at` — our knowledge
/// time, the honest clock we can actually promise on); pro/data are
/// real-time. Pure so the boundary math is unit-testable.
#[must_use]
pub fn visibility_bound(tier: Tier, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if tier.realtime() {
        None
    } else {
        Some(now - Duration::hours(FREE_DELAY_HOURS))
    }
}

/// `sha256` hex of a presented token — the only representation at rest.
#[must_use]
pub fn hash_key(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Mints a fresh API key: `gfk_` + 64 hex chars of 32 CSPRNG bytes. Shown
/// exactly once (the creation response); only its hash is stored.
#[must_use]
pub fn generate_key() -> String {
    let bytes: [u8; 32] = rand::random();
    let mut out = String::with_capacity(4 + 64);
    out.push_str("gfk_");
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Verifies the `X-Admin-Token` header against the configured bootstrap
/// token. Fail closed: no configured token = the admin surface does not
/// exist (`401`), never "open until configured". Comparison is over SHA-256
/// digests, so timing reveals nothing about the token bytes.
///
/// # Errors
/// [`ApiError`] `401` with a stable code.
pub fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state.config.admin_token.as_deref() else {
        return Err(ApiError::unauthorized(
            "admin_disabled",
            "the admin surface is disabled (ADMIN_TOKEN is not configured)",
        ));
    };
    let presented = headers
        .get("x-admin-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("admin_token_required", "X-Admin-Token required"))?;
    if Sha256::digest(presented.as_bytes()) != Sha256::digest(expected.as_bytes()) {
        return Err(ApiError::unauthorized(
            "invalid_admin_token",
            "X-Admin-Token does not match",
        ));
    }
    Ok(())
}

/// Route-layer admin gate for the reviewer surface (design §7.2): review
/// tasks expose record context in real time, so the whole subtree sits
/// behind the bootstrap admin token (the freemium delay must not leak
/// through the back door).
pub async fn admin_gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Err(err) = require_admin(&state, request.headers()) {
        return err.into_response();
    }
    next.run(request).await
}

/// Daily request quota per tier (UTC calendar day).
#[must_use]
pub fn daily_quota(tier: Tier) -> i64 {
    match tier {
        Tier::Free => FREE_DAILY_REQUESTS,
        Tier::Pro => PRO_DAILY_REQUESTS,
        Tier::Data => DATA_DAILY_REQUESTS,
    }
}

/// Per-IP per-minute counters for anonymous traffic (see module docs for
/// why this is only a backstop).
pub type UnauthCounters = Arc<Mutex<HashMap<(String, i64), u32>>>;

/// Keeps the counter map bounded: when it grows past this, stale minutes are
/// pruned on the next hit.
const COUNTER_PRUNE_THRESHOLD: usize = 10_000;

/// The auth + metering middleware: resolves the key, enforces the quota,
/// records the usage event and stamps [`AuthContext`] into extensions.
pub async fn authenticate(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Owned slices up front: holding `&Request` across the awaits below
    // would make the future !Send (the body is !Sync).
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let forwarded_for = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let path = request.uri().path().to_owned();
    // A VALID admin token exempts the request from the anonymous backstop
    // (mirroring how Bearer keys use the DB quota instead): the operator
    // dashboard polls every 15s and its SSR traffic shares one "direct"
    // bucket with all anonymous requests. Mere header presence must NOT
    // exempt — that would make the backstop trivially bypassable — so the
    // token is validated here first. `admin_gate` still re-validates on the
    // admin subtree; on public routes a valid token merely skips the
    // backstop (the token holder is the operator).
    let admin_exempt = require_admin(&state, request.headers()).is_ok();
    let context = match resolve(&state, authorization, forwarded_for, path, admin_exempt).await {
        Ok(context) => context,
        Err(err) => return err.into_response(),
    };
    request.extensions_mut().insert(context);
    next.run(request).await
}

async fn resolve(
    state: &AppState,
    authorization: Option<String>,
    forwarded_for: Option<String>,
    path: String,
    admin_exempt: bool,
) -> Result<AuthContext, ApiError> {
    let Some(authorization) = authorization else {
        // Skipped entirely (not even incremented) for validated admin
        // tokens, so admin polling cannot starve the shared bucket.
        if !admin_exempt {
            limit_anonymous(state, forwarded_for.as_deref())?;
        }
        return Ok(AuthContext::anonymous());
    };
    let token = authorization
        .strip_prefix("Bearer ")
        .filter(|token| token.starts_with("gfk_"))
        .ok_or_else(|| {
            ApiError::unauthorized(
                "invalid_authorization",
                "expected `Authorization: Bearer gfk_...`",
            )
        })?;

    let row: Option<(String, String, String)> = sqlx::query_as(
        "select k.id, k.user_id, u.tier \
         from api_key k join user_account u on u.id = k.user_id \
         where k.key_hash = $1 and k.revoked_at is null",
    )
    .bind(hash_key(token))
    .fetch_optional(&state.pool)
    .await?;
    // Unknown/revoked keys are 401, never silently downgraded to free: a
    // revoked key that kept browsing would make revocation meaningless.
    let (api_key_id, user_id, tier_token) =
        row.ok_or_else(|| ApiError::unauthorized("invalid_key", "unknown or revoked API key"))?;
    let tier: Tier = crate::dto::from_token(tier_token, "tier")?;

    let used_today: i64 = sqlx::query_scalar(
        "select count(*) from usage_event \
         where user_id = $1 and occurred_at >= date_trunc('day', now())",
    )
    .bind(&user_id)
    .fetch_one(&state.pool)
    .await?;
    let quota = daily_quota(tier);
    if used_today >= quota {
        return Err(ApiError::too_many_requests(
            "quota_exceeded",
            format!("daily quota of {quota} requests exhausted; resets at UTC midnight"),
        ));
    }
    // Metering row = quota counter = billing source (one ledger, three uses).
    // Labeled with the raw request path (bounded: our own /v1 surface).
    sqlx::query(
        "insert into usage_event (id, user_id, api_key_id, endpoint) values ($1, $2, $3, $4)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&user_id)
    .bind(&api_key_id)
    .bind(&path)
    .execute(&state.pool)
    .await?;

    Ok(AuthContext {
        tier,
        principal: Some(Principal {
            user_id,
            api_key_id,
        }),
    })
}

/// Anonymous backstop limiter: per-IP, per-UTC-minute, in-memory.
fn limit_anonymous(state: &AppState, forwarded_for: Option<&str>) -> Result<(), ApiError> {
    // Behind the CDN/proxy the client is the first X-Forwarded-For hop;
    // direct connections collapse to one bucket, which the generous ceiling
    // absorbs.
    let ip = forwarded_for
        .and_then(|value| value.split(',').next())
        .map_or_else(|| "direct".to_owned(), |ip| ip.trim().to_owned());
    let minute = Utc::now().timestamp() / 60;
    let Ok(mut counters) = state.unauth_counters.lock() else {
        // A poisoned lock means a panic elsewhere; failing open on the
        // BACKSTOP (the CDN limit still stands) beats 500-ing all anonymous
        // traffic.
        return Ok(());
    };
    if counters.len() > COUNTER_PRUNE_THRESHOLD {
        counters.retain(|(_, m), _| *m == minute);
    }
    let hits = counters.entry((ip, minute)).or_insert(0);
    *hits += 1;
    if *hits > state.config.unauth_requests_per_minute {
        return Err(ApiError::too_many_requests(
            "rate_limited",
            "anonymous request rate exceeded; authenticate with an API key or retry shortly",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn tiers_key_format_and_hash_at_rest() {
        let key = generate_key();
        assert!(key.starts_with("gfk_"));
        assert_eq!(key.len(), 68);
        assert_ne!(key, generate_key(), "keys are random");
        let hash = hash_key(&key);
        assert_eq!(hash.len(), 64, "sha256 hex");
        assert_eq!(hash, hash_key(&key), "deterministic");
        assert!(!hash.contains(&key[4..]), "hash reveals nothing");
    }

    #[test]
    fn tiers_visibility_bound_is_24h_for_free_none_for_paid() {
        let now = Utc::now();
        let bound = visibility_bound(Tier::Free, now).unwrap();
        assert_eq!(now - bound, Duration::hours(24));
        assert_eq!(visibility_bound(Tier::Pro, now), None);
        assert_eq!(visibility_bound(Tier::Data, now), None);
    }

    #[test]
    fn tiers_daily_quota_matches_the_design_table() {
        assert_eq!(daily_quota(Tier::Free), 60, "design §6.2: 60 req/day");
        assert!(daily_quota(Tier::Pro) > daily_quota(Tier::Free));
        assert!(daily_quota(Tier::Data) > daily_quota(Tier::Pro));
    }
}
