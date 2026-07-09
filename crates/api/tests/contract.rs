//! Contract test (plan Task 10): boots the axum app on a test pool seeded by
//! the REAL Task 9 pipeline over the `us_house` fixtures, GETs `/v1/records`
//! and `/v1/politicians/{id}/records`, and validates every response body
//! against the emitted `OpenAPI` schema (`jsonschema`). Asserts ULID cursor
//! pagination (page 2 begins strictly after page 1's last id) and that
//! `verification_state` is present on EVERY record.
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::promote::{ResolveOutcome, Verdict, resolve_review_task};
use pipeline::run::{LocalFiling, Runner};
use pipeline::stages::roster::{resolve_politician, seed_roster};
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;

// ---------------------------------------------------------------- seeding --
// Same Task 9 machinery the pipeline e2e suite drives: migrate, seed the
// regime + roster from the archived index evidence slices, run the pipeline
// over the five committed fixtures (13 Gold rows, all `unverified`).

fn evidence_xml(file: &str) -> String {
    let path = workspace_root()
        .join("docs")
        .join("regimes")
        .join("us-house")
        .join("evidence")
        .join(file);
    std::fs::read_to_string(path).unwrap()
}

/// Four E1 slice members + the goal-021 scanned fixture's paper filer.
fn full_roster() -> Vec<pipeline::stages::roster::RosterMember> {
    let mut roster = us_house::seed::roster_from_index_xml(&evidence_xml(
        "94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e.2026FD-slice.xml",
    ))
    .unwrap();
    roster.extend(
        us_house::seed::roster_from_index_xml(&evidence_xml(
            "f312caf490ddb96fa4b2b4fc73cc67ad0eb335d004c9b4db82e3b48cd22b6bc7.2026FD-slice-9115811.xml",
        ))
        .unwrap(),
    );
    roster
}

fn fixture_inputs() -> Vec<LocalFiling> {
    let root = fixtures_dir("us_house");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| LocalFiling {
            path: dir.join("input.pdf"),
        })
        .collect()
}

fn temp_bronze_root() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("govfolio-contract-{}-{nanos}", std::process::id()))
}

async fn seed_via_pipeline(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
    seed_roster(pool, &us_house::seed::regime_binding(), &full_roster())
        .await
        .unwrap();

    let adapter = UsHouseAdapter::default();
    let binding = UsHouseBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(temp_bronze_root()).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx).unwrap();
    let report = runner.run_local(&fixture_inputs()).await.unwrap();
    assert_eq!(report.failed, Vec::<String>::new());
    assert_eq!(report.gold_inserted, 13, "1+8+2+1+1 fixture rows");

    // Age the seed past the free-tier 24h delay (goal 050): this suite
    // exercises the public read surface as an anonymous (= free) caller,
    // and freshly discovered filings would be invisible to it. The tier
    // matrix itself is proven in tests/tiers.rs.
    sqlx::query("update filing set discovered_at = discovered_at - interval '25 hours'")
        .execute(pool)
        .await
        .unwrap();
}

// ------------------------------------------------------------- app + auth --

/// Bootstrap admin token for this suite (the admin surface is disabled when
/// unset — goal 050 fail-closed gate).
const TEST_ADMIN: &str = "contract-suite-admin";

fn test_app(pool: &PgPool) -> Router {
    api::app(
        pool.clone(),
        api::ApiConfig {
            admin_token: Some(TEST_ADMIN.to_owned()),
            ..api::ApiConfig::new()
        },
    )
}

/// Creates a user + key through the admin bootstrap endpoints; returns the
/// plaintext bearer token (only ever available at creation, by design).
async fn seed_key(app: &Router, email: &str, tier: &str) -> String {
    let (status, user) = send(
        app,
        "POST",
        "/v1/users",
        Some(&json!({"email": email, "tier": tier})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create user: {user:#}");
    let (status, created) = send(
        app,
        "POST",
        "/v1/keys",
        Some(&json!({"user_id": user["id"], "label": "contract"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create key: {created:#}");
    created["key"].as_str().unwrap().to_owned()
}

// --------------------------------------------------------- schema harness --
// "Validate against the EMITTED OpenAPI schema": `api::openapi_json()` is the
// exact byte producer behind `packages/contracts/openapi.json` (the CI drift
// gate pins the committed copy to it).

fn openapi_doc() -> Value {
    serde_json::from_str(&api::openapi_json().unwrap()).unwrap()
}

/// Standalone JSON Schema for one operation's response: the response schema
/// node (a `$ref`) wrapped in `allOf`, with the doc's `components` carried
/// along so internal `#/components/schemas/...` pointers resolve.
fn response_schema(doc: &Value, method: &str, path: &str, status: &str) -> Value {
    let node =
        &doc["paths"][path][method]["responses"][status]["content"]["application/json"]["schema"];
    assert!(
        node.is_object(),
        "contract must declare a JSON response schema for {method} {path} {status}"
    );
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "allOf": [node.clone()],
        "components": doc["components"].clone(),
    })
}

fn validator_for(doc: &Value, path: &str, status: &str) -> jsonschema::Validator {
    validator_for_op(doc, "get", path, status)
}

fn validator_for_op(doc: &Value, method: &str, path: &str, status: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&response_schema(doc, method, path, status)).unwrap()
}

fn assert_valid(validator: &jsonschema::Validator, body: &Value) {
    let errors: Vec<String> = validator
        .iter_errors(body)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "response body violates the OpenAPI contract:\n{}\nbody: {body:#}",
        errors.join("\n")
    );
}

/// Raw GET with optional request headers — for probing response headers and
/// bodies the JSON helper would obscure (the ETag/304 path). Carries the
/// suite admin token (ignored by public endpoints; required by the review
/// surface since goal 050).
async fn get_raw(
    app: &Router,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Bytes) {
    let mut request = Request::get(uri).header("x-admin-token", TEST_ADMIN);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send_with(app, "GET", uri, &[], None).await
}

/// Sends a JSON-bodied request (POST/PUT/DELETE); `None` body for DELETE.
/// Returns `Value::Null` for empty response bodies (204). Carries the suite
/// admin token (see [`get_raw`]).
async fn send(app: &Router, method: &str, uri: &str, body: Option<&Value>) -> (StatusCode, Value) {
    send_with(app, method, uri, &[], body).await
}

/// `send` with extra headers (e.g. `Authorization: Bearer gfk_...`).
async fn send_with(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-admin-token", TEST_ADMIN);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        return (status, Value::Null);
    }
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("{method} {uri} returned non-JSON ({status}): {e}"));
    (status, body)
}

fn item_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Every record carries `verification_state` — the trust surface travels with
/// the data, on every page, no exceptions.
fn assert_verification_state_on_every_record(page: &Value) {
    for item in page["items"].as_array().unwrap() {
        assert_eq!(
            item["verification_state"].as_str(),
            Some("unverified"),
            "verification_state must be present on every record: {item:#}"
        );
    }
}

// ------------------------------------------------------------------ tests --

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn records_paginate_by_ulid_cursor_and_match_the_contract(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/records", "200");
    assert!(
        ok.validate(&json!({ "items": "not-an-array" })).is_err(),
        "the contract validator must have teeth (refs resolved, shape enforced)"
    );

    // Ground truth: all 13 record ids in ULID (= id) order.
    let all: Vec<String> = sqlx::query_scalar("select id from disclosure_record order by id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(all.len(), 13);

    // Page 1.
    let (status, page1) = get(&app, "/v1/records?limit=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page1);
    assert_verification_state_on_every_record(&page1);
    assert_eq!(item_ids(&page1), all[..5], "page 1 = first five ids");
    let cursor1 = page1["next_cursor"].as_str().unwrap();
    assert_eq!(cursor1, all[4], "cursor = last id of the page");

    // Page 2 begins strictly AFTER page 1's last id.
    let (status, page2) = get(&app, &format!("/v1/records?limit=5&cursor={cursor1}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page2);
    assert_verification_state_on_every_record(&page2);
    let ids2 = item_ids(&page2);
    assert_eq!(ids2, all[5..10]);
    assert!(
        ids2.iter().all(|id| id.as_str() > cursor1),
        "every page-2 id sorts after the cursor"
    );

    // Final page: remainder, and the cursor chain terminates.
    let cursor2 = page2["next_cursor"].as_str().unwrap();
    let (status, page3) = get(&app, &format!("/v1/records?limit=5&cursor={cursor2}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page3);
    assert_verification_state_on_every_record(&page3);
    assert_eq!(item_ids(&page3), all[10..]);
    assert!(
        page3["next_cursor"].is_null(),
        "an exhausted listing has no next_cursor"
    );

    // Minimal filters stay contract-typed: no verified rows exist yet.
    let (status, verified) = get(&app, "/v1/records?verification_state=verified").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &verified);
    assert_eq!(verified["items"].as_array().unwrap().len(), 0);

    // Money on the wire = decimal strings (invariant 7), straight from core types.
    let with_value = page1["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| !item["value"].is_null())
        .unwrap(); // fixtures carry banded values
    assert!(with_value["value"]["low"].is_string());

    // Errors use the consistent envelope and are contract-valid too.
    let err = validator_for(&doc, "/v1/records", "400");
    let (status, body) = get(&app, "/v1/records?cursor=not-a-ulid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_cursor");
    let (status, body) = get(&app, "/v1/records?limit=0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_limit");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn politician_timeline_pages_and_matches_the_contract(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/politicians/{id}/records";
    let ok = validator_for(&doc, path, "200");

    // The multi-row fixture filer (8 records).
    let politician_id = resolve_politician(
        &pool,
        &us_house::seed::regime_binding(),
        "Hon. Lloyd K. Smucker",
        "PA11",
    )
    .await
    .unwrap()
    .unwrap();
    let timeline: Vec<String> =
        sqlx::query_scalar("select id from disclosure_record where politician_id = $1 order by id")
            .bind(&politician_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(timeline.len(), 8, "the multi-row fixture publishes 8 rows");

    // Page 1 + page 2: same ULID cursor rule as /v1/records.
    let base = format!("/v1/politicians/{politician_id}/records");
    let (status, page1) = get(&app, &format!("{base}?limit=5")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page1);
    assert_verification_state_on_every_record(&page1);
    assert_eq!(item_ids(&page1), timeline[..5]);
    for item in page1["items"].as_array().unwrap() {
        assert_eq!(item["politician_id"].as_str().unwrap(), politician_id);
    }
    let cursor = page1["next_cursor"].as_str().unwrap();
    assert_eq!(cursor, timeline[4]);

    let (status, page2) = get(&app, &format!("{base}?limit=5&cursor={cursor}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page2);
    assert_verification_state_on_every_record(&page2);
    let ids2 = item_ids(&page2);
    assert_eq!(ids2, timeline[5..]);
    assert!(ids2.iter().all(|id| id.as_str() > cursor));
    assert!(page2["next_cursor"].is_null());

    // Unknown politician: 404 in the same error envelope.
    let err = validator_for(&doc, path, "404");
    let (status, body) = get(&app, "/v1/politicians/01ARZ3NDEKTSV4RRFFQ69G5FAV/records").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "not_found");
}

/// `/v1/records` filter parameters ARE the shared grammar (design §6.3):
/// results must equal an independently written SQL predicate with the same
/// documented semantics.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn records_filter_through_the_shared_grammar(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/records", "200");

    // asset_class filter.
    let expected: Vec<String> = sqlx::query_scalar(
        "select id from disclosure_record where asset_class = 'equity' order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!expected.is_empty(), "fixtures carry equity rows");
    let (status, page) = get(&app, "/v1/records?asset_class=equity&limit=200").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page);
    assert_eq!(item_ids(&page), expected);

    // value_min: band-overlap semantics (open-ended bands can always reach).
    let expected: Vec<String> = sqlx::query_scalar(
        "select id from disclosure_record \
         where value_low is not null \
           and (value_high is null or value_high >= 50000.00) \
         order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let (status, page) = get(&app, "/v1/records?value_min=50000.00&limit=200").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page);
    assert_eq!(item_ids(&page), expected);

    // An out-of-vocabulary filter value rejects in the error envelope.
    let err = validator_for(&doc, "/v1/records", "400");
    let (status, body) = get(&app, "/v1/records?record_type=bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_query");
}

/// Alert-rules CRUD: contract-valid round trip, strict filter validation
/// (unknown keys reject — the schema is the committed grammar snapshot),
/// and the one-channel-per-type rule. Since goal 050 the surface requires a
/// pro/data key and rules belong to the authenticated account (the tier
/// matrix itself is proven in tests/tiers.rs).
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alert_rules_crud_round_trip_matches_the_contract(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/alert-rules";
    let id_path = "/v1/alert-rules/{id}";
    let key = seed_key(&app, "rules@contract", "pro").await;
    let bearer = format!("Bearer {key}");
    let auth: &[(&str, &str)] = &[("authorization", &bearer)];

    // POST: created, contract-valid, filter normalized through the grammar.
    let spec = json!({
        "filter": { "record_type": "transaction", "value_min": "1000.00" },
        "channels": [
            { "type": "email", "to": "alerts@example.org" },
            { "type": "webhook", "url": "https://example.org/hook", "secret": "s3cret" },
        ],
    });
    let created_ok = validator_for_op(&doc, "post", path, "201");
    let (status, rule) = send_with(&app, "POST", path, auth, Some(&spec)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_valid(&created_ok, &rule);
    assert_eq!(rule["filter"], spec["filter"]);
    assert_eq!(rule["digest"], json!(false));
    assert_eq!(rule["active"], json!(true));
    let rule_id = rule["id"].as_str().unwrap().to_owned();

    // POST with a typo'd filter key: the committed schema rejects it
    // (a silently ignored key would match everything — fail closed).
    let bad = json!({
        "filter": { "recordtype": "transaction" },
        "channels": [{ "type": "email", "to": "a@example.org" }],
    });
    let err = validator_for_op(&doc, "post", path, "400");
    let (status, body) = send_with(&app, "POST", path, auth, Some(&bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_filter");

    // POST with two channels of one type: rejected (dedup key is per type).
    let bad = json!({
        "filter": {},
        "channels": [
            { "type": "webhook", "url": "https://a.example.org", "secret": "s1" },
            { "type": "webhook", "url": "https://b.example.org", "secret": "s2" },
        ],
    });
    let (status, body) = send_with(&app, "POST", path, auth, Some(&bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_channels");

    // GET list: contract-valid; scoped to the authenticated account.
    let list_ok = validator_for(&doc, path, "200");
    let (status, list) = send_with(&app, "GET", path, auth, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&list_ok, &list);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // PUT: replaces, contract-valid, updated_at advances.
    let update = json!({
        "filter": { "asset_class": "equity" },
        "channels": [{ "type": "email", "to": "alerts@example.org" }],
        "digest": true,
    });
    let updated_ok = validator_for_op(&doc, "put", id_path, "200");
    let (status, updated) = send_with(
        &app,
        "PUT",
        &format!("/v1/alert-rules/{rule_id}"),
        auth,
        Some(&update),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&updated_ok, &updated);
    assert_eq!(updated["digest"], json!(true));
    assert_eq!(updated["filter"], update["filter"]);
    assert!(updated["updated_at"].as_str().unwrap() >= rule["created_at"].as_str().unwrap());

    // PUT/DELETE on an unknown id: 404 envelope.
    let missing = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let (status, body) = send_with(
        &app,
        "PUT",
        &format!("/v1/alert-rules/{missing}"),
        auth,
        Some(&update),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // DELETE: 204, then gone.
    let (status, body) = send_with(
        &app,
        "DELETE",
        &format!("/v1/alert-rules/{rule_id}"),
        auth,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);
    let (status, _) = send_with(
        &app,
        "DELETE",
        &format!("/v1/alert-rules/{rule_id}"),
        auth,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = send_with(&app, "GET", path, auth, None).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

// ------------------------------------------------- goal 040a read surface --

/// The reviewer's corrected facts for the amended Boeing row — the same edit
/// seed the promote suite proves invariant 1 with: the amount band was
/// mis-extracted one band too low; everything else re-attested as filed.
/// Identity fields are unbound (nil ULIDs) — `promote` pins them from the
/// original row.
fn corrected_boeing() -> GoldCandidate {
    GoldCandidate {
        filing_id: "00000000000000000000000000".parse().unwrap(),
        politician_id: "00000000000000000000000000".parse().unwrap(),
        regime_id: "00000000000000000000000000".parse().unwrap(),
        instrument_id: None,
        asset_description_raw: "Boeing Company (BA) [ST]".to_owned(),
        record_type: RecordType::Transaction,
        asset_class: AssetClass::Equity,
        side: Some(Side::Sell),
        transaction_date: Some(NaiveDate::from_ymd_opt(2025, 12, 9).unwrap()),
        as_of_date: None,
        notified_date: Some(NaiveDate::from_ymd_opt(2025, 12, 9).unwrap()),
        value: Some(
            ValueInterval::new(
                rust_decimal::Decimal::new(1_500_100, 2),
                Some(rust_decimal::Decimal::new(5_000_000, 2)),
                Currency::USD,
            )
            .unwrap(),
        ),
        owner: Some(Owner::Self_),
        extraction_confidence: None, // human correction, not an extractor guess
        extracted_by: "review:contract_test@1".to_owned(),
        fingerprint: None,
        details: serde_json::json!({
            "doc_id": "20033759",
            "row_ordinal": 1,
            "row_id": "2000152831",
            "asset_type_code": "ST",
            "amount_band_raw": "$15,001 - $50,000",
            "transaction_type_raw": "S",
            "partial_sale": false,
            "cap_gains_over_200": null,
            "filing_status_raw": "Amended",
            "owner_source": "default_self",
            "subholding_of": "Interactive Brokers LLC",
            "vehicle_owner_code": null,
            "vehicle_location": null,
            "description": null,
            "comments": "Sold at a $1,440 loss.",
            "signed_date": "2026-01-07"
        }),
    }
}

/// The multi-row fixture filer's politician id.
async fn smucker_id(pool: &PgPool) -> String {
    resolve_politician(
        pool,
        &us_house::seed::regime_binding(),
        "Hon. Lloyd K. Smucker",
        "PA11",
    )
    .await
    .unwrap()
    .unwrap()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn politicians_list_pages_searches_and_matches_the_contract(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/politicians", "200");
    assert!(
        ok.validate(&json!({ "items": "not-an-array" })).is_err(),
        "the contract validator must have teeth"
    );

    // Ground truth: all five roster politicians in ULID order.
    let all: Vec<String> = sqlx::query_scalar("select id from politician order by id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(all.len(), 5, "four index-slice members + the paper filer");

    // Page walk, limit 2: the same ULID cursor rule as /v1/records.
    let (status, page1) = get(&app, "/v1/politicians?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page1);
    assert_eq!(item_ids(&page1), all[..2]);
    for item in page1["items"].as_array().unwrap() {
        assert!(
            item["canonical_name"]
                .as_str()
                .is_some_and(|n| !n.is_empty()),
            "every listed politician carries a canonical name: {item:#}"
        );
    }
    let cursor1 = page1["next_cursor"].as_str().unwrap();
    assert_eq!(cursor1, all[1], "cursor = last id of the page");
    let (status, page2) = get(&app, &format!("/v1/politicians?limit=2&cursor={cursor1}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page2);
    assert_eq!(item_ids(&page2), all[2..4]);
    let cursor2 = page2["next_cursor"].as_str().unwrap();
    let (status, page3) = get(&app, &format!("/v1/politicians?limit=2&cursor={cursor2}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page3);
    assert_eq!(item_ids(&page3), all[4..]);
    assert!(
        page3["next_cursor"].is_null(),
        "exhausted listing terminates"
    );

    // Name query: canonical_name ILIKE, case-insensitive, no fuzzy magic.
    let smucker = smucker_id(&pool).await;
    let (status, hits) = get(&app, "/v1/politicians?q=smucker").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &hits);
    assert_eq!(item_ids(&hits), vec![smucker.clone()]);

    // Alias-only match: "Hon. Lloyd" appears in the as-filed alias, not the
    // canonical name — the alias join must carry it.
    let (status, hits) = get(&app, "/v1/politicians?q=Hon.%20Lloyd").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&hits), vec![smucker]);

    // ILIKE metacharacters are literals, never wildcards: '%' matches nothing.
    let (status, hits) = get(&app, "/v1/politicians?q=%25").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&hits), Vec::<String>::new());

    // Errors use the consistent envelope.
    let err = validator_for(&doc, "/v1/politicians", "400");
    let (status, body) = get(&app, "/v1/politicians?cursor=not-a-ulid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_cursor");
    let (status, body) = get(&app, "/v1/politicians?limit=0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_limit");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn politician_profile_carries_mandates_and_the_record_summary(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/politicians/{id}";
    let ok = validator_for(&doc, path, "200");

    let id = smucker_id(&pool).await;
    let (status, profile) = get(&app, &format!("/v1/politicians/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &profile);
    assert_eq!(profile["id"], json!(id));
    assert_eq!(profile["canonical_name"], json!("Lloyd K. Smucker"));

    // Mandates straight from Gold.
    let mandates = profile["mandates"].as_array().unwrap();
    assert_eq!(mandates.len(), 1, "one seeded House mandate");
    assert_eq!(mandates[0]["jurisdiction_id"], json!("us"));
    assert_eq!(mandates[0]["body"], json!("US House"));
    assert_eq!(mandates[0]["role"], json!("Representative"));
    assert_eq!(mandates[0]["district"], json!("PA11"));

    // Record summary equals an independently written SQL aggregate.
    let (count, first, last): (i64, Option<NaiveDate>, Option<NaiveDate>) = sqlx::query_as(
        "select count(*), min(event_date), max(event_date) \
         from disclosure_record where politician_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 8, "the multi-row fixture publishes 8 rows");
    assert_eq!(profile["records"]["count"], json!(count));
    assert_eq!(profile["records"]["first_event_date"], json!(first));
    assert_eq!(profile["records"]["last_event_date"], json!(last));

    // Unknown politician: 404 in the envelope.
    let err = validator_for(&doc, path, "404");
    let (status, body) = get(&app, "/v1/politicians/01ARZ3NDEKTSV4RRFFQ69G5FAV").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "not_found");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn record_detail_returns_provenance_and_the_supersession_chain(pool: PgPool) {
    seed_via_pipeline(&pool).await;

    // Resolve the run's one open review task as an EDIT: promote inserts a
    // superseding 'corrected' record (invariant 1) — the chain under test.
    let (task_id, original_id): (String, String) = sqlx::query_as(
        "select id, target_id from review_task \
         where reason = 'ptr_amendment_unlinked' and status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let outcome = resolve_review_task(
        &pool,
        &task_id,
        Verdict::Edit {
            regime_code: "us_house".to_owned(),
            corrected: Box::new(corrected_boeing()),
        },
        None,
    )
    .await
    .unwrap();
    let ResolveOutcome::Applied {
        superseding_record_id: Some(superseding_id),
        ..
    } = outcome
    else {
        panic!("edit resolution must insert a superseding record: {outcome:?}");
    };

    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/records/{id}";
    let ok = validator_for(&doc, path, "200");
    assert!(
        ok.validate(&json!({ "record": "not-an-object" })).is_err(),
        "the contract validator must have teeth"
    );

    // The original record: full provenance + the superseding correction.
    let (status, detail) = get(&app, &format!("/v1/records/{original_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &detail);
    assert_eq!(detail["record"]["id"], json!(original_id));
    assert_eq!(detail["record"]["verification_state"], json!("unverified"));

    // Provenance equals an independent SQL join over filing/raw_document.
    let (filing_id, external_id, filed_date, raw_id, source_url, sha256): (
        String,
        Option<String>,
        Option<NaiveDate>,
        String,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "select f.id, f.external_id, f.filed_date, d.id, d.source_url, d.sha256 \
         from disclosure_record r \
         join filing f on f.id = r.filing_id \
         join raw_document d on d.id = f.raw_document_id \
         where r.id = $1",
    )
    .bind(&original_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let filing = &detail["provenance"]["filing"];
    assert_eq!(filing["id"], json!(filing_id));
    assert_eq!(filing["external_id"], json!(external_id));
    assert_eq!(filing["filed_date"], json!(filed_date));
    assert!(
        filing.as_object().unwrap().contains_key("published_at"),
        "provenance declares when the government published the filing"
    );
    let raw = &detail["provenance"]["raw_document"];
    assert_eq!(raw["id"], json!(raw_id));
    assert_eq!(raw["source_url"], json!(source_url));
    assert_eq!(raw["sha256"], json!(sha256));
    assert!(raw["fetched_at"].is_string(), "archived copies are dated");
    let regime = &detail["provenance"]["regime"];
    assert_eq!(regime["id"], json!(us_house::seed::REGIME_ID));
    assert_eq!(regime["regime_type"], json!("transaction_report"));
    assert_eq!(regime["value_precision"], json!("banded"));

    // Supersession chain, both directions of supersedes_record_id.
    assert_eq!(
        detail["supersedes"].as_array().unwrap().len(),
        0,
        "the original supersedes nothing"
    );
    let superseding = detail["superseded_by"].as_array().unwrap();
    assert_eq!(superseding.len(), 1, "one correction supersedes it");
    assert_eq!(superseding[0]["id"], json!(superseding_id));
    assert_eq!(superseding[0]["verification_state"], json!("corrected"));
    assert_eq!(superseding[0]["supersedes_record_id"], json!(original_id));
    assert!(
        superseding[0]["value"]["low"].is_string(),
        "money stays decimal strings in chain records (invariant 7)"
    );

    // The correction's own page points back at the original.
    let (status, detail) = get(&app, &format!("/v1/records/{superseding_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &detail);
    assert_eq!(detail["superseded_by"].as_array().unwrap().len(), 0);
    let chain = detail["supersedes"].as_array().unwrap();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0]["id"], json!(original_id));

    // Unknown record: 404 in the envelope.
    let err = validator_for(&doc, path, "404");
    let (status, body) = get(&app, "/v1/records/01ARZ3NDEKTSV4RRFFQ69G5FAV").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "not_found");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn jurisdictions_and_regimes_serve_the_scorecard_metadata(pool: PgPool) {
    // The scorecard is regime metadata only — the seed stage suffices.
    govfolio_core::db::migrate(&pool).await.unwrap();
    seed_regime(&pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
    let app = test_app(&pool);
    let doc = openapi_doc();

    let ok = validator_for(&doc, "/v1/jurisdictions", "200");
    assert!(
        ok.validate(&json!("garbage")).is_err(),
        "the contract validator must have teeth"
    );
    let (status, list) = get(&app, "/v1/jurisdictions").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &list);
    let us = list
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == "us")
        .unwrap();
    assert_eq!(us["name"], json!("United States"));
    assert_eq!(us["iso_code"], json!("US"));
    assert_eq!(us["level"], json!("national"));

    // The scorecard source (design §7.3): regime metadata joined in.
    let regimes = us["regimes"].as_array().unwrap();
    assert_eq!(regimes.len(), 1);
    let regime = &regimes[0];
    assert_eq!(regime["id"], json!(us_house::seed::REGIME_ID));
    assert_eq!(regime["body"], json!("US House"));
    assert_eq!(regime["regime_type"], json!("transaction_report"));
    assert_eq!(regime["value_precision"], json!("banded"));
    assert_eq!(regime["disclosure_lag_days"], json!(45));
    assert_eq!(
        regime["source_url"],
        json!("https://disclosures-clerk.house.gov/FinancialDisclosure")
    );
    assert!(regime["cadence"].as_str().is_some_and(|c| !c.is_empty()));
    assert_eq!(regime["effective_from"], json!("2012-04-04"));

    // /v1/regimes = the flat scorecard endpoint (design §6.1): same shape.
    let ok = validator_for(&doc, "/v1/regimes", "200");
    let (status, flat) = get(&app, "/v1/regimes").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &flat);
    assert_eq!(flat.as_array().unwrap().len(), 1);
    assert_eq!(flat[0], *regime, "one regime shape behind both doors");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn search_returns_typed_hits_and_escapes_like_metacharacters(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    // No pipeline stage writes instruments yet (below-threshold matches stay
    // NULL — invariant 3), so the instrument arm is seeded directly.
    for (id, name, ticker) in [
        ("01ARZ3NDEKTSV4RRFFQ69G5FA0", "Boeing Company", "BA"),
        ("01ARZ3NDEKTSV4RRFFQ69G5FA1", "Apple Inc.", "AAPL"),
    ] {
        sqlx::query(
            "insert into instrument (id, name, ticker, asset_class) values ($1, $2, $3, 'equity')",
        )
        .bind(id)
        .bind(name)
        .bind(ticker)
        .execute(&pool)
        .await
        .unwrap();
    }
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/search", "200");
    assert!(
        ok.validate(&json!({ "politicians": "not-an-array" }))
            .is_err(),
        "the contract validator must have teeth"
    );

    // Politician arm: name/alias ILIKE.
    let smucker = smucker_id(&pool).await;
    let (status, results) = get(&app, "/v1/search?q=smucker").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &results);
    assert_eq!(results["query"], json!("smucker"));
    let politician_hits: Vec<&str> = results["politicians"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["id"].as_str().unwrap())
        .collect();
    assert_eq!(politician_hits, vec![smucker.as_str()]);
    assert_eq!(results["instruments"].as_array().unwrap().len(), 0);

    // Instrument arm: by name and by ticker.
    let (status, results) = get(&app, "/v1/search?q=boeing").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &results);
    let names: Vec<&str> = results["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Boeing Company"]);
    let (status, results) = get(&app, "/v1/search?q=AAPL").await;
    assert_eq!(status, StatusCode::OK);
    let tickers: Vec<&str> = results["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["ticker"].as_str().unwrap())
        .collect();
    assert_eq!(tickers, vec!["AAPL"]);

    // '%' is a literal, never a wildcard: nothing contains one.
    let (status, results) = get(&app, "/v1/search?q=%25").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &results);
    assert_eq!(results["politicians"].as_array().unwrap().len(), 0);
    assert_eq!(results["instruments"].as_array().unwrap().len(), 0);

    // Missing or blank q: 400 in the envelope.
    let err = validator_for(&doc, "/v1/search", "400");
    let (status, body) = get(&app, "/v1/search").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_query");
    let (status, body) = get(&app, "/v1/search?q=%20").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_query");
}

// ------------------------------------------------ goal 041a review surface --

/// Inserts an open review task with controlled `priority`/`created_at` (test
/// seeding, like the instrument rows above — task rows are pipeline data;
/// adjudication still only ever happens through promote).
async fn seed_task(
    pool: &PgPool,
    target_id: &str,
    reason: &str,
    priority: f32,
    created_at: &str,
) -> String {
    let id = ulid::Ulid::new().to_string();
    sqlx::query(
        "insert into review_task \
           (id, target_kind, target_id, reason, priority_score, created_at) \
         values ($1, 'disclosure_record', $2, $3, $4, $5::timestamptz)",
    )
    .bind(&id)
    .bind(target_id)
    .bind(reason)
    .bind(priority)
    .bind(created_at)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// The full row rendered by Postgres itself — every column, byte-identical or
/// not. THE probe for "facts are never `UPDATE`d" (T11 technique).
async fn row_text(pool: &PgPool, record_id: &str) -> String {
    sqlx::query_scalar("select d::text from disclosure_record d where id = $1")
        .bind(record_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The row minus `verification_state` (the one column confirm/reject may
/// touch), as canonical jsonb text.
async fn row_except_state(pool: &PgPool, record_id: &str) -> String {
    sqlx::query_scalar(
        "select (to_jsonb(d) - 'verification_state')::text \
         from disclosure_record d where id = $1",
    )
    .bind(record_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn record_state(pool: &PgPool, record_id: &str) -> String {
    sqlx::query_scalar("select verification_state from disclosure_record where id = $1")
        .bind(record_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn queue_task_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["task"]["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn review_queue_ranks_by_priority_then_age_and_paginates(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/review-tasks";
    let ok = validator_for(&doc, path, "200");
    assert!(
        ok.validate(&json!({ "items": "not-an-array" })).is_err(),
        "the contract validator must have teeth"
    );

    // The pipeline's own task (priority 0, created now()).
    let (pipeline_task, boeing_record): (String, String) = sqlx::query_as(
        "select id, target_id from review_task where reason = 'ptr_amendment_unlinked'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Three controlled tasks on other records. Priority dominates age (the
    // mid-priority task is the OLDEST); created_at asc breaks priority ties.
    let others: Vec<String> =
        sqlx::query_scalar("select id from disclosure_record where id <> $1 order by id limit 3")
            .bind(&boeing_record)
            .fetch_all(&pool)
            .await
            .unwrap();
    let hi_old = seed_task(&pool, &others[0], "spot_check", 5.0, "2026-07-01T00:00:00Z").await;
    let hi_new = seed_task(&pool, &others[1], "spot_check", 5.0, "2026-07-02T00:00:00Z").await;
    let mid = seed_task(
        &pool,
        &others[2],
        "user_report",
        1.5,
        "2026-06-01T00:00:00Z",
    )
    .await;
    let expected = [hi_old, hi_new, mid, pipeline_task.clone()];

    // Full queue: priority_score desc, then created_at asc; open by default.
    let (status, page) = get(&app, "/v1/review-tasks?limit=200").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page);
    assert_eq!(queue_task_ids(&page), expected);
    assert!(page["next_cursor"].is_null());

    // Every record-kind item carries the reviewer's scan summary.
    let boeing_item = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["task"]["id"] == json!(pipeline_task))
        .unwrap();
    let summary = &boeing_item["record"];
    assert_eq!(summary["record_id"], json!(boeing_record));
    assert_eq!(
        summary["asset_description_raw"],
        json!("Boeing Company (BA) [ST]")
    );
    assert_eq!(summary["politician_name"], json!("David Rouzer"));
    assert_eq!(summary["record_type"], json!("transaction"));
    assert_eq!(summary["verification_state"], json!("unverified"));
    assert!(
        summary["value"]["low"].is_string(),
        "money stays decimal strings on the queue (invariant 7)"
    );
    assert!(
        summary["extracted_by"]
            .as_str()
            .is_some_and(|e| !e.is_empty()),
        "the queue shows who extracted"
    );

    // Cursor pagination preserves the ranking exactly.
    let (status, page1) = get(&app, "/v1/review-tasks?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page1);
    assert_eq!(queue_task_ids(&page1), expected[..2]);
    let cursor = page1["next_cursor"].as_str().unwrap();
    assert_eq!(cursor, expected[1], "cursor = last task id of the page");
    let (status, page2) = get(&app, &format!("/v1/review-tasks?limit=2&cursor={cursor}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page2);
    assert_eq!(queue_task_ids(&page2), expected[2..]);
    assert!(page2["next_cursor"].is_null());

    // Status filter (no resolved tasks yet); malformed inputs reject.
    let (status, resolved) = get(&app, "/v1/review-tasks?status=resolved").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resolved["items"].as_array().unwrap().len(), 0);
    let err = validator_for(&doc, path, "400");
    let (status, body) = get(&app, "/v1/review-tasks?status=bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_status");
    let (status, body) = get(&app, "/v1/review-tasks?cursor=not-a-ulid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_cursor");
    // A well-formed ULID that names no task cannot anchor a page.
    let (status, body) = get(&app, "/v1/review-tasks?cursor=01ARZ3NDEKTSV4RRFFQ69G5FAV").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_cursor");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn review_task_detail_serves_record_and_extraction_context(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let path = "/v1/review-tasks/{id}";
    let ok = validator_for(&doc, path, "200");
    assert!(
        ok.validate(&json!({ "task": [] })).is_err(),
        "the contract validator must have teeth"
    );

    // The pipeline's Boeing task: text-path extraction, no cache entry.
    let (task_id, record_id): (String, String) = sqlx::query_as(
        "select id, target_id from review_task where reason = 'ptr_amendment_unlinked'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, detail) = get(&app, &format!("/v1/review-tasks/{task_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &detail);
    assert_eq!(detail["task"]["id"], json!(task_id));
    assert_eq!(detail["task"]["status"], json!("open"));
    assert_eq!(detail["task"]["reason"], json!("ptr_amendment_unlinked"));

    // Full target record in the 040a RecordDetail shape, provenance included.
    assert_eq!(detail["record"]["record"]["id"], json!(record_id));
    let raw = &detail["record"]["provenance"]["raw_document"];
    assert_eq!(raw["sha256"].as_str().unwrap().len(), 64);
    assert!(
        raw.as_object().unwrap().contains_key("source_url"),
        "the reviewer sees where the Bronze copy came from"
    );

    // The LLM pre-review note: extraction context travels with the task.
    assert_eq!(
        detail["extraction"]["extracted_by"],
        detail["record"]["record"]["extracted_by"]
    );
    assert!(
        detail["extraction"]
            .as_object()
            .unwrap()
            .contains_key("extraction_confidence"),
        "confidence is part of the pre-review note"
    );
    assert!(
        detail["extraction"]["cache"].is_null(),
        "text-path records have no extraction_cache entry"
    );

    // The scanned-paper record extracted through the LLM seam: its cache row
    // (seeded exactly like production pg_put writes it) surfaces model +
    // cross-check provenance.
    let (llm_record, llm_sha, llm_tag): (String, String, String) = sqlx::query_as(
        "select r.id, d.sha256, r.extracted_by from disclosure_record r \
         join filing f on f.id = r.filing_id \
         join raw_document d on d.id = f.raw_document_id \
         where r.extracted_by like '%/llm@%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into extraction_cache \
           (document_sha256, extractor_tag, model_id, rows, provenance) \
         values ($1, $2, 'claude-haiku-4-5-20251001', '[]'::jsonb, \
                 '{\"source\": \"live anthropic messages call\", \"cross_checked\": true}'::jsonb)",
    )
    .bind(&llm_sha)
    .bind(&llm_tag)
    .execute(&pool)
    .await
    .unwrap();
    let llm_task = seed_task(
        &pool,
        &llm_record,
        "spot_check",
        2.0,
        "2026-07-03T00:00:00Z",
    )
    .await;
    let (status, detail) = get(&app, &format!("/v1/review-tasks/{llm_task}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &detail);
    assert_eq!(detail["extraction"]["extracted_by"], json!(llm_tag));
    let cache = &detail["extraction"]["cache"];
    assert_eq!(cache["model_id"], json!("claude-haiku-4-5-20251001"));
    assert!(cache["cached_at"].is_string());
    assert_eq!(cache["provenance"]["cross_checked"], json!(true));

    // Unknown task: 404 in the envelope.
    let err = validator_for(&doc, path, "404");
    let (status, body) = get(&app, "/v1/review-tasks/01ARZ3NDEKTSV4RRFFQ69G5FAV").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "not_found");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn resolve_confirm_and_reject_round_trip_with_exact_audit(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let resolve_path = "/v1/review-tasks/{id}/resolve";
    let audit_path = "/v1/review-tasks/{id}/audit";
    let ok = validator_for_op(&doc, "post", resolve_path, "200");
    let audit_ok = validator_for(&doc, audit_path, "200");

    // Two adjudications on two records (neither is the Boeing edit target).
    let records: Vec<String> = sqlx::query_scalar(
        "select id from disclosure_record \
         where id not in (select target_id from review_task) order by id limit 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let confirm_task = seed_task(
        &pool,
        &records[0],
        "spot_check",
        3.0,
        "2026-07-03T00:00:00Z",
    )
    .await;
    let reject_task = seed_task(
        &pool,
        &records[1],
        "user_report",
        3.5,
        "2026-07-03T00:00:00Z",
    )
    .await;

    // Confirm: the ONE sanctioned state transition, nothing else touched.
    let before = row_except_state(&pool, &records[0]).await;
    let (status, resolved) = send(
        &app,
        "POST",
        &format!("/v1/review-tasks/{confirm_task}/resolve"),
        Some(&json!({"reviewer": "rev-a", "verdict": "confirm", "note": "matches the filing"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &resolved);
    assert_eq!(resolved["outcome"], json!("applied"));
    assert_eq!(resolved["record_id"], json!(records[0]));
    assert!(resolved["superseding_record_id"].is_null());
    assert_eq!(record_state(&pool, &records[0]).await, "verified");
    assert_eq!(
        row_except_state(&pool, &records[0]).await,
        before,
        "confirm touches ONLY verification_state"
    );

    // Double-resolve: 409 in the envelope; nothing changes.
    let conflict = validator_for_op(&doc, "post", resolve_path, "409");
    let (status, body) = send(
        &app,
        "POST",
        &format!("/v1/review-tasks/{confirm_task}/resolve"),
        Some(&json!({"reviewer": "rev-b", "verdict": "reject"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_valid(&conflict, &body);
    assert_eq!(body["error"]["code"], "already_resolved");
    assert_eq!(record_state(&pool, &records[0]).await, "verified");

    // Reject: disputed, only the state column.
    let before = row_except_state(&pool, &records[1]).await;
    let (status, resolved) = send(
        &app,
        "POST",
        &format!("/v1/review-tasks/{reject_task}/resolve"),
        Some(&json!({"reviewer": "rev-a", "verdict": "reject"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &resolved);
    assert_eq!(record_state(&pool, &records[1]).await, "disputed");
    assert_eq!(row_except_state(&pool, &records[1]).await, before);

    // Door validation: malformed requests never reach promote (no verdict was
    // adjudicated, so they are not audit-logged either).
    for (body, code) in [
        (
            json!({"reviewer": "  ", "verdict": "confirm"}),
            "invalid_reviewer",
        ),
        (
            json!({"reviewer": "rev-a", "verdict": "approve"}),
            "invalid_verdict",
        ),
        (
            json!({"reviewer": "rev-a", "verdict": "edit"}),
            "invalid_edit",
        ),
        (
            json!({"reviewer": "rev-a", "verdict": "confirm", "corrected": {}}),
            "invalid_edit",
        ),
    ] {
        let (status, err_body) = send(
            &app,
            "POST",
            &format!("/v1/review-tasks/{reject_task}/resolve"),
            Some(&body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{code}");
        assert_eq!(err_body["error"]["code"], json!(code));
    }

    // Unknown task: 404 (no audit row can reference a task that never existed).
    let (status, body) = send(
        &app,
        "POST",
        "/v1/review-tasks/01ARZ3NDEKTSV4RRFFQ69G5FAV/resolve",
        Some(&json!({"reviewer": "rev-a", "verdict": "confirm"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // The audit log, exactly.
    let (status, log) = get(&app, &format!("/v1/review-tasks/{confirm_task}/audit")).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&audit_ok, &log);
    let entries = log.as_array().unwrap();
    assert_eq!(entries.len(), 2, "applied + the conflicting second attempt");
    assert_eq!(entries[0]["reviewer"], json!("rev-a"));
    assert_eq!(entries[0]["verdict"], json!("confirm"));
    assert_eq!(entries[0]["outcome"], json!("applied"));
    assert_eq!(entries[0]["note"], json!("matches the filing"));
    assert_eq!(entries[0]["affected_record_ids"], json!([records[0]]));
    assert_eq!(entries[1]["reviewer"], json!("rev-b"));
    assert_eq!(entries[1]["verdict"], json!("reject"));
    assert_eq!(entries[1]["outcome"], json!("conflict"));
    assert_eq!(entries[1]["affected_record_ids"], json!([]));

    let (_, log) = get(&app, &format!("/v1/review-tasks/{reject_task}/audit")).await;
    let entries = log.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["verdict"], json!("reject"));
    assert_eq!(entries[0]["outcome"], json!("applied"));
    assert_eq!(entries[0]["affected_record_ids"], json!([records[1]]));

    // Exactly one row per attempt that carried a verdict — nothing more.
    let total: i64 = sqlx::query_scalar("select count(*) from review_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 3);

    // Audit log of an unknown task: 404 in the envelope.
    let (status, body) = get(&app, "/v1/review-tasks/01ARZ3NDEKTSV4RRFFQ69G5FAV/audit").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn resolve_edit_supersedes_through_promote_and_audits_failures(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for_op(&doc, "post", "/v1/review-tasks/{id}/resolve", "200");

    let (task_id, original_id): (String, String) = sqlx::query_as(
        "select id, target_id from review_task \
         where reason = 'ptr_amendment_unlinked' and status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let before = row_text(&pool, &original_id).await;

    // Corrected facts in the wire shape; identity fields OMITTED — promote
    // pins them from the original row (reviewer identity input is ignored).
    let mut corrected = serde_json::to_value(corrected_boeing()).unwrap();
    for key in ["filing_id", "politician_id", "regime_id", "fingerprint"] {
        corrected.as_object_mut().unwrap().remove(key);
    }

    // A contract-violating correction fails closed: 500, task still open, no
    // superseding row, original untouched — but the ATTEMPT is audit-logged.
    let mut bad = corrected.clone();
    bad["details"] = json!({});
    let (status, body) = send(
        &app,
        "POST",
        &format!("/v1/review-tasks/{task_id}/resolve"),
        Some(&json!({
            "reviewer": "rev-a", "verdict": "edit", "regime_code": "us_house",
            "corrected": bad, "note": "first try",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], "internal");
    let (task_status, corrected_rows): (String, i64) = sqlx::query_as(
        "select (select status from review_task where id = $1), \
                (select count(*) from disclosure_record where verification_state = 'corrected')",
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(task_status, "open");
    assert_eq!(corrected_rows, 0);
    assert_eq!(row_text(&pool, &original_id).await, before);

    // The valid correction: applied through the REAL promote path.
    let (status, resolved) = send(
        &app,
        "POST",
        &format!("/v1/review-tasks/{task_id}/resolve"),
        Some(&json!({
            "reviewer": "rev-a", "verdict": "edit", "regime_code": "us_house",
            "corrected": corrected, "note": "band was one too low",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &resolved);
    assert_eq!(resolved["outcome"], json!("applied"));
    assert_eq!(resolved["record_id"], json!(original_id));
    let superseding_id = resolved["superseding_record_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Invariant 1, byte-level: the original row never changed.
    assert_eq!(
        row_text(&pool, &original_id).await,
        before,
        "edits supersede, never update"
    );

    // The correction is a real Gold row wired into the supersession chain.
    let (status, detail) = get(&app, &format!("/v1/records/{original_id}")).await;
    assert_eq!(status, StatusCode::OK);
    let superseded_by = detail["superseded_by"].as_array().unwrap();
    assert_eq!(superseded_by.len(), 1);
    assert_eq!(superseded_by[0]["id"], json!(superseding_id));
    assert_eq!(superseded_by[0]["verification_state"], json!("corrected"));
    assert_eq!(superseded_by[0]["supersedes_record_id"], json!(original_id));
    assert_eq!(superseded_by[0]["value"]["high"], json!("50000.00"));

    // Audit trail, exactly: the failed attempt, then the applied edit with
    // BOTH affected record ids.
    let (status, log) = get(&app, &format!("/v1/review-tasks/{task_id}/audit")).await;
    assert_eq!(status, StatusCode::OK);
    let entries = log.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["verdict"], json!("edit"));
    assert_eq!(entries[0]["outcome"], json!("failed"));
    assert_eq!(entries[0]["note"], json!("first try"));
    assert_eq!(entries[0]["affected_record_ids"], json!([]));
    assert_eq!(entries[1]["outcome"], json!("applied"));
    assert_eq!(entries[1]["note"], json!("band was one too low"));
    assert_eq!(
        entries[1]["affected_record_ids"],
        json!([original_id, superseding_id])
    );
}

// ------------------------------------------ admin observability surface --

/// The eight admin snapshot doors that answer 200 with only a database (the
/// ninth, `/v1/admin/loop`, is repo-root-gated and probed separately).
const ADMIN_DB_PATHS: [&str; 8] = [
    "/v1/admin/overview",
    "/v1/admin/coverage",
    "/v1/admin/backfill",
    "/v1/admin/pipeline",
    "/v1/admin/quality",
    "/v1/admin/storage",
    "/v1/admin/serving",
    "/v1/admin/infra",
];

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn admin_observability_endpoints_gate_and_match_the_contract(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();

    // Without the token: 401 in the envelope, fail closed — the admin
    // surface must not leak a byte of operational data.
    let err = validator_for(&doc, "/v1/admin/overview", "401");
    let response = app
        .clone()
        .oneshot(
            Request::get("/v1/admin/overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "admin_token_required");

    // With the token: every DB-backed door answers 200 and validates against
    // the emitted schema.
    for path in ADMIN_DB_PATHS {
        let ok = validator_for(&doc, path, "200");
        assert!(
            ok.validate(&json!({ "generated_at": "not-even-close" }))
                .is_err(),
            "the contract validator must have teeth: {path}"
        );
        let (status, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body:#}");
        assert_valid(&ok, &body);
    }

    // Spot checks that the snapshots carry the seeded reality, not shapes
    // alone: the pipeline run opened one review task and published 13 rows.
    let (_, overview) = get(&app, "/v1/admin/overview").await;
    assert_eq!(overview["queue_depths"]["review_open"], json!(1));
    let (_, coverage) = get(&app, "/v1/admin/coverage").await;
    let us_house = coverage["regimes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["body"] == json!("US House"))
        .unwrap();
    assert_eq!(us_house["gold_records"], json!(13));
    assert_eq!(us_house["regime_codes"], json!(["us_house"]));

    // The opt-in br sweep: contract-valid; zero collisions on fixture data.
    let ok = validator_for(&doc, "/v1/admin/quality", "200");
    let (status, body) = get(&app, "/v1/admin/quality?sweep=br").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &body);
    assert_eq!(body["collision_sweep"]["pass"], json!(true));
    // An unknown sweep fails closed: 400 in the envelope, no partial answer.
    let err = validator_for(&doc, "/v1/admin/quality", "400");
    let (status, body) = get(&app, "/v1/admin/quality?sweep=bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_sweep");

    // /v1/admin/loop without GOVFOLIO_REPO_ROOT: 503 by design (the cloud
    // posture), in the envelope.
    let unavailable = validator_for(&doc, "/v1/admin/loop", "503");
    let (status, body) = get(&app, "/v1/admin/loop").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_valid(&unavailable, &body);
    assert_eq!(body["error"]["code"], "repo_root_unset");

    // With the checkout mounted: 200, contract-valid, real goals render.
    let mounted = api::app(
        pool.clone(),
        api::ApiConfig {
            admin_token: Some(TEST_ADMIN.to_owned()),
            repo_root: Some(workspace_root()),
            ..api::ApiConfig::new()
        },
    );
    let ok = validator_for(&doc, "/v1/admin/loop", "200");
    let (status, body) = get(&mounted, "/v1/admin/loop").await;
    assert_eq!(status, StatusCode::OK, "{body:#}");
    assert_valid(&ok, &body);
    assert!(
        !body["goals"].as_array().unwrap().is_empty(),
        "the goal queue parses from the real 000-INDEX.md"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn etag_round_trips_and_if_none_match_serves_304(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);

    // Every 200 GET carries a strong ETag: the quoted sha256 of the body.
    let (status, headers, body) = get_raw(&app, "/v1/records", &[]).await;
    assert_eq!(status, StatusCode::OK);
    let etag_header = headers.get("etag");
    assert!(etag_header.is_some(), "200 GET responses carry an ETag");
    let etag = etag_header.unwrap().to_str().unwrap().to_owned();
    assert!(
        etag.len() == 66 && etag.starts_with('"') && etag.ends_with('"'),
        "strong quoted sha256 hex, got {etag:?}"
    );
    assert!(!body.is_empty());

    // If-None-Match round trip: 304, empty body, ETag still present.
    let (status, headers, body) = get_raw(&app, "/v1/records", &[("if-none-match", &etag)]).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty(), "304 carries no body");
    assert_eq!(headers.get("etag").unwrap().to_str().unwrap(), etag);

    // A stale validator misses: full 200 again.
    let stale = format!("\"{}\"", "0".repeat(64));
    let (status, _, body) = get_raw(&app, "/v1/records", &[("if-none-match", &stale)]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    // A validator list and the wildcard both match.
    let list = format!("{stale}, {etag}");
    let (status, _, _) = get_raw(&app, "/v1/records", &[("if-none-match", &list)]).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    let (status, _, _) = get_raw(&app, "/v1/records", &[("if-none-match", "*")]).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);

    // Detail/list endpoints all sit behind the same middleware.
    let (status, headers, _) = get_raw(&app, "/v1/jurisdictions", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers.get("etag").is_some(),
        "ETags everywhere (design §6.1)"
    );

    // Error responses carry no validator.
    let (status, headers, _) = get_raw(&app, "/v1/records?limit=0", &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(headers.get("etag").is_none());
}
