//! Goal 050 tier matrix (design §6.2 — the freemium boundary, mechanically):
//! free sees NOTHING younger than 24h and everything older; pro/data see
//! real time; unauthenticated behaves as free; quotas meter through
//! `usage_event` (429 over quota); key revocation is immediate; the delay
//! holds on the timeline and detail doors too, not just the listing; the
//! Stripe webhook mirror flips tiers with signed events only.
//!
//! Seeding is direct SQL (not the pipeline): the matrix needs exact control
//! of `filing.discovered_at` — one filing discovered 25h ago (visible to
//! free) and one discovered minutes ago (paid tiers only).
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

use api::ApiConfig;

const ADMIN_TOKEN: &str = "test-admin-token-050";
const STRIPE_SECRET: &str = "whsec_test_050";

fn test_config() -> ApiConfig {
    ApiConfig {
        admin_token: Some(ADMIN_TOKEN.to_owned()),
        stripe_webhook_secret: Some(STRIPE_SECRET.to_owned()),
        ..ApiConfig::new()
    }
}

fn test_app(pool: &PgPool) -> Router {
    api::app(pool.clone(), test_config())
}

// ------------------------------------------------------------------ http --

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = match body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("{method} {uri} returned non-JSON ({status}): {e}"))
    };
    (status, body)
}

async fn get_as(app: &Router, key: &str, uri: &str) -> (StatusCode, Value) {
    let bearer = format!("Bearer {key}");
    request(app, "GET", uri, &[("authorization", &bearer)], None).await
}

async fn get_anon(app: &Router, uri: &str) -> (StatusCode, Value) {
    request(app, "GET", uri, &[], None).await
}

fn item_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

// -------------------------------------------------------------- accounts --

/// Creates a user + key through the real admin bootstrap endpoints (the
/// plaintext key is only ever available here — hash-at-rest is asserted
/// separately).
async fn user_with_key(app: &Router, email: &str, tier: &str) -> (String, String, String) {
    let (status, user) = request(
        app,
        "POST",
        "/v1/users",
        &[("x-admin-token", ADMIN_TOKEN)],
        Some(&json!({"email": email, "tier": tier})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create user: {user:#}");
    let user_id = user["id"].as_str().unwrap().to_owned();
    let (status, created) = request(
        app,
        "POST",
        "/v1/keys",
        &[("x-admin-token", ADMIN_TOKEN)],
        Some(&json!({"user_id": user_id, "label": "test"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create key: {created:#}");
    let key = created["key"].as_str().unwrap().to_owned();
    assert!(key.starts_with("gfk_"), "keys are gfk_-prefixed: {key}");
    let key_id = created["id"].as_str().unwrap().to_owned();
    (user_id, key, key_id)
}

// --------------------------------------------------------------- seeding --

struct Seed {
    politician: String,
    old_record: String,
    fresh_record: String,
}

/// Two records for one politician: one on a filing discovered 25h ago
/// (past the free delay) and one discovered a minute ago (paid only).
async fn seed_two_ages(pool: &PgPool) -> Seed {
    govfolio_core::db::migrate(pool).await.unwrap();
    let politician_id = ulid::Ulid::new().to_string();
    let regime_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "insert into jurisdiction (id, name, level) values ('us', 'United States', 'national')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_regime \
           (id, jurisdiction_id, body, regime_type, value_precision, effective_from) \
         values ($1, 'us', 'US House', 'transaction_report', 'banded', '2012-01-01')",
    )
    .bind(&regime_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("insert into politician (id, canonical_name) values ($1, 'Test Person')")
        .bind(&politician_id)
        .execute(pool)
        .await
        .unwrap();

    let insert_record = async |age: Duration, tag: &str| -> String {
        let raw_id = ulid::Ulid::new().to_string();
        let filing_id = ulid::Ulid::new().to_string();
        let record_id = ulid::Ulid::new().to_string();
        sqlx::query(
            "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
             values ($1, $2, $3, 'application/pdf', now())",
        )
        .bind(&raw_id)
        .bind(format!("file:///bronze/{tag}.pdf"))
        .bind(format!("{tag}-sha256"))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into filing \
               (id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
                discovered_at) \
             values ($1, $2, $3, $4, $5, 'ptr', $6)",
        )
        .bind(&filing_id)
        .bind(&regime_id)
        .bind(&politician_id)
        .bind(&raw_id)
        .bind(tag)
        .bind(Utc::now() - age)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into disclosure_record \
               (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
                asset_class, side, transaction_date, extracted_by, fingerprint) \
             values ($1, $2, $3, $4, $5, 'transaction', 'equity', 'buy', '2026-06-01', \
                     'tiers-test', $6)",
        )
        .bind(&record_id)
        .bind(&filing_id)
        .bind(&politician_id)
        .bind(&regime_id)
        .bind(format!("{tag} asset"))
        .bind(format!("fp-{tag}"))
        .execute(pool)
        .await
        .unwrap();
        record_id
    };

    let old_record_id = insert_record(Duration::hours(25), "old").await;
    let fresh_record_id = insert_record(Duration::minutes(1), "fresh").await;
    Seed {
        politician: politician_id,
        old_record: old_record_id,
        fresh_record: fresh_record_id,
    }
}

// ------------------------------------------------------------------ tests --

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_free_sees_zero_fresh_records_pro_sees_realtime(pool: PgPool) {
    let seed = seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (_, free_key, _) = user_with_key(&app, "free@test", "free").await;
    let (_, pro_key, _) = user_with_key(&app, "pro@test", "pro").await;
    let (_, data_key, _) = user_with_key(&app, "data@test", "data").await;

    // Free key: the fresh record does not exist yet; the old one does.
    let (status, page) = get_as(&app, &free_key, "/v1/records").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        item_ids(&page),
        vec![seed.old_record.clone()],
        "free tier must see ZERO records younger than 24h and ALL older ones"
    );

    // Unauthenticated behaves exactly as free.
    let (status, page) = get_anon(&app, "/v1/records").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&page), vec![seed.old_record.clone()]);

    // Pro and data keys: real time.
    for key in [&pro_key, &data_key] {
        let (status, page) = get_as(&app, key, "/v1/records").await;
        assert_eq!(status, StatusCode::OK);
        let mut ids = item_ids(&page);
        ids.sort();
        let mut expected = vec![seed.old_record.clone(), seed.fresh_record.clone()];
        expected.sort();
        assert_eq!(ids, expected, "paid tiers are real-time");
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_delay_holds_on_timeline_detail_and_profile_summary(pool: PgPool) {
    let seed = seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (_, free_key, _) = user_with_key(&app, "free@test", "free").await;
    let (_, pro_key, _) = user_with_key(&app, "pro@test", "pro").await;

    // Timeline: same statement, same delay.
    let timeline = format!("/v1/politicians/{}/records", seed.politician);
    let (status, page) = get_as(&app, &free_key, &timeline).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&page), vec![seed.old_record.clone()]);
    let (_, page) = get_as(&app, &pro_key, &timeline).await;
    assert_eq!(item_ids(&page).len(), 2);
    let (_, page) = get_anon(&app, &timeline).await;
    assert_eq!(item_ids(&page), vec![seed.old_record.clone()]);

    // Detail: a fresh record is 404 for free/anonymous, 200 for pro.
    let fresh_detail = format!("/v1/records/{}", seed.fresh_record);
    let (status, body) = get_as(&app, &free_key, &fresh_detail).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "free detail leak: {body:#}");
    assert_eq!(body["error"]["code"], json!("not_found"));
    let (status, _) = get_anon(&app, &fresh_detail).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get_as(&app, &pro_key, &fresh_detail).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get_as(&app, &free_key, &format!("/v1/records/{}", seed.old_record)).await;
    assert_eq!(status, StatusCode::OK, "old records stay visible to free");

    // Profile summary counts flow through the same evaluator.
    let profile = format!("/v1/politicians/{}", seed.politician);
    let (_, body) = get_as(&app, &free_key, &profile).await;
    assert_eq!(body["records"]["count"], json!(1), "free summary");
    let (_, body) = get_as(&app, &pro_key, &profile).await;
    assert_eq!(body["records"]["count"], json!(2), "pro summary");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_quota_exhaustion_is_429_with_correct_usage_ledger(pool: PgPool) {
    let seed = seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (user_id, free_key, _) = user_with_key(&app, "quota@test", "free").await;

    // Free daily quota is 60 (design §6.2). Admin-authenticated bootstrap
    // calls carry no key, so the ledger starts empty.
    let mut served = 0;
    let final_body = loop {
        let (status, body) = get_as(&app, &free_key, "/v1/records").await;
        match status {
            StatusCode::OK => served += 1,
            StatusCode::TOO_MANY_REQUESTS => break body,
            other => panic!("unexpected status {other}: {body:#}"),
        }
        assert!(served <= 60, "quota never enforced");
    };
    assert_eq!(served, 60, "exactly the free quota is served");
    assert_eq!(final_body["error"]["code"], json!("quota_exceeded"));
    assert!(final_body["error"]["message"].is_string());

    // The ledger counts exactly the SERVED requests (the 429 added no row),
    // each labeled with the endpoint.
    let (rows, distinct_endpoint): (i64, i64) = sqlx::query_as(
        "select count(*), count(distinct endpoint) from usage_event where user_id = $1",
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 60, "usage_event rows == served requests");
    assert_eq!(distinct_endpoint, 1, "all rows labeled /v1/records");

    // Quota exhaustion still answers 429 on other doors too.
    let (status, _) = get_as(
        &app,
        &free_key,
        &format!("/v1/politicians/{}", seed.politician),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_key_lifecycle_masked_listing_and_immediate_revocation(pool: PgPool) {
    seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (_, key, key_id) = user_with_key(&app, "keys@test", "free").await;

    // Plaintext never at rest: only the sha256 hex is stored.
    let stored: Vec<String> = sqlx::query_scalar("select key_hash from api_key")
        .fetch_all(&pool)
        .await
        .unwrap();
    for hash in &stored {
        assert_eq!(hash.len(), 64);
        assert!(!key.contains(hash.as_str()) && !hash.contains(&key[4..]));
    }

    // The listing is masked: metadata only, no key material of any form.
    let (status, list) = get_as(&app, &key, "/v1/keys").await;
    assert_eq!(status, StatusCode::OK);
    let entry = &list.as_array().unwrap()[0];
    assert_eq!(entry["id"], json!(key_id.clone()));
    assert_eq!(entry["label"], json!("test"));
    assert!(entry.get("key").is_none() && entry.get("key_hash").is_none());

    // Self-revocation is immediate: the very next request is 401.
    let bearer = format!("Bearer {key}");
    let (status, _) = request(
        &app,
        "DELETE",
        &format!("/v1/keys/{key_id}"),
        &[("authorization", &bearer)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = get_as(&app, &key, "/v1/records").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revocation must be immediate"
    );
    assert_eq!(body["error"]["code"], json!("invalid_key"));

    // A made-up key is 401 too — never silently downgraded to anonymous.
    let (status, body) = get_as(&app, &format!("gfk_{}", "0".repeat(64)), "/v1/records").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], json!("invalid_key"));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_admin_bootstrap_gate_fails_closed(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();

    // Wrong/missing token: 401 with stable codes.
    let app = test_app(&pool);
    let body = json!({"email": "x@test", "tier": "free"});
    let (status, out) = request(&app, "POST", "/v1/users", &[], Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(out["error"]["code"], json!("admin_token_required"));
    let (status, out) = request(
        &app,
        "POST",
        "/v1/users",
        &[("x-admin-token", "wrong")],
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(out["error"]["code"], json!("invalid_admin_token"));

    // No ADMIN_TOKEN configured = the surface does not exist at all.
    let unconfigured = api::app(
        pool.clone(),
        ApiConfig {
            admin_token: None,
            ..test_config()
        },
    );
    let (status, out) = request(
        &unconfigured,
        "POST",
        "/v1/users",
        &[("x-admin-token", ADMIN_TOKEN)],
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(out["error"]["code"], json!("admin_disabled"));

    // The review surface (real-time record context) sits behind the same
    // gate — the delay has no back door.
    let (status, _) = get_anon(&app, "/v1/review-tasks").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = request(
        &app,
        "GET",
        "/v1/review-tasks",
        &[("x-admin-token", ADMIN_TOKEN)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_alert_rules_are_paid_only_and_account_scoped(pool: PgPool) {
    seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (_, free_key, _) = user_with_key(&app, "free@test", "free").await;
    let (pro_user, pro_key, _) = user_with_key(&app, "pro@test", "pro").await;
    let (_, other_key, _) = user_with_key(&app, "other@test", "data").await;

    let spec = json!({
        "filter": {},
        "channels": [{"type": "webhook", "url": "https://example.test/hook", "secret": "s"}],
    });
    // Anonymous: 401. Free: 403 (alerts are the paid fast path, §6.2).
    let (status, _) = request(&app, "POST", "/v1/alert-rules", &[], Some(&spec)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let bearer_free = format!("Bearer {free_key}");
    let (status, body) = request(
        &app,
        "POST",
        "/v1/alert-rules",
        &[("authorization", &bearer_free)],
        Some(&spec),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], json!("tier_required"));

    // Pro: created, owned by the authenticated account (never body-supplied).
    let bearer_pro = format!("Bearer {pro_key}");
    let (status, rule) = request(
        &app,
        "POST",
        "/v1/alert-rules",
        &[("authorization", &bearer_pro)],
        Some(&spec),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{rule:#}");
    assert_eq!(rule["user_id"], json!(pro_user.clone()));
    let rule_id = rule["id"].as_str().unwrap().to_owned();

    // Listing and mutation are account-scoped: another account sees nothing
    // and cannot delete the rule (404, not 403 — no existence leak).
    let (_, list) = get_as(&app, &other_key, "/v1/alert-rules").await;
    assert_eq!(list.as_array().unwrap().len(), 0);
    let bearer_other = format!("Bearer {other_key}");
    let (status, _) = request(
        &app,
        "DELETE",
        &format!("/v1/alert-rules/{rule_id}"),
        &[("authorization", &bearer_other)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = get_as(&app, &pro_key, "/v1/alert-rules").await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_anonymous_backstop_limits_per_ip(pool: PgPool) {
    seed_two_ages(&pool).await;
    let app = api::app(
        pool.clone(),
        ApiConfig {
            unauth_requests_per_minute: 3,
            ..test_config()
        },
    );
    for _ in 0..3 {
        let (status, _) = request(
            &app,
            "GET",
            "/v1/records",
            &[("x-forwarded-for", "203.0.113.9")],
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, body) = request(
        &app,
        "GET",
        "/v1/records",
        &[("x-forwarded-for", "203.0.113.9")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], json!("rate_limited"));
    // Another IP is unaffected (per-IP buckets).
    let (status, _) = request(
        &app,
        "GET",
        "/v1/records",
        &[("x-forwarded-for", "203.0.113.10")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_admin_token_requests_bypass_the_anonymous_backstop(pool: PgPool) {
    seed_two_ages(&pool).await;
    let app = api::app(
        pool.clone(),
        ApiConfig {
            unauth_requests_per_minute: 1,
            ..test_config()
        },
    );
    // A VALID admin token is exempt from the anonymous backstop: the ops
    // dashboard polls every 15s and must never 429 (nor starve the shared
    // bucket SSR anonymous traffic lives in). Limit is 1; four requests
    // through one IP all answer 200.
    for _ in 0..4 {
        let (status, body) = request(
            &app,
            "GET",
            "/v1/admin/overview",
            &[
                ("x-admin-token", ADMIN_TOKEN),
                ("x-forwarded-for", "203.0.113.21"),
            ],
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:#}");
    }
    // Exempt means NOT COUNTED, not merely not-rejected: after four admin
    // requests from this IP, a plain anonymous request still has the whole
    // limit-of-1 budget (any increment above would have 429'd it).
    let (status, _) = request(
        &app,
        "GET",
        "/v1/records",
        &[("x-forwarded-for", "203.0.113.21")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Header PRESENCE alone must not exempt — that would make the backstop
    // trivially bypassable. A wrong token passes the limiter once (401 from
    // the admin gate), then the bucket catches the second attempt.
    let (status, body) = request(
        &app,
        "GET",
        "/v1/admin/overview",
        &[
            ("x-admin-token", "wrong-token"),
            ("x-forwarded-for", "203.0.113.22"),
        ],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], json!("invalid_admin_token"));
    let (status, body) = request(
        &app,
        "GET",
        "/v1/admin/overview",
        &[
            ("x-admin-token", "wrong-token"),
            ("x-forwarded-for", "203.0.113.22"),
        ],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], json!("rate_limited"));
    // Plain anonymous traffic is still limited exactly as before.
    let (status, _) = request(
        &app,
        "GET",
        "/v1/records",
        &[("x-forwarded-for", "203.0.113.23")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = request(
        &app,
        "GET",
        "/v1/records",
        &[("x-forwarded-for", "203.0.113.23")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], json!("rate_limited"));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn tiers_stripe_webhook_mirrors_subscriptions_with_signed_events_only(pool: PgPool) {
    let seed = seed_two_ages(&pool).await;
    let app = test_app(&pool);
    let (user_id, key, _) = user_with_key(&app, "billing@test", "free").await;
    sqlx::query("update user_account set stripe_customer_id = 'cus_050' where id = $1")
        .bind(&user_id)
        .execute(&pool)
        .await
        .unwrap();

    let subscription_event = |kind: &str, status: &str| {
        json!({
            "id": "evt_050",
            "type": kind,
            "data": {"object": {
                "id": "sub_050",
                "customer": "cus_050",
                "status": status,
                "current_period_end": 1_790_000_000,
                "metadata": {"govfolio_tier": "pro"},
            }},
        })
        .to_string()
    };
    let post_signed = |payload: String, secret: &'static str| {
        let app = app.clone();
        async move {
            let header =
                govfolio_core::stripe::signature_header(secret, Utc::now().timestamp(), &payload);
            let request = Request::builder()
                .method("POST")
                .uri("/v1/stripe/webhook")
                .header("stripe-signature", header)
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap();
            let response = app.oneshot(request).await.unwrap();
            response.status()
        }
    };

    // Before: free tier — fresh record invisible.
    let fresh_detail = format!("/v1/records/{}", seed.fresh_record);
    let (status, _) = get_as(&app, &key, &fresh_detail).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A signed activation flips the tier and mirrors the subscription.
    let status = post_signed(
        subscription_event("customer.subscription.updated", "active"),
        STRIPE_SECRET,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tier: String = sqlx::query_scalar("select tier from user_account where id = $1")
        .bind(&user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tier, "pro");
    let (sub_status, has_period_end): (String, bool) = sqlx::query_as(
        "select status, current_period_end is not null from subscription \
         where stripe_subscription_id = 'sub_050' and user_id = $1",
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sub_status, "active");
    assert!(has_period_end);
    // ... and the upgrade is effective immediately on the API.
    let (status, _) = get_as(&app, &key, &fresh_detail).await;
    assert_eq!(status, StatusCode::OK, "upgrade applies in real time");

    // A rule created while pro is deactivated when the subscription lapses.
    sqlx::query(
        "insert into alert_rule (id, user_id, filter, channels) \
         values ($1, $2, '{}'::jsonb, '[]'::jsonb)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&user_id)
    .execute(&pool)
    .await
    .unwrap();
    let status = post_signed(
        subscription_event("customer.subscription.deleted", "canceled"),
        STRIPE_SECRET,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tier: String = sqlx::query_scalar("select tier from user_account where id = $1")
        .bind(&user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tier, "free", "lapsed subscription downgrades");
    let active_rules: i64 =
        sqlx::query_scalar("select count(*) from alert_rule where user_id = $1 and active")
            .bind(&user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_rules, 0, "alerts stop when the subscription lapses");

    // Unsigned/badly signed events change NOTHING (fail closed).
    let status = post_signed(
        subscription_event("customer.subscription.updated", "active"),
        "whsec_wrong",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let tier: String = sqlx::query_scalar("select tier from user_account where id = $1")
        .bind(&user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tier, "free", "a bad signature must never mutate state");

    // No configured secret = 503, fail closed.
    let unconfigured = api::app(
        pool.clone(),
        ApiConfig {
            stripe_webhook_secret: None,
            ..test_config()
        },
    );
    let (status, body) = request(
        &unconfigured,
        "POST",
        "/v1/stripe/webhook",
        &[],
        Some(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], json!("stripe_unconfigured"));
}
