//! Two-stage publication smoke (plan Task 11): resolving a `review_task`
//! through `pipeline::promote` — the supersede-never-update invariant
//! (invariant 1) locked behind tests before any reviewer UI exists.
//! DB-gated like the other sqlx suites: `--ignored` + postgres on
//! `DATABASE_URL`.
//!
//! Proves, against Gold rows seeded by the REAL T9 pipeline over the five
//! `us_house` fixtures:
//! - `confirm` flips the target record `unverified → 'verified'` — the ONE
//!   sanctioned state transition — and touches nothing else on the row;
//! - `edit` INSERTS a superseding record (`'corrected'`,
//!   `supersedes_record_id` set, fresh fingerprint) plus one outbox event in
//!   the SAME transaction, while the original row stays byte-identical on
//!   every column (`d::text` before/after compare — facts are never
//!   `UPDATE`d);
//! - a contract-violating edit rolls back whole: no superseding row, no
//!   outbox event, task still open (atomicity);
//! - `reject` marks the record `'disputed'` and touches nothing else;
//! - a second resolution attempt of the same task is an idempotent no-op.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use chrono::NaiveDate;
use sqlx::{AssertSqlSafe, PgPool};

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::promote::{ResolveOutcome, Verdict, resolve_review_task};
use pipeline::run::{LocalFiling, Runner};
use pipeline::stages::roster::seed_roster;
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;

/// Tables a resolution could possibly write — idempotency snapshots sweep them.
const WRITE_TABLES: &[&str] = &[
    "disclosure_record",
    "outbox_event",
    "review_task",
    "filing",
    "raw_document",
];

async fn table_counts(pool: &PgPool) -> Vec<(String, i64)> {
    let mut counts = Vec::with_capacity(WRITE_TABLES.len());
    for table in WRITE_TABLES {
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

/// The full offline roster: the four E1 index-slice members plus the paper
/// filer of the goal-021 scanned fixture (its own archived index slice).
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
    std::env::temp_dir().join(format!(
        "govfolio-promote-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

/// Seeds regime + roster and runs the REAL T9 pipeline over the five
/// fixtures, then returns the one open review task the run files
/// (`ptr_amendment_unlinked`) as `(task_id, target_record_id)`.
async fn seed_via_pipeline(pool: &PgPool, tag: &str) -> (String, String) {
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
        BronzeStore::open(temp_bronze_root(tag)).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx).unwrap();
    let report = runner.run_local(&fixture_inputs()).await.unwrap();
    assert_eq!(report.failed, Vec::<String>::new());
    assert_eq!(report.gold_inserted, 13, "1+8+2+1+1 fixture rows");

    let (task_id, record_id): (String, String) = sqlx::query_as(
        "select id, target_id from review_task \
         where reason = 'ptr_amendment_unlinked' and status = 'open'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let state: String =
        sqlx::query_scalar("select verification_state from disclosure_record where id = $1")
            .bind(&record_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state, "unverified", "pipeline publishes unverified");
    (task_id, record_id)
}

/// The full row rendered by Postgres itself — every column, byte-identical or
/// not. THE probe for "facts are never `UPDATE`d".
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

/// The reviewer's corrected facts for the amended Boeing row: the amount band
/// was mis-extracted one band too low; everything else re-attested as filed.
/// Identity fields are unbound (nil ULIDs) — `promote` pins them from the
/// original row, exactly like `publish` binds candidates.
fn corrected_boeing(details: serde_json::Value) -> GoldCandidate {
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
        extracted_by: "review:promote_test@1".to_owned(),
        fingerprint: None,
        details,
    }
}

/// Contract-valid corrected `details` (`us_house.transaction` schema) with the
/// band string fixed to match the corrected value interval.
fn corrected_details() -> serde_json::Value {
    serde_json::json!({
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
    })
}

fn edit_verdict(details: serde_json::Value) -> Verdict {
    Verdict::Edit {
        regime_code: "us_house".to_owned(),
        corrected: Box::new(corrected_boeing(details)),
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn confirm_flips_unverified_to_verified_and_is_idempotent(pool: PgPool) {
    let (task_id, record_id) = seed_via_pipeline(&pool, "confirm").await;
    let before_rest = row_except_state(&pool, &record_id).await;

    // A task id that does not exist fails closed.
    let err = resolve_review_task(&pool, "01NOPE00000000000000000000", Verdict::Confirm, None)
        .await
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("01NOPE00000000000000000000"),
        "missing task is an error naming the task: {err:#}"
    );

    let outcome = resolve_review_task(&pool, &task_id, Verdict::Confirm, None)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        ResolveOutcome::Applied {
            record_id: record_id.clone(),
            superseding_record_id: None,
        }
    );

    // The sanctioned transition — and ONLY it: every other column untouched,
    // every other record still unverified.
    assert_eq!(record_state(&pool, &record_id).await, "verified");
    assert_eq!(row_except_state(&pool, &record_id).await, before_rest);
    let unverified: i64 = sqlx::query_scalar(
        "select count(*) from disclosure_record where verification_state = 'unverified'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unverified, 12, "the other 12 records stay unverified");

    // The task is resolved with an audit payload.
    let (status, verdict, has_resolved_at): (String, String, bool) = sqlx::query_as(
        "select status, resolution->>'verdict', resolved_at is not null \
         from review_task where id = $1",
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), verdict.as_str()), ("resolved", "confirm"));
    assert!(has_resolved_at);

    // Second resolution attempt — same verdict OR a different one — is a no-op.
    let counts = table_counts(&pool).await;
    let again = resolve_review_task(&pool, &task_id, Verdict::Confirm, None)
        .await
        .unwrap();
    assert_eq!(again, ResolveOutcome::AlreadyResolved);
    let conflicting = resolve_review_task(&pool, &task_id, Verdict::Reject, None)
        .await
        .unwrap();
    assert_eq!(conflicting, ResolveOutcome::AlreadyResolved);
    assert_eq!(table_counts(&pool).await, counts);
    assert_eq!(record_state(&pool, &record_id).await, "verified");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn edit_supersedes_and_never_updates_the_original(pool: PgPool) {
    let (task_id, record_id) = seed_via_pipeline(&pool, "edit").await;
    let (original_fp, original_filing, original_politician, original_regime): (
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "select fingerprint, filing_id, politician_id, regime_id \
         from disclosure_record where id = $1",
    )
    .bind(&record_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // BEFORE snapshot: the whole original row as Postgres renders it.
    let before = row_text(&pool, &record_id).await;

    // A correction that violates the details contract rolls back WHOLE:
    // no superseding row, no outbox event, task still open (invariant 5 at
    // promotion + same-txn atomicity).
    let err = resolve_review_task(&pool, &task_id, edit_verdict(serde_json::json!({})), None)
        .await
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("details"),
        "failure names the contract violation: {err:#}"
    );
    let (corrected_rows, corrected_events, task_status): (i64, i64, String) = sqlx::query_as(
        "select (select count(*) from disclosure_record where verification_state = 'corrected'), \
                (select count(*) from outbox_event where kind = 'disclosure_record.corrected'), \
                (select status from review_task where id = $1)",
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((corrected_rows, corrected_events), (0, 0));
    assert_eq!(task_status, "open");
    assert_eq!(row_text(&pool, &record_id).await, before);

    // The valid correction: superseding row + outbox event, one transaction.
    let outcome = resolve_review_task(&pool, &task_id, edit_verdict(corrected_details()), None)
        .await
        .unwrap();
    let ResolveOutcome::Applied {
        record_id: applied_record,
        superseding_record_id: Some(superseding_id),
    } = outcome
    else {
        panic!("edit must apply with a superseding record id: {outcome:?}");
    };
    assert_eq!(applied_record, record_id);

    // AFTER: the original row is byte-identical on EVERY column — the
    // supersede-never-update invariant (invariant 1).
    assert_eq!(
        row_text(&pool, &record_id).await,
        before,
        "the original row must never be UPDATEd by a correction"
    );

    // The superseding row carries the corrected facts.
    let (state, supersedes, fp, value_high, band, filing, politician, regime): (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "select verification_state, supersedes_record_id, fingerprint, value_high::text, \
                details->>'amount_band_raw', filing_id, politician_id, regime_id \
         from disclosure_record where id = $1",
    )
    .bind(&superseding_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "corrected");
    assert_eq!(supersedes, record_id, "supersedes_record_id → the original");
    assert_eq!(value_high, "50000.00");
    assert_eq!(band, "$15,001 - $50,000");
    assert!(
        fp.len() == 64 && fp.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')),
        "superseding fingerprint is 64 lowercase hex: {fp}"
    );
    assert_ne!(fp, original_fp, "the correction gets a NEW fingerprint");
    assert_eq!(
        (filing, politician, regime),
        (original_filing, original_politician, original_regime),
        "identity is pinned from the original, never reviewer-supplied"
    );

    // Exactly one outbox event for the supersession, linking both rows.
    let events: Vec<(String, String)> = sqlx::query_as(
        "select payload->>'record_id', payload->>'superseded_record_id' \
         from outbox_event where kind = 'disclosure_record.corrected'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(events, [(superseding_id.clone(), record_id.clone())]);

    // 13 pipeline rows + 1 superseding row; task resolved with the audit trail.
    let gold: i64 = sqlx::query_scalar("select count(*) from disclosure_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gold, 14);
    let (status, verdict, recorded_superseding): (String, String, String) = sqlx::query_as(
        "select status, resolution->>'verdict', resolution->>'superseding_record_id' \
         from review_task where id = $1",
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), verdict.as_str()), ("resolved", "edit"));
    assert_eq!(recorded_superseding, superseding_id);

    // Second resolution attempt of the same task: idempotent no-op.
    let counts = table_counts(&pool).await;
    let again = resolve_review_task(&pool, &task_id, edit_verdict(corrected_details()), None)
        .await
        .unwrap();
    assert_eq!(again, ResolveOutcome::AlreadyResolved);
    assert_eq!(table_counts(&pool).await, counts);
    assert_eq!(row_text(&pool, &record_id).await, before);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn reject_marks_the_record_disputed_and_is_idempotent(pool: PgPool) {
    let (task_id, record_id) = seed_via_pipeline(&pool, "reject").await;
    let before_rest = row_except_state(&pool, &record_id).await;

    let outcome = resolve_review_task(&pool, &task_id, Verdict::Reject, None)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        ResolveOutcome::Applied {
            record_id: record_id.clone(),
            superseding_record_id: None,
        }
    );

    // Reviewer looked and could NOT confirm: 'disputed' (design §7.2 reject
    // routing; 'unverified' would claim nobody adjudicated it).
    assert_eq!(record_state(&pool, &record_id).await, "disputed");
    assert_eq!(row_except_state(&pool, &record_id).await, before_rest);

    let (status, verdict): (String, String) =
        sqlx::query_as("select status, resolution->>'verdict' from review_task where id = $1")
            .bind(&task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), verdict.as_str()), ("resolved", "reject"));

    let counts = table_counts(&pool).await;
    let again = resolve_review_task(&pool, &task_id, Verdict::Reject, None)
        .await
        .unwrap();
    assert_eq!(again, ResolveOutcome::AlreadyResolved);
    assert_eq!(table_counts(&pool).await, counts);
    assert_eq!(record_state(&pool, &record_id).await, "disputed");
}
