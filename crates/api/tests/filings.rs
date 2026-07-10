//! `GET /v1/filings/{id}/document`: serves the archived Bronze copy, gated by
//! the SAME freshness bound as `/v1/records/{id}` (goal 050 — a filing's
//! document must never be more visible than its own records).
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use api::ApiConfig;

fn test_app(pool: &sqlx::PgPool) -> Router {
    api::app(pool.clone(), ApiConfig::new())
}

/// Test admin bootstrap token — mirrors the e2e/reviewer test convention of
/// an obvious dummy value (`crates/api/tests/*` review-surface suites use
/// the same shape).
const TEST_ADMIN_TOKEN: &str = "filings-test-admin-token";

fn test_app_with_admin(pool: &sqlx::PgPool) -> Router {
    api::app(
        pool.clone(),
        ApiConfig {
            admin_token: Some(TEST_ADMIN_TOKEN.to_owned()),
            ..ApiConfig::new()
        },
    )
}

struct Seeded {
    filing_id: String,
    document_path: std::path::PathBuf,
}

/// One filing + `raw_document` + `disclosure_record`, `discovered_at` controlled
/// by `age` (mirrors `crates/api/tests/tiers.rs::seed_two_ages`'s shape, but
/// returns the filing id and writes a REAL temp file so the document route
/// can actually read bytes back).
async fn seed_filing(pool: &sqlx::PgPool, age: Duration, bytes: &[u8], mime: &str) -> Seeded {
    govfolio_core::db::migrate(pool).await.unwrap();
    let politician_id = ulid::Ulid::new().to_string();
    let regime_id = ulid::Ulid::new().to_string();
    let raw_id = ulid::Ulid::new().to_string();
    let filing_id = ulid::Ulid::new().to_string();
    let record_id = ulid::Ulid::new().to_string();

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

    let document_path = std::env::temp_dir().join(format!("govfolio-filing-doc-test-{raw_id}.bin"));
    tokio::fs::write(&document_path, bytes).await.unwrap();
    let storage_uri = format!("file://{}", document_path.display());

    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
         values ($1, $2, $3, $4, now())",
    )
    .bind(&raw_id)
    .bind(&storage_uri)
    .bind(format!("{raw_id}-sha256"))
    .bind(mime)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into filing \
           (id, regime_id, politician_id, raw_document_id, external_id, filing_type, discovered_at) \
         values ($1, $2, $3, $4, 'ext-1', 'ptr', $5)",
    )
    .bind(&filing_id)
    .bind(&regime_id)
    .bind(&politician_id)
    .bind(&raw_id)
    .bind(Utc::now() - age)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
            asset_class, side, transaction_date, extracted_by, fingerprint) \
         values ($1, $2, $3, $4, 'test asset', 'transaction', 'equity', 'buy', '2026-06-01', \
                 'filings-test', $5)",
    )
    .bind(&record_id)
    .bind(&filing_id)
    .bind(&politician_id)
    .bind(&regime_id)
    .bind(format!("fp-{raw_id}"))
    .execute(pool)
    .await
    .unwrap();

    Seeded {
        filing_id,
        document_path,
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn serves_the_archived_bytes_with_the_sniffed_content_type(pool: sqlx::PgPool) {
    let seeded = seed_filing(
        &pool,
        Duration::hours(25),
        b"%PDF-1.7 test",
        "application/pdf",
    )
    .await;
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/filings/{}/document", seeded.filing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/pdf"
    );
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff",
        "served documents must never be browser-MIME-sniffed"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"%PDF-1.7 test");
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn unknown_filing_is_404(pool: sqlx::PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/filings/01UNKNOWNFILINGID000000000/document")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn embargoed_filing_document_404s_for_anonymous_free_tier_same_as_the_record_would(
    pool: sqlx::PgPool,
) {
    // Discovered 1 minute ago: invisible to the free tier (goal 050's 24h
    // delay) — this is the exact scenario a naive filing-id-only lookup
    // would leak.
    let seeded = seed_filing(&pool, Duration::minutes(1), b"fresh bytes", "text/html").await;
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/filings/{}/document", seeded.filing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a not-yet-visible filing's document must 404 exactly like its record would"
    );
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}

// -------------------------------------------------- admin (reviewer) route --

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn admin_route_serves_a_fresh_filing_the_public_route_would_embargo(pool: sqlx::PgPool) {
    // Same freshness the previous test proved 404s on the PUBLIC route — the
    // whole point of the admin variant is that reviewers see this anyway
    // (mirrors review::get_review_task's RecordFilter::default() use).
    let seeded = seed_filing(&pool, Duration::minutes(1), b"fresh bytes", "text/html").await;
    let app = test_app_with_admin(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/admin/filings/{}/document", seeded.filing_id))
                .header("x-admin-token", TEST_ADMIN_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"fresh bytes");
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn admin_route_without_a_valid_token_is_401_not_a_silent_fallback_to_public_visibility(
    pool: sqlx::PgPool,
) {
    let seeded = seed_filing(&pool, Duration::minutes(1), b"fresh bytes", "text/html").await;
    let app = test_app_with_admin(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/admin/filings/{}/document", seeded.filing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}
