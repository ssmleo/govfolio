//! `worker::migrate_local_to_prod` coverage (per founder-directed policy,
//! 2026-07-09 session direction — pending write-back into a future root
//! `CLAUDE.md` invariant): proves the LOCAL -> PROD row-copy logic against
//! TWO local ephemeral Postgres databases standing in for "local" and
//! "prod" — there is no way to safely test against real prod, and the
//! module doc explains why two local databases are sufficient to prove the
//! cross-database copy + idempotency logic. The `#[sqlx::test]` macro
//! provisions the LOCAL
//! stand-in; the PROD stand-in is created/dropped manually (same mechanism
//! the macro itself uses, against `DATABASE_URL`'s server) since the macro
//! only provisions one ephemeral database per test function.
//!
//! GCS is never touched: [`FakeUploader`] fakes [`BronzeUploader`] instead
//! (per the design's own instruction — no test may depend on real
//! GCS/gcloud).
//!
//! DB-gated like the other sqlx suites (`--ignored` + postgres on
//! `DATABASE_URL`/`localhost:5433`).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

use chrono::{NaiveDate, Utc};
use sqlx::{AssertSqlSafe, PgPool};

use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed, seed_regime};
use worker::migrate_local_to_prod::{BronzeUploader, UploadOutcome, migrate_regime};

const REGIME_ID: &str = "0TESTREG00000000000000001";
const JURISDICTION_ID: &str = "zz";
const BODY: &str = "Test Body";
const POLITICIAN_ID: &str = "0POL0000000000000000TEST1";
const MANDATE_ID: &str = "0MAN0000000000000000TEST1";
const RAW_DOCUMENT_ID: &str = "0DOC0000000000000000TEST1";
const FILING_ID: &str = "0FIL0000000000000000TEST1";
const RECORD_ID: &str = "0REC0000000000000000TEST1";
const OUTBOX_ID: &str = "0OUT0000000000000000TEST1";
const REVIEW_TASK_FILING_ID: &str = "0RVW0000000000000000TES1";
const REVIEW_TASK_RECORD_ID: &str = "0RVW0000000000000000TES2";
const REVIEW_TASK_REGIME_ID: &str = "0RVW0000000000000000TES3";
const REVIEW_AUDIT_ID: &str = "0AUD0000000000000000TEST1";
const SHA256: &str = "aaaabbbbccccddddeeeeffff00001111222233334444555566667777888899aa";

/// Fakes [`BronzeUploader`]: never touches disk/GCS. Tracks which sha256s
/// it's already "uploaded" so a second `migrate_regime` call over the same
/// data reports `AlreadyPresent`, exactly mirroring real content-addressed
/// GCS idempotency.
struct FakeUploader {
    bucket: String,
    seen: Mutex<BTreeSet<String>>,
}

impl FakeUploader {
    fn new(bucket: &str) -> Self {
        Self {
            bucket: bucket.to_owned(),
            seen: Mutex::new(BTreeSet::new()),
        }
    }
}

impl BronzeUploader for FakeUploader {
    fn ensure_uploaded(&self, sha256: &str, _local_path: &Path) -> anyhow::Result<UploadOutcome> {
        let uri = format!("gs://{}/{sha256}", self.bucket);
        let mut seen = self.seen.lock().unwrap();
        if seen.insert(sha256.to_owned()) {
            Ok(UploadOutcome::Uploaded(uri))
        } else {
            Ok(UploadOutcome::AlreadyPresent(uri))
        }
    }
}

/// Creates a fresh, empty ephemeral database on the SAME server
/// `#[sqlx::test]` itself uses (`DATABASE_URL`, default matching the local
/// dev Postgres convention) — standing in for "prod". Returns the pool, the
/// db name, and the admin URL (both needed to drop it at teardown).
async fn create_ephemeral_prod() -> (PgPool, String, String) {
    let base = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/govfolio".to_owned());
    let admin_url = with_database(&base, "postgres");
    let admin = PgPool::connect(&admin_url).await.unwrap();
    let db_name = format!(
        "govfolio_test_prod_{}",
        ulid::Ulid::new().to_string().to_lowercase()
    );
    // db_name is a locally-generated ULID suffix, not external input, hence AssertSqlSafe.
    sqlx::query(AssertSqlSafe(format!("create database \"{db_name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPool::connect(&with_database(&base, &db_name))
        .await
        .unwrap();
    (pool, db_name, admin_url)
}

async fn drop_ephemeral_prod(pool: PgPool, admin_url: &str, db_name: &str) {
    pool.close().await;
    if let Ok(admin) = PgPool::connect(admin_url).await {
        let _ = sqlx::query(AssertSqlSafe(format!(
            "drop database if exists \"{db_name}\""
        )))
        .execute(&admin)
        .await;
    }
}

fn with_database(url: &str, db_name: &str) -> String {
    let base = url.rsplit_once('/').map_or(url, |(base, _)| base);
    format!("{base}/{db_name}")
}

/// Seeds LOCAL with one complete regime's worth of Gold data: a regime +
/// jurisdiction, one politician (+ alias + mandate), one `raw_document`, one
/// filing, one `disclosure_record`, one `outbox_event`, THREE review
/// tasks — one targeting the filing, one targeting the record (both must
/// migrate), and one targeting the regime itself (must NOT migrate — see
/// `worker::migrate_local_to_prod`'s module doc) — and one `review_audit`
/// row against the filing-scoped task (must migrate).
// A linear sequence of one-row-per-table seed inserts, not real complexity.
#[allow(clippy::too_many_lines)]
async fn seed_local(local: &PgPool) {
    govfolio_core::db::migrate(local).await.unwrap();
    seed_regime(
        local,
        &RegimeSeed {
            jurisdiction: JurisdictionSeed {
                id: JURISDICTION_ID.to_owned(),
                name: "Testland".to_owned(),
                iso_code: None,
                level: "national".to_owned(),
            },
            regime_id: REGIME_ID.to_owned(),
            body: BODY.to_owned(),
            regime_type: "transaction_report".to_owned(),
            value_precision: "exact".to_owned(),
            cadence: None,
            disclosure_lag_days: None,
            source_url: None,
            effective_from: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        },
    )
    .await
    .unwrap();

    sqlx::query("insert into politician (id, canonical_name) values ($1, $2)")
        .bind(POLITICIAN_ID)
        .bind("Test Politician")
        .execute(local)
        .await
        .unwrap();
    sqlx::query("insert into politician_alias (politician_id, alias) values ($1, $2)")
        .bind(POLITICIAN_ID)
        .bind("Test Politician")
        .execute(local)
        .await
        .unwrap();
    sqlx::query(
        "insert into mandate (id, politician_id, jurisdiction_id, body, role, district, \
           start_date) values ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(MANDATE_ID)
    .bind(POLITICIAN_ID)
    .bind(JURISDICTION_ID)
    .bind(BODY)
    .bind("Member")
    .bind("AT-1")
    .bind(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap())
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(RAW_DOCUMENT_ID)
    .bind(format!("/local/bronze/{SHA256}"))
    .bind(SHA256)
    .bind("application/pdf")
    .bind(Utc::now())
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into filing (id, regime_id, politician_id, raw_document_id, external_id, \
           filing_type, discovered_at) values ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(FILING_ID)
    .bind(REGIME_ID)
    .bind(POLITICIAN_ID)
    .bind(RAW_DOCUMENT_ID)
    .bind("EXT1")
    .bind("P")
    .bind(Utc::now())
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
            asset_class, as_of_date, extracted_by, fingerprint) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(RECORD_ID)
    .bind(FILING_ID)
    .bind(POLITICIAN_ID)
    .bind(REGIME_ID)
    .bind("Test Asset")
    .bind("holding")
    .bind("equity")
    .bind(NaiveDate::from_ymd_opt(2020, 6, 1).unwrap())
    .bind("test")
    .bind("fp-test-1")
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into outbox_event (id, kind, payload, dispatched_at) values ($1, $2, $3, $4)",
    )
    .bind(OUTBOX_ID)
    .bind("disclosure_record.published")
    .bind(serde_json::json!({"record_id": RECORD_ID, "regime_id": REGIME_ID}))
    .bind(Utc::now())
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, $2, $3, $4)",
    )
    .bind(REVIEW_TASK_FILING_ID)
    .bind("filing")
    .bind(FILING_ID)
    .bind("test_filing_reason")
    .execute(local)
    .await
    .unwrap();
    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, $2, $3, $4)",
    )
    .bind(REVIEW_TASK_RECORD_ID)
    .bind("disclosure_record")
    .bind(RECORD_ID)
    .bind("test_record_reason")
    .execute(local)
    .await
    .unwrap();
    // Regime-level task: must NOT migrate (see module doc — local
    // operational bookkeeping, not filing/record-scoped business data).
    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, $2, $3, $4)",
    )
    .bind(REVIEW_TASK_REGIME_ID)
    .bind("regime")
    .bind(REGIME_ID)
    .bind("publish_blocked_frozen")
    .execute(local)
    .await
    .unwrap();

    sqlx::query(
        "insert into review_audit (id, review_task_id, reviewer, verdict, outcome) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(REVIEW_AUDIT_ID)
    .bind(REVIEW_TASK_FILING_ID)
    .bind("test-reviewer")
    .bind("confirm")
    .bind("applied")
    .execute(local)
    .await
    .unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn first_run_migrates_everything_second_run_is_a_no_op(local: PgPool) {
    seed_local(&local).await;
    let (prod, prod_db, admin_url) = create_ephemeral_prod().await;
    govfolio_core::db::migrate(&prod).await.unwrap();
    let uploader = FakeUploader::new("test-bucket");
    let bronze_root = std::env::temp_dir().join(format!(
        "govfolio-migrate-test-bronze-{}",
        std::process::id()
    ));

    let first = migrate_regime(&local, &prod, REGIME_ID, &bronze_root, &uploader)
        .await
        .unwrap();
    assert_eq!(first.politician.migrated, 1);
    assert_eq!(first.politician_alias.migrated, 1);
    assert_eq!(first.mandate.migrated, 1);
    assert_eq!(first.raw_document.migrated, 1);
    assert_eq!(
        first.raw_document_uploaded, 1,
        "first run must upload the bytes"
    );
    assert_eq!(first.raw_document_gcs_already_present, 0);
    assert_eq!(first.filing.migrated, 1);
    assert_eq!(first.disclosure_record.migrated, 1);
    assert_eq!(first.outbox_event.migrated, 1);
    // Exactly the filing- and record-scoped tasks — NOT the regime one.
    assert_eq!(first.review_task.migrated, 2);
    assert_eq!(first.review_audit.migrated, 1);
    assert_eq!(first.politician.failed, 0);
    assert_eq!(first.filing.failed, 0);
    assert_eq!(first.disclosure_record.failed, 0);
    assert_eq!(first.review_task.failed, 0);
    assert_eq!(first.review_audit.failed, 0);

    // storage_uri was rewritten to gs://, and dispatched_at survived intact
    // (invariant 2 / the "never causes a real alert" requirement).
    let storage_uri: String =
        sqlx::query_scalar("select storage_uri from raw_document where id = $1")
            .bind(RAW_DOCUMENT_ID)
            .fetch_one(&prod)
            .await
            .unwrap();
    assert_eq!(storage_uri, format!("gs://test-bucket/{SHA256}"));
    let dispatched_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("select dispatched_at from outbox_event where id = $1")
            .bind(OUTBOX_ID)
            .fetch_one(&prod)
            .await
            .unwrap();
    assert!(
        dispatched_at.is_some(),
        "dispatched_at must never be NULLed out"
    );
    let regime_task_count: i64 =
        sqlx::query_scalar("select count(*) from review_task where id = $1")
            .bind(REVIEW_TASK_REGIME_ID)
            .fetch_one(&prod)
            .await
            .unwrap();
    assert_eq!(
        regime_task_count, 0,
        "regime-scoped review tasks must not migrate"
    );
    let audit_row: (String, String, String) =
        sqlx::query_as("select review_task_id, verdict, outcome from review_audit where id = $1")
            .bind(REVIEW_AUDIT_ID)
            .fetch_one(&prod)
            .await
            .unwrap();
    assert_eq!(
        audit_row,
        (
            REVIEW_TASK_FILING_ID.to_owned(),
            "confirm".to_owned(),
            "applied".to_owned()
        ),
        "review_audit row must migrate against its review_task"
    );

    let second = migrate_regime(&local, &prod, REGIME_ID, &bronze_root, &uploader)
        .await
        .unwrap();
    assert_eq!(second.politician.migrated, 0);
    assert_eq!(second.politician.already_present, 1);
    assert_eq!(second.politician_alias.migrated, 0);
    assert_eq!(second.politician_alias.already_present, 1);
    assert_eq!(second.mandate.migrated, 0);
    assert_eq!(second.mandate.already_present, 1);
    assert_eq!(second.raw_document.migrated, 0);
    assert_eq!(second.raw_document.already_present, 1);
    assert_eq!(
        second.raw_document_uploaded, 0,
        "second run must not re-upload"
    );
    assert_eq!(second.raw_document_gcs_already_present, 1);
    assert_eq!(second.filing.migrated, 0);
    assert_eq!(second.filing.already_present, 1);
    assert_eq!(second.disclosure_record.migrated, 0);
    assert_eq!(second.disclosure_record.already_present, 1);
    assert_eq!(second.outbox_event.migrated, 0);
    assert_eq!(second.outbox_event.already_present, 1);
    assert_eq!(second.review_task.migrated, 0);
    assert_eq!(second.review_task.already_present, 2);
    assert_eq!(second.review_audit.migrated, 0);
    assert_eq!(second.review_audit.already_present, 1);

    let audit_count: i64 = sqlx::query_scalar("select count(*) from review_audit where id = $1")
        .bind(REVIEW_AUDIT_ID)
        .fetch_one(&prod)
        .await
        .unwrap();
    assert_eq!(audit_count, 1, "second run must not duplicate the row");

    let record_count: i64 =
        sqlx::query_scalar("select count(*) from disclosure_record where id = $1")
            .bind(RECORD_ID)
            .fetch_one(&prod)
            .await
            .unwrap();
    assert_eq!(record_count, 1, "second run must not duplicate the row");

    let _ = std::fs::remove_dir_all(&bronze_root);
    drop_ephemeral_prod(prod, &admin_url, &prod_db).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn never_updates_an_existing_prod_row(local: PgPool) {
    seed_local(&local).await;
    let (prod, prod_db, admin_url) = create_ephemeral_prod().await;
    govfolio_core::db::migrate(&prod).await.unwrap();
    // Pre-seed PROD's regime/jurisdiction + politician + raw_document +
    // filing (FK targets), then a disclosure_record under the SAME id LOCAL
    // has, but with different content — as if it landed via an earlier
    // migration or a since-corrected local re-normalization. Per invariant
    // 1, this migration must be a PURE INSERT: it must never UPDATE this
    // row back toward LOCAL's differing content.
    seed_regime(
        &prod,
        &RegimeSeed {
            jurisdiction: JurisdictionSeed {
                id: JURISDICTION_ID.to_owned(),
                name: "Testland".to_owned(),
                iso_code: None,
                level: "national".to_owned(),
            },
            regime_id: REGIME_ID.to_owned(),
            body: BODY.to_owned(),
            regime_type: "transaction_report".to_owned(),
            value_precision: "exact".to_owned(),
            cadence: None,
            disclosure_lag_days: None,
            source_url: None,
            effective_from: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        },
    )
    .await
    .unwrap();
    sqlx::query("insert into politician (id, canonical_name) values ($1, $2)")
        .bind(POLITICIAN_ID)
        .bind("Test Politician")
        .execute(&prod)
        .await
        .unwrap();
    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(RAW_DOCUMENT_ID)
    .bind(format!("gs://test-bucket/{SHA256}"))
    .bind(SHA256)
    .bind("application/pdf")
    .bind(Utc::now())
    .execute(&prod)
    .await
    .unwrap();
    sqlx::query(
        "insert into filing (id, regime_id, politician_id, raw_document_id, external_id, \
           filing_type, discovered_at) values ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(FILING_ID)
    .bind(REGIME_ID)
    .bind(POLITICIAN_ID)
    .bind(RAW_DOCUMENT_ID)
    .bind("EXT1")
    .bind("P")
    .bind(Utc::now())
    .execute(&prod)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
            asset_class, as_of_date, extracted_by, fingerprint) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(RECORD_ID)
    .bind(FILING_ID)
    .bind(POLITICIAN_ID)
    .bind(REGIME_ID)
    .bind("PROD-ONLY DESCRIPTION — must survive untouched")
    .bind("holding")
    .bind("equity")
    .bind(NaiveDate::from_ymd_opt(2020, 6, 1).unwrap())
    .bind("prod-seed")
    .bind("fp-test-1")
    .execute(&prod)
    .await
    .unwrap();

    let uploader = FakeUploader::new("test-bucket");
    let bronze_root = std::env::temp_dir().join(format!(
        "govfolio-migrate-test-bronze-noupdate-{}",
        std::process::id()
    ));
    let report = migrate_regime(&local, &prod, REGIME_ID, &bronze_root, &uploader)
        .await
        .unwrap();
    assert_eq!(
        report.disclosure_record.migrated, 0,
        "the pre-existing row must be counted already_present, not migrated"
    );
    assert_eq!(report.disclosure_record.already_present, 1);

    let (asset_description_raw, extracted_by): (String, String) = sqlx::query_as(
        "select asset_description_raw, extracted_by from disclosure_record where id = $1",
    )
    .bind(RECORD_ID)
    .fetch_one(&prod)
    .await
    .unwrap();
    assert_eq!(
        asset_description_raw, "PROD-ONLY DESCRIPTION — must survive untouched",
        "invariant 1: this migration must never UPDATE an existing PROD row"
    );
    assert_eq!(extracted_by, "prod-seed");

    let _ = std::fs::remove_dir_all(&bronze_root);
    drop_ephemeral_prod(prod, &admin_url, &prod_db).await;
}
