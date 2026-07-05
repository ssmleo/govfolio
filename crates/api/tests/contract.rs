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
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
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

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("GET {uri} returned non-JSON ({status}): {e}"));
    (status, body)
}

/// Sends a JSON-bodied request (POST/PUT/DELETE); `None` body for DELETE.
/// Returns `Value::Null` for empty response bodies (204).
async fn send(app: &Router, method: &str, uri: &str, body: Option<&Value>) -> (StatusCode, Value) {
    let request = Request::builder().method(method).uri(uri);
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
    let app = api::app(pool.clone());
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
    let app = api::app(pool.clone());
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
    let app = api::app(pool.clone());
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
/// and the one-channel-per-type rule.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alert_rules_crud_round_trip_matches_the_contract(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = api::app(pool.clone());
    let doc = openapi_doc();
    let path = "/v1/alert-rules";
    let id_path = "/v1/alert-rules/{id}";

    // POST: created, contract-valid, filter normalized through the grammar.
    let spec = json!({
        "user_id": "user-030",
        "filter": { "record_type": "transaction", "value_min": "1000.00" },
        "channels": [
            { "type": "email", "to": "alerts@example.org" },
            { "type": "webhook", "url": "https://example.org/hook", "secret": "s3cret" },
        ],
    });
    let created_ok = validator_for_op(&doc, "post", path, "201");
    let (status, rule) = send(&app, "POST", path, Some(&spec)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_valid(&created_ok, &rule);
    assert_eq!(rule["filter"], spec["filter"]);
    assert_eq!(rule["digest"], json!(false));
    assert_eq!(rule["active"], json!(true));
    let rule_id = rule["id"].as_str().unwrap().to_owned();

    // POST with a typo'd filter key: the committed schema rejects it
    // (a silently ignored key would match everything — fail closed).
    let bad = json!({
        "user_id": "user-030",
        "filter": { "recordtype": "transaction" },
        "channels": [{ "type": "email", "to": "a@example.org" }],
    });
    let err = validator_for_op(&doc, "post", path, "400");
    let (status, body) = send(&app, "POST", path, Some(&bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_valid(&err, &body);
    assert_eq!(body["error"]["code"], "invalid_filter");

    // POST with two channels of one type: rejected (dedup key is per type).
    let bad = json!({
        "user_id": "user-030",
        "filter": {},
        "channels": [
            { "type": "webhook", "url": "https://a.example.org", "secret": "s1" },
            { "type": "webhook", "url": "https://b.example.org", "secret": "s2" },
        ],
    });
    let (status, body) = send(&app, "POST", path, Some(&bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_channels");

    // GET list: contract-valid; user_id filter works.
    let list_ok = validator_for(&doc, path, "200");
    let (status, list) = get(&app, "/v1/alert-rules?user_id=user-030").await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&list_ok, &list);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, other) = get(&app, "/v1/alert-rules?user_id=someone-else").await;
    assert_eq!(other.as_array().unwrap().len(), 0);

    // PUT: replaces, contract-valid, updated_at advances.
    let update = json!({
        "user_id": "user-030",
        "filter": { "asset_class": "equity" },
        "channels": [{ "type": "email", "to": "alerts@example.org" }],
        "digest": true,
    });
    let updated_ok = validator_for_op(&doc, "put", id_path, "200");
    let (status, updated) = send(
        &app,
        "PUT",
        &format!("/v1/alert-rules/{rule_id}"),
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
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/v1/alert-rules/{missing}"),
        Some(&update),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // DELETE: 204, then gone.
    let (status, body) = send(&app, "DELETE", &format!("/v1/alert-rules/{rule_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);
    let (status, _) = send(&app, "DELETE", &format!("/v1/alert-rules/{rule_id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = get(&app, "/v1/alert-rules").await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}
