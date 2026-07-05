//! End-to-end local pipeline run over the real `us_house` fixtures (plan Task 9).
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
//!
//! Proves, against a live schema:
//! - Bronze rows (`raw_document`), Silver rows (`stg_us_house` + `stg_meta`), and
//!   Gold rows exist after one run; every Gold row is `unverified`;
//! - one `outbox_event` per inserted record, written in the same transaction
//!   (count match + join, plus a dedicated rollback test);
//! - `pipeline_run` rows carry unique idempotency keys + stats;
//! - a second run inserts NOTHING (row counts identical across all tables), and
//!   a forcibly replayed publish stage still inserts nothing (row-level
//!   `ON CONFLICT DO NOTHING`, invariant 4 — not just stage-skip);
//! - an unresolved filer fails closed: `review_task`, no filing, no Gold rows
//!   (invariant 3 — never guess).
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use sqlx::{AssertSqlSafe, PgPool};

use govfolio_core::domain::enums::{AssetClass, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::run::{FilingIdentity, LocalFiling, RunReport, Runner};
use pipeline::stages::publish::{FilingSpec, publish_filing};
use pipeline::stages::roster::{resolve_politician, seed_roster};
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;

/// Every table of migrations 0001+0002+0004 — the "second run inserts
/// nothing" assertion sweeps ALL of them.
const ALL_TABLES: &[&str] = &[
    "jurisdiction",
    "disclosure_regime",
    "politician",
    "politician_alias",
    "mandate",
    "instrument",
    "instrument_alias",
    "raw_document",
    "filing",
    "disclosure_record",
    "review_task",
    "pipeline_run",
    "outbox_event",
    "stg_us_house",
    "stg_meta",
    "extraction_cache",
];

async fn table_counts(pool: &PgPool) -> Vec<(String, i64)> {
    let mut counts = Vec::with_capacity(ALL_TABLES.len());
    for table in ALL_TABLES {
        // Test-constant identifiers only, hence AssertSqlSafe.
        let n: i64 = sqlx::query_scalar(AssertSqlSafe(format!("select count(*) from {table}")))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.push(((*table).to_owned(), n));
    }
    counts
}

fn evidence_xml(file: &str) -> String {
    let path = workspace_root()
        .join("docs")
        .join("regimes")
        .join("us-house")
        .join("evidence")
        .join(file);
    std::fs::read_to_string(path).unwrap()
}

fn evidence_index_xml() -> String {
    evidence_xml(
        "94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e.2026FD-slice.xml",
    )
}

/// The archived index slice for the goal-021 scanned paper fixture's filer
/// (Hon. Diana Harshbarger, TN01 — `DocID` 9115811).
fn evidence_slice_9115811() -> String {
    evidence_xml(
        "f312caf490ddb96fa4b2b4fc73cc67ad0eb335d004c9b4db82e3b48cd22b6bc7.2026FD-slice-9115811.xml",
    )
}

fn fixture_inputs() -> Vec<LocalFiling> {
    let root = fixtures_dir("us_house");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 5, "expected the T8 + goal-021 fixture cases");
    dirs.into_iter()
        .map(|dir| LocalFiling {
            path: dir.join("input.pdf"),
        })
        .collect()
}

fn temp_bronze_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("govfolio-e2e-{tag}-{}-{nanos}", std::process::id()))
}

async fn run_once(pool: &PgPool, bronze_root: &Path) -> RunReport {
    let adapter = UsHouseAdapter::default();
    let binding = UsHouseBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(bronze_root).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx).unwrap();
    runner.run_local(&fixture_inputs()).await.unwrap()
}

async fn migrate_and_seed_regime(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn full_local_run_publishes_and_second_run_inserts_nothing(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let mut roster = us_house::seed::roster_from_index_xml(&evidence_index_xml()).unwrap();
    roster.extend(us_house::seed::roster_from_index_xml(&evidence_slice_9115811()).unwrap());
    let seeded = seed_roster(&pool, &us_house::seed::regime_binding(), &roster)
        .await
        .unwrap();
    assert_eq!(
        seeded, 5,
        "four E1 slice members + the goal-021 paper filer"
    );

    let bronze_root = temp_bronze_root("idempotent");
    let report1 = run_once(&pool, &bronze_root).await;
    assert_eq!(report1.failed, Vec::<String>::new());
    assert_eq!(report1.filings, 5);
    assert_eq!(report1.gold_inserted, 13, "1+8+2+1+1 fixture rows");
    assert_eq!(report1.outbox_written, 13);

    // Bronze + Silver + linkage.
    let raw_docs: i64 = sqlx::query_scalar("select count(*) from raw_document")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw_docs, 5);
    let silver: i64 = sqlx::query_scalar("select count(*) from stg_us_house")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(silver, 13);
    let meta: i64 = sqlx::query_scalar(
        "select count(*) from stg_meta m join pipeline_run r on r.id = m.pipeline_run_id \
         where m.stg_table = 'stg_us_house'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(meta, 13, "every silver row is run-linked");

    // The scanned paper filing went through the LLM seam: llm@1-tagged rows
    // at the staged 0.9 confidence, resolved to the prefix-less paper alias.
    let (llm_rows, llm_confidence): (i64, Option<f32>) = sqlx::query_as(
        "select count(*), min(confidence) from stg_us_house \
         where extractor = 'us_house_ptr/llm@1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(llm_rows, 1, "the scanned fixture is the one LLM-path row");
    assert_eq!(
        llm_confidence,
        Some(0.9f32),
        "staged llm@1 wrapper confidence"
    );

    // Gold: all unverified, fingerprints present and unique.
    let states: Vec<String> =
        sqlx::query_scalar("select distinct verification_state from disclosure_record")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(states, ["unverified"]);
    let bad_fingerprints: i64 = sqlx::query_scalar(
        "select count(*) from disclosure_record where fingerprint !~ '^[0-9a-f]{64}$'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad_fingerprints, 0);

    // One outbox event per record, same-transaction (count match + 1:1 join).
    let (records, events, joined): (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from disclosure_record), \
                (select count(*) from outbox_event where kind = 'disclosure_record.published'), \
                (select count(*) from disclosure_record dr \
                   join outbox_event oe on oe.payload->>'record_id' = dr.id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(records, 13);
    assert_eq!(events, records, "one outbox event per gold record");
    assert_eq!(joined, records, "every record has its event");

    // pipeline_run: 3 stages x 5 filings, all succeeded, unique keys, stats kept.
    let (runs, keys, succeeded, with_stats): (i64, i64, i64, i64) = sqlx::query_as(
        "select count(*), count(distinct idempotency_key), \
                count(*) filter (where status = 'succeeded'), \
                count(*) filter (where stats <> '{}'::jsonb) from pipeline_run",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(runs, 15);
    assert_eq!(keys, 15, "idempotency keys are unique per stage unit");
    assert_eq!(succeeded, 15);
    assert_eq!(with_stats, 15, "every stage records audit stats");

    // Amendment routing (regime doc §3.7): the single Amended row opened a task.
    let amendment_targets: Vec<String> = sqlx::query_scalar(
        "select target_id from review_task \
         where reason = 'ptr_amendment_unlinked' and status = 'open'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let amended_record: String = sqlx::query_scalar(
        "select id from disclosure_record where details->>'filing_status_raw' = 'Amended'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(amendment_targets, [amended_record]);
    let tasks: i64 = sqlx::query_scalar("select count(*) from review_task")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tasks, 1, "no other review tasks on a clean run");

    // SECOND RUN INSERTS NOTHING — row counts identical across all tables.
    let counts1 = table_counts(&pool).await;
    let report2 = run_once(&pool, &bronze_root).await;
    assert_eq!(report2.failed, Vec::<String>::new());
    assert_eq!(report2.gold_inserted, 0);
    assert_eq!(report2.outbox_written, 0);
    let counts2 = table_counts(&pool).await;
    assert_eq!(counts1, counts2, "second run must insert nothing");

    // Row-level idempotency, not just stage replay: force the publish stages to
    // re-execute; ON CONFLICT DO NOTHING must still insert zero rows.
    sqlx::query("update pipeline_run set status = 'failed' where stage = 'publish'")
        .execute(&pool)
        .await
        .unwrap();
    let report3 = run_once(&pool, &bronze_root).await;
    assert_eq!(report3.failed, Vec::<String>::new());
    assert_eq!(report3.gold_inserted, 0);
    let counts3 = table_counts(&pool).await;
    assert_eq!(
        counts1, counts3,
        "replayed publish must insert nothing (invariant 4)"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn unresolved_filer_fails_closed_no_gold_plus_review_task(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    // Seed the roster WITHOUT the AK00 filer (typical_single_row's Begich).
    let mut roster: Vec<_> = us_house::seed::roster_from_index_xml(&evidence_index_xml())
        .unwrap()
        .into_iter()
        .filter(|m| m.district != "AK00")
        .collect();
    roster.extend(us_house::seed::roster_from_index_xml(&evidence_slice_9115811()).unwrap());
    assert_eq!(roster.len(), 4);
    seed_roster(&pool, &us_house::seed::regime_binding(), &roster)
        .await
        .unwrap();

    let bronze_root = temp_bronze_root("unresolved");
    let report = run_once(&pool, &bronze_root).await;
    assert_eq!(report.failed.len(), 1, "exactly one filing fails closed");
    assert!(
        report.failed[0].contains("20020055"),
        "the unresolved filing is identified: {:?}",
        report.failed
    );

    // No guessing: no filing row, no Gold rows for the unresolved filer.
    let (filings, gold): (i64, i64) = sqlx::query_as(
        "select (select count(*) from filing), (select count(*) from disclosure_record)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(filings, 4);
    assert_eq!(gold, 12, "8+2+1+1 rows from the four resolvable filings");

    let tasks: Vec<String> = sqlx::query_scalar(
        "select target_id from review_task \
         where reason = 'unresolved_filer' and status = 'open'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tasks, ["us_house:20020055"]);

    // Re-running retries the failed publish but must not duplicate the task.
    let counts1 = table_counts(&pool).await;
    let report2 = run_once(&pool, &bronze_root).await;
    assert_eq!(report2.failed.len(), 1);
    let counts2 = table_counts(&pool).await;
    assert_eq!(counts1, counts2, "fail-closed retry inserts nothing new");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn publish_writes_gold_and_outbox_in_one_transaction(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let regime = us_house::seed::regime_binding();
    let roster = us_house::seed::roster_from_index_xml(&evidence_index_xml()).unwrap();
    seed_roster(&pool, &regime, &roster).await.unwrap();
    let politician_id = resolve_politician(&pool, &regime, "Hon. Nicholas Begich III", "AK00")
        .await
        .unwrap()
        .unwrap(); // seeded filer must resolve

    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
         values ('01BX5ZZKBKACTAV9WEVGEMMVD9', 'file:///tmp/t.pdf', \
                 '3333333333333333333333333333333333333333333333333333333333333333', \
                 'application/pdf', now())",
    )
    .execute(&pool)
    .await
    .unwrap();

    let identity = FilingIdentity {
        external_id: "99990001".to_owned(),
        filer_name: "Hon. Nicholas Begich III".to_owned(),
        district: "AK00".to_owned(),
        filing_type: "P".to_owned(),
        filed_date: Some(NaiveDate::from_ymd_opt(2026, 6, 12).unwrap()),
    };
    let spec = FilingSpec {
        regime_id: &regime.regime_id,
        regime_code: "us_house",
        politician_id: &politician_id,
        raw_document_id: "01BX5ZZKBKACTAV9WEVGEMMVD9",
        identity: &identity,
        discovered_at: chrono::Utc::now(),
    };

    let good = candidate(serde_json::json!({
        "doc_id": "99990001",
        "row_ordinal": 1,
        "row_id": null,
        "asset_type_code": "ST",
        "amount_band_raw": "$1,001 - $15,000",
        "transaction_type_raw": "P",
        "partial_sale": false,
        "cap_gains_over_200": null,
        "filing_status_raw": "New",
        "owner_source": "default_self",
        "subholding_of": null,
        "vehicle_owner_code": null,
        "vehicle_location": null,
        "description": null,
        "comments": null,
        "signed_date": "2026-06-12"
    }));
    // Violates the (us_house, transaction) details contract (invariant 5).
    let bad = candidate(serde_json::json!({}));

    // good inserts first, then bad aborts: the SAME TXN must roll both back.
    let err = publish_filing(&pool, &spec, &[good.clone(), bad], &|_| Vec::new())
        .await
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("details"),
        "failure names the contract violation: {err:#}"
    );
    let (filings, gold, events): (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from filing), \
                (select count(*) from disclosure_record), \
                (select count(*) from outbox_event)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (filings, gold, events),
        (0, 0, 0),
        "atomic: no partial publish survives a rollback"
    );

    // The good candidate alone publishes: gold + outbox together.
    let stats = publish_filing(&pool, &spec, &[good], &|_| Vec::new())
        .await
        .unwrap();
    assert_eq!(stats.gold_inserted, 1);
    assert_eq!(stats.outbox_written, 1);
    let (filings, gold, events): (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from filing), \
                (select count(*) from disclosure_record), \
                (select count(*) from outbox_event)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((filings, gold, events), (1, 1, 1));
}

/// A minimal valid `us_house` transaction candidate; identity fields are
/// unbound (nil ULIDs) exactly as the adapter emits them in pool-backed mode —
/// `publish_filing` binds the real ids.
fn candidate(details: serde_json::Value) -> GoldCandidate {
    GoldCandidate {
        filing_id: "00000000000000000000000000".parse().unwrap(),
        politician_id: "00000000000000000000000000".parse().unwrap(),
        regime_id: "00000000000000000000000000".parse().unwrap(),
        instrument_id: None,
        asset_description_raw: "Test Asset (TST) [ST]".to_owned(),
        record_type: RecordType::Transaction,
        asset_class: AssetClass::Equity,
        side: Some(Side::Buy),
        transaction_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
        as_of_date: None,
        notified_date: Some(NaiveDate::from_ymd_opt(2026, 6, 2).unwrap()),
        value: Some(
            govfolio_core::domain::value::ValueInterval::new(
                rust_decimal::Decimal::new(100_100, 2),
                Some(rust_decimal::Decimal::new(1_500_000, 2)),
                govfolio_core::domain::enums::Currency::USD,
            )
            .unwrap(),
        ),
        owner: Some(govfolio_core::domain::enums::Owner::Self_),
        extraction_confidence: Some(0.98),
        extracted_by: "us_house_ptr/text@1".to_owned(),
        fingerprint: None,
        details,
    }
}
