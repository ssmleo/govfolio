//! Ops observability contract suite (goal 090): boots the axum app on a test
//! pool seeded by the REAL pipeline over the `us_house` fixtures (same Task 9
//! machinery as `tests/contract.rs`), then proves the `/v1/admin/ops/*`
//! surface: admin-gate fail-closed auth, every endpoint contract-valid
//! against the emitted `OpenAPI` schema, backfill year bucketing + fixture
//! gold counts, runs stage filter + keyset pagination, extraction-cost
//! null-tolerance (zeros until goal 021 writes `stats.extraction`), and the
//! `/healthz` mounting position (tokenless, un-ETagged).
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::run::{LocalFiling, Runner};
use pipeline::stages::roster::seed_roster;
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;

// ---------------------------------------------------------------- seeding --
// Same harness as tests/contract.rs: migrate, seed the regime + roster from
// the archived index evidence slices, run the pipeline over the five
// committed fixtures (13 Gold rows, all `unverified`).

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
    std::env::temp_dir().join(format!("govfolio-ops-{}-{nanos}", std::process::id()))
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

/// One row each into `sentinel_watch`, `drift_report`, `delivery` and
/// `sample_audit` — the pipeline seed never writes these (their writers live
/// in the worker: sentinel, sampler, alert dispatcher), so without this the
/// row decode/serialize paths of `SentinelSource`, `DriftReportEntry`,
/// `DeadDelivery` and `SampleAuditSlice` would only ever be validated as
/// EMPTY arrays. Careful not to disturb the fixture ground truths the tests
/// assert: the sentinel row is NOT frozen (`frozen_regimes` stays 0), the
/// drift report opens no review task (`review_open` stays 1), and the outbox
/// event is already dispatched.
async fn seed_ops_extras(pool: &PgPool) {
    sqlx::query(
        "insert into sentinel_watch (regime_code, last_status, last_layout_hash, last_count, \
                                     last_etag, last_modified, frozen) \
         values ('us_house', 200, 'layouthash0', 5, '\"etag-ops-suite\"', \
                 'Tue, 07 Jul 2026 22:10:04 GMT', false)",
    )
    .execute(pool)
    .await
    .unwrap();
    // ULID-shaped id: the contract publishes the ULID pattern on drift ids.
    sqlx::query(
        "insert into drift_report (id, regime_code, drift_kind, priority_score, \
                                   freezes_publication, dedup_key, detail) \
         values ('0DRFT000000000000000000001', 'us_house', 'count_delta', 41.5, false, \
                 'us_house:count_delta:ops-suite', '{\"previous\": 5, \"observed\": 4}')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("insert into alert_rule (id, user_id) values ('ops-suite-rule', 'ops-suite-user')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into outbox_event (id, kind, payload, dispatched_at) \
         values ('ops-suite-outbox', 'record.published', '{}', now())",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into delivery (id, alert_rule_id, outbox_event_id, channel, dedup_key, \
                               status, attempts, last_error) \
         values ('0DEAD000000000000000000001', 'ops-suite-rule', 'ops-suite-outbox', 'webhook', \
                 'ops-suite-rule:ops-suite-outbox:webhook', 'dead', 3, 'HTTP 410 Gone')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into sample_audit (id, regime_id, record_id, sample_month, seed, status) \
         select 'ops-suite-sample-1', regime_id, id, '2026-06', 42, 'confirmed' \
         from disclosure_record order by id limit 1",
    )
    .execute(pool)
    .await
    .unwrap();
}

// ------------------------------------------------------------- app + auth --

/// Bootstrap admin token for this suite (the admin surface is disabled when
/// unset — goal 050 fail-closed gate, shared by the ops subtree).
const TEST_ADMIN: &str = "ops-suite-admin";

/// Every admin-gated ops path — URI and `OpenAPI` path are identical (all
/// parameters travel in the query string, none in the path).
const OPS_PATHS: [&str; 8] = [
    "/v1/admin/ops/overview",
    "/v1/admin/ops/runs",
    "/v1/admin/ops/runs/summary",
    "/v1/admin/ops/backfill",
    "/v1/admin/ops/freezes",
    "/v1/admin/ops/review-health",
    "/v1/admin/ops/deliveries",
    "/v1/admin/ops/extraction-costs",
];

fn test_app(pool: &PgPool) -> Router {
    api::app(
        pool.clone(),
        api::ApiConfig {
            admin_token: Some(TEST_ADMIN.to_owned()),
            ..api::ApiConfig::new()
        },
    )
}

/// Raw GET with an OPTIONAL admin token — unlike the contract suite this
/// harness controls the token per call, because fail-closed auth is itself
/// under test here.
async fn get_raw(app: &Router, uri: &str, token: Option<&str>) -> (StatusCode, HeaderMap, Bytes) {
    let mut request = Request::get(uri);
    if let Some(token) = token {
        request = request.header("x-admin-token", token);
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

async fn get_json(app: &Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let (status, _, bytes) = get_raw(app, uri, token).await;
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("GET {uri} returned non-JSON ({status}): {e}"));
    (status, body)
}

// --------------------------------------------------------- schema harness --
// Same technique as tests/contract.rs: `api::openapi_json()` is the exact
// byte producer behind `packages/contracts/openapi.json`, so validating
// bodies against it validates against the committed contract.

fn openapi_doc() -> Value {
    serde_json::from_str(&api::openapi_json().unwrap()).unwrap()
}

/// Standalone JSON Schema for one GET operation's response: the response
/// schema node (a `$ref`) wrapped in `allOf`, with the doc's `components`
/// carried along so internal `#/components/schemas/...` pointers resolve.
fn response_schema(doc: &Value, path: &str, status: &str) -> Value {
    let node =
        &doc["paths"][path]["get"]["responses"][status]["content"]["application/json"]["schema"];
    assert!(
        node.is_object(),
        "contract must declare a JSON response schema for GET {path} {status}"
    );
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "allOf": [node.clone()],
        "components": doc["components"].clone(),
    })
}

fn validator_for(doc: &Value, path: &str, status: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&response_schema(doc, path, status)).unwrap()
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

fn run_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

// ------------------------------------------------------------------ tests --

/// Plan test (1): the whole ops subtree sits behind the admin gate. A missing
/// token is `401 admin_token_required`; a wrong token is
/// `401 invalid_admin_token`; and with `ADMIN_TOKEN` unset the surface does
/// not exist AT ALL (`401 admin_disabled`, even when a token is presented) —
/// fail closed, never "open until configured".
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn ops_endpoints_require_the_admin_token_and_fail_closed(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);
    let doc = openapi_doc();

    for path in OPS_PATHS {
        let err = validator_for(&doc, path, "401");
        let (status, body) = get_json(&app, path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        assert_valid(&err, &body);
        assert_eq!(body["error"]["code"], "admin_token_required", "{path}");
    }

    // A wrong token is rejected with its own stable code.
    let (status, body) = get_json(&app, "/v1/admin/ops/overview", Some("not-the-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "invalid_admin_token");

    // ADMIN_TOKEN unset: the surface is disabled, not open (fail closed).
    let disabled = api::app(pool.clone(), api::ApiConfig::new());
    for path in OPS_PATHS {
        let (status, body) = get_json(&disabled, path, Some(TEST_ADMIN)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(body["error"]["code"], "admin_disabled", "{path}");
    }
}

/// Plan test (2): with the token, every ops endpoint answers `200` and the
/// body validates against the emitted `OpenAPI` schema — the same document
/// committed to `packages/contracts/openapi.json`. The worker-written tables
/// (sentinel/drift/delivery/sample-audit) are seeded with one row each so the
/// four row DTOs are validated WITH data, not as vacuously-valid empty arrays.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn every_ops_endpoint_answers_200_and_matches_the_contract(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    seed_ops_extras(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();

    // The validator must have teeth (refs resolved, shape enforced).
    let overview_ok = validator_for(&doc, "/v1/admin/ops/overview", "200");
    assert!(
        overview_ok
            .validate(&json!({ "totals": "not-an-object" }))
            .is_err(),
        "the contract validator must have teeth"
    );

    for path in OPS_PATHS {
        let ok = validator_for(&doc, path, "200");
        let (status, body) = get_json(&app, path, Some(TEST_ADMIN)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body:#}");
        assert_valid(&ok, &body);
    }
    let healthz_ok = validator_for(&doc, "/healthz", "200");
    let (status, body) = get_json(&app, "/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&healthz_ok, &body);

    // The seeded worker-table rows actually travelled through the row DTOs —
    // an empty array would have validated no per-item shape at all.
    let (_, freezes) = get_json(&app, "/v1/admin/ops/freezes", Some(TEST_ADMIN)).await;
    assert_eq!(
        freezes["sources"].as_array().unwrap().len(),
        1,
        "the seeded sentinel_watch row decoded through SentinelSource"
    );
    assert_eq!(
        freezes["drift"].as_array().unwrap().len(),
        1,
        "the seeded drift_report row decoded through DriftReportEntry"
    );
    let (_, deliveries) = get_json(&app, "/v1/admin/ops/deliveries", Some(TEST_ADMIN)).await;
    assert_eq!(
        deliveries["dead_recent"].as_array().unwrap().len(),
        1,
        "the seeded dead delivery decoded through DeadDelivery"
    );
    let (_, review) = get_json(&app, "/v1/admin/ops/review-health", Some(TEST_ADMIN)).await;
    let audit = review["sample_audit"].as_array().unwrap();
    assert_eq!(
        audit.len(),
        1,
        "the seeded sample_audit row decoded through SampleAuditSlice"
    );
    assert_eq!(audit[0]["confirmed"], json!(1));

    // Spot truths straight from the fixture seed (5 filings → 13 Gold rows,
    // all unverified; 5 roster politicians; the pipeline's one open task).
    let (_, overview) = get_json(&app, "/v1/admin/ops/overview", Some(TEST_ADMIN)).await;
    assert_eq!(overview["totals"]["filings"], json!(5));
    assert_eq!(overview["totals"]["gold_records"], json!(13));
    assert_eq!(overview["totals"]["gold_unverified"], json!(13));
    assert_eq!(overview["totals"]["politicians"], json!(5));
    assert_eq!(overview["totals"]["review_open"], json!(1));
    assert_eq!(overview["totals"]["frozen_regimes"], json!(0));
    assert!(
        overview["runs_24h"]["started"].as_i64().unwrap() > 0,
        "the seeding pipeline recorded runs"
    );
    assert_eq!(
        overview["extraction_month"]["hard_cap_usd"],
        json!("200.00"),
        "the founder-set HARD CAP travels as a decimal string (invariant 7)"
    );
    assert!(
        overview["last_activity"]["last_publish_succeeded_at"].is_string(),
        "publish finished during seeding"
    );
}

/// Plan test (3): `/backfill` buckets `us_house` by filing year — every bucket
/// carries a real (non-null) year, and filings/documents/gold per year equal
/// an independently written SQL predicate over the fixture seed.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn backfill_reports_us_house_years_and_gold_from_the_fixtures(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/admin/ops/backfill", "200");

    let (status, body) = get_json(&app, "/v1/admin/ops/backfill", Some(TEST_ADMIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &body);

    let regimes = body["regimes"].as_array().unwrap();
    assert_eq!(regimes.len(), 1, "one seeded regime with filings");
    let us = &regimes[0];
    assert_eq!(us["regime_code"], json!("us_house"));
    assert_eq!(us["regime_id"], json!(us_house::seed::REGIME_ID));
    assert_eq!(us["jurisdiction_id"], json!("us"));

    // Whole-history totals against the fixture ground truth.
    let totals = &us["totals"];
    assert_eq!(totals["filings"], json!(5));
    assert_eq!(totals["gold_records"], json!(13));
    assert_eq!(totals["gold_unverified"], json!(13));
    assert_eq!(totals["review_open"], json!(1), "the pipeline's one task");
    let bronze: i64 = sqlx::query_scalar("select count(distinct raw_document_id) from filing")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(totals["bronze_documents"], json!(bronze));
    let silver: i64 = sqlx::query_scalar("select count(*) from stg_us_house")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        totals["silver_rows"],
        json!(silver),
        "every Silver row's document fed a us_house filing"
    );

    // Stage progress mirrors pipeline_run for the adapter code.
    let publish_succeeded: i64 = sqlx::query_scalar(
        "select count(*) from pipeline_run \
         where adapter = 'us_house' and stage = 'publish' and status = 'succeeded'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let publish_stage = us["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == "publish")
        .unwrap();
    assert_eq!(publish_stage["succeeded"], json!(publish_succeeded));

    // Year buckets: the live binding always populates filed_date, so no
    // filing may land in the unknown-year bucket.
    let null_dates: i64 =
        sqlx::query_scalar("select count(*) from filing where filed_date is null")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(null_dates, 0, "us_house always carries filed_date");
    let years = us["years"].as_array().unwrap();
    assert!(!years.is_empty());
    let expected: Vec<(i32, i64, i64, i64)> = sqlx::query_as(
        "select extract(year from filed_date)::int as year, count(*), \
                count(distinct raw_document_id), count(distinct politician_id) \
         from filing group by 1 order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(years.len(), expected.len());
    let mut gold_sum = 0;
    for (bucket, (year, filings, documents, politicians)) in years.iter().zip(&expected) {
        assert_eq!(
            bucket["year"],
            json!(year),
            "buckets carry real years, oldest first — never null here"
        );
        assert_eq!(bucket["filings"], json!(filings));
        assert_eq!(bucket["documents"], json!(documents));
        assert_eq!(bucket["politicians_with_filings"], json!(politicians));

        // Gold per year equals an independently written join.
        let (gold, unverified): (i64, i64) = sqlx::query_as(
            "select count(*), count(*) filter (where r.verification_state = 'unverified') \
             from disclosure_record r join filing f on f.id = r.filing_id \
             where extract(year from f.filed_date)::int = $1",
        )
        .bind(year)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(bucket["gold_records"], json!(gold));
        assert_eq!(bucket["gold_unverified"], json!(unverified));
        gold_sum += gold;

        // Roster denominator: mandates active during the year, written as a
        // range predicate (independent of the endpoint's generate_series).
        let roster: i64 = sqlx::query_as::<_, (i64,)>(
            "select count(distinct m.politician_id) from mandate m \
             join disclosure_regime r \
               on r.jurisdiction_id = m.jurisdiction_id and r.body = m.body \
             where r.id = $1 and extract(year from m.start_date)::int <= $2 \
               and (m.end_date is null or extract(year from m.end_date)::int >= $2)",
        )
        .bind(us_house::seed::REGIME_ID)
        .bind(year)
        .fetch_one(&pool)
        .await
        .unwrap()
        .0;
        let expected_roster = (roster > 0).then_some(roster);
        assert_eq!(
            bucket["roster_members"],
            json!(expected_roster),
            "roster denominator for year {year}"
        );
    }
    assert_eq!(gold_sum, 13, "year buckets partition all fixture gold");

    // The adapter filter matches the resolved code, or nothing.
    let (status, filtered) = get_json(
        &app,
        "/v1/admin/ops/backfill?adapter=us_house",
        Some(TEST_ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["regimes"].as_array().unwrap().len(), 1);
    let (status, filtered) =
        get_json(&app, "/v1/admin/ops/backfill?adapter=br", Some(TEST_ADMIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["regimes"].as_array().unwrap().len(), 0);
}

/// Plan test (4): `/runs` — the stage filter equals an independent SQL
/// predicate, and keyset pagination by `id desc` makes page 2 strictly older
/// (every id sorts below the cursor) with no overlap or gap.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn runs_filter_by_stage_and_keyset_paginate_strictly_older(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/admin/ops/runs", "200");

    // Ground truth: every run id, newest first (ULID desc = claim-time desc).
    let all: Vec<String> = sqlx::query_scalar("select id from pipeline_run order by id desc")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        all.len() >= 4,
        "the seeding pipeline recorded runs: {all:?}"
    );

    // Page 1.
    let (status, page1) = get_json(&app, "/v1/admin/ops/runs?limit=2", Some(TEST_ADMIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page1);
    assert_eq!(run_ids(&page1), all[..2]);
    let cursor = page1["next_cursor"].as_str().unwrap().to_owned();
    assert_eq!(cursor, all[1], "cursor = last id of the page");

    // Page 2: strictly older than the cursor, exactly the next slice.
    let (status, page2) = get_json(
        &app,
        &format!("/v1/admin/ops/runs?limit=2&cursor={cursor}"),
        Some(TEST_ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page2);
    let ids2 = run_ids(&page2);
    assert_eq!(ids2, all[2..4]);
    assert!(
        ids2.iter().all(|id| id.as_str() < cursor.as_str()),
        "every page-2 id sorts strictly below the cursor (older claim)"
    );

    // Full listing exhausts with no cursor.
    let (status, full) = get_json(&app, "/v1/admin/ops/runs?limit=200", Some(TEST_ADMIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(run_ids(&full), all);
    assert!(full["next_cursor"].is_null());

    // Stage filter equals the independent predicate.
    let publishes: Vec<String> =
        sqlx::query_scalar("select id from pipeline_run where stage = 'publish' order by id desc")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!publishes.is_empty(), "seeding published fixtures");
    let (status, page) = get_json(
        &app,
        "/v1/admin/ops/runs?stage=publish&limit=200",
        Some(TEST_ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &page);
    assert_eq!(run_ids(&page), publishes);
    for item in page["items"].as_array().unwrap() {
        assert_eq!(item["stage"], json!("publish"));
        assert_eq!(item["adapter"], json!("us_house"));
    }

    // A time window in the past matches nothing (binds actually apply).
    let (status, page) = get_json(
        &app,
        "/v1/admin/ops/runs?until=2000-01-01T00:00:00Z",
        Some(TEST_ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 0);
    assert!(page["next_cursor"].is_null());

    // Malformed inputs reject in the envelope.
    let err = validator_for(&doc, "/v1/admin/ops/runs", "400");
    for (uri, code) in [
        ("/v1/admin/ops/runs?status=bogus", "invalid_status"),
        ("/v1/admin/ops/runs?cursor=not-a-ulid", "invalid_cursor"),
        ("/v1/admin/ops/runs?limit=0", "invalid_limit"),
        ("/v1/admin/ops/runs?limit=201", "invalid_limit"),
    ] {
        let (status, body) = get_json(&app, uri, Some(TEST_ADMIN)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_valid(&err, &body);
        assert_eq!(body["error"]["code"], json!(code), "{uri}");
    }
}

/// Plan test (5): `/extraction-costs` reports one row per requested month,
/// all zeros — real parse runs exist from the seeding but none carries a
/// `stats.extraction` block (goal 021 Phase 2 lands it), which proves the
/// SQL is null-tolerant instead of 500-ing.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn extraction_costs_report_zero_months_without_extraction_stats(pool: PgPool) {
    seed_via_pipeline(&pool).await;
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/v1/admin/ops/extraction-costs", "200");

    // The null-tolerance claim needs real parse rows WITHOUT the block.
    let (parse_runs, with_extraction): (i64, i64) = sqlx::query_as(
        "select count(*), count(*) filter (where stats ? 'extraction') \
         from pipeline_run where stage = 'parse'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(parse_runs > 0, "seeding recorded parse runs");
    assert_eq!(with_extraction, 0, "nothing writes stats.extraction yet");

    let (status, body) = get_json(
        &app,
        "/v1/admin/ops/extraction-costs?months=4",
        Some(TEST_ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_valid(&ok, &body);
    assert_eq!(body["hard_cap_usd"], json!("200.00"));

    let months = body["months"].as_array().unwrap();
    assert_eq!(months.len(), 4, "generate_series fills every month");
    let labels: Vec<&str> = months
        .iter()
        .map(|m| m["month"].as_str().unwrap())
        .collect();
    assert!(
        labels.windows(2).all(|w| w[0] < w[1]),
        "months are distinct, oldest first: {labels:?}"
    );
    assert_eq!(
        labels.last().copied().unwrap(),
        chrono::Utc::now().format("%Y-%m").to_string(),
        "the window ends at the current month"
    );
    for month in months {
        assert_eq!(month["tokens_in"], json!(0), "{month:#}");
        assert_eq!(month["tokens_out"], json!(0), "{month:#}");
        assert_eq!(month["estimated_cost_usd"], json!("0.00"), "{month:#}");
        assert_eq!(month["extraction_runs"], json!(0), "{month:#}");
        assert_eq!(month["cache_entries_created"], json!(0), "{month:#}");
    }

    // The overview's current-month block reports the same zeros vs the cap.
    let (_, overview) = get_json(&app, "/v1/admin/ops/overview", Some(TEST_ADMIN)).await;
    let month = &overview["extraction_month"];
    assert_eq!(month["tokens_in"], json!(0));
    assert_eq!(month["tokens_out"], json!(0));
    assert_eq!(month["estimated_cost_usd"], json!("0.00"));
    assert_eq!(month["cap_utilization_pct"], json!(0.0));

    // Out-of-range windows reject in the envelope.
    let err = validator_for(&doc, "/v1/admin/ops/extraction-costs", "400");
    for uri in [
        "/v1/admin/ops/extraction-costs?months=0",
        "/v1/admin/ops/extraction-costs?months=25",
    ] {
        let (status, body) = get_json(&app, uri, Some(TEST_ADMIN)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_valid(&err, &body);
        assert_eq!(body["error"]["code"], "invalid_months", "{uri}");
    }
}

/// Plan test (6): `/healthz` answers `200` WITHOUT a token AND without an
/// `ETag` header — locking its mounting position after the middleware layers
/// (no auth, no ETag/304 on a probe) — while the gated ops surface behind
/// the layers still carries one.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn healthz_is_tokenless_and_carries_no_etag(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);
    let doc = openapi_doc();
    let ok = validator_for(&doc, "/healthz", "200");

    let (status, headers, bytes) = get_raw(&app, "/healthz", None).await;
    assert_eq!(status, StatusCode::OK, "liveness needs no token");
    assert!(
        headers.get("etag").is_none(),
        "a probe is never 304-cached (mounted after the etag layer)"
    );
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_valid(&ok, &body);
    assert_eq!(body["status"], json!("ok"));
    assert_eq!(body["db"], json!("ok"));

    // Contrast: the gated surface behind the layers DOES carry an ETag.
    let (status, headers, _) = get_raw(&app, "/v1/admin/ops/overview", Some(TEST_ADMIN)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers.get("etag").is_some(),
        "ops bodies are ETagged for poll-friendly 304s"
    );
}

/// Mechanical gate for the hardcoded Silver union: the ops SQL cannot
/// enumerate `stg_%` tables at runtime (sqlx wants `&'static str`), so a new
/// adapter's staging migration MUST be added to `TOTALS_SQL` and
/// `BACKFILL_SILVER_SQL` by hand — this test diffs `information_schema`
/// against both consts and fails closed the moment a `stg_%` table lands
/// without joining the union, instead of silently undercounting that regime's
/// `silver_rows` forever.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn silver_union_covers_every_stg_table(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    // `stg_meta` is the ONE known non-regime `stg_%` table (run linkage,
    // design §4.2 "one stg_<regime> table per adapter ... plus stg_meta") —
    // excluded by exact name so any other new supporting table still trips
    // the gate and forces a conscious exclusion here.
    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_name like 'stg\\_%' \
           and table_name <> 'stg_meta' \
         order by table_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        !tables.is_empty(),
        "at least one Silver staging table exists"
    );
    for table in &tables {
        for (name, sql) in [
            ("TOTALS_SQL", api::routes::ops::TOTALS_SQL),
            ("BACKFILL_SILVER_SQL", api::routes::ops::BACKFILL_SILVER_SQL),
        ] {
            // Word-boundary-ish match: "from stg_x " / "from stg_x)" so a
            // prefix table (stg_us vs stg_us_house) can never false-positive.
            let referenced =
                sql.contains(&format!("from {table} ")) || sql.contains(&format!("from {table})"));
            assert!(
                referenced,
                "Silver staging table {table} is missing from {name} — \
                 add it to the union in crates/api/src/routes/ops.rs or the \
                 ops surface undercounts silver_rows for its regime"
            );
        }
    }
}

/// Review follow-up: `/runs/summary` and `/freezes` reject malformed params
/// OVER HTTP, not only in the bare validator unit tests — if a refactor drops
/// a `validate_*` call from a handler, the bad value would flow into
/// `date_trunc($2, ...)`/the scope bind and 500 instead of the contract's
/// 400; this pins the wiring.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn summary_and_freezes_reject_bad_params_over_http(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);
    let doc = openapi_doc();

    for (uri, path, code) in [
        (
            "/v1/admin/ops/runs/summary?hours=0",
            "/v1/admin/ops/runs/summary",
            "invalid_hours",
        ),
        (
            "/v1/admin/ops/runs/summary?hours=721",
            "/v1/admin/ops/runs/summary",
            "invalid_hours",
        ),
        (
            "/v1/admin/ops/runs/summary?bucket=week",
            "/v1/admin/ops/runs/summary",
            "invalid_bucket",
        ),
        (
            "/v1/admin/ops/freezes?status=frozen",
            "/v1/admin/ops/freezes",
            "invalid_status",
        ),
    ] {
        let err = validator_for(&doc, path, "400");
        let (status, body) = get_json(&app, uri, Some(TEST_ADMIN)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_valid(&err, &body);
        assert_eq!(body["error"]["code"], json!(code), "{uri}");
    }
}
