//! Backfill-mode alert suppression (goal 081 Task 2): `FilingSpec::backfill`
//! threaded into `insert_outbox` (`crates/pipeline/src/stages/publish.rs`).
//! DB-gated like the other sqlx suites: `--ignored` + postgres on
//! `DATABASE_URL`.
//!
//! Proves, against the real `us_house` evidence fixture + a contract-valid
//! Gold candidate (the same shapes `e2e_local.rs` already exercises):
//! - a backfill-mode `publish_filing` still writes Gold rows normally
//!   (`gold_inserted > 0` — the historical facts are real, invariant 1/2);
//! - its `outbox_event` is written already `dispatched_at`-stamped, so a
//!   subsequent `match_pass` sees zero undispatched events (`matched.events
//!   == 0`) — no real subscriber alert ever fires for a backfilled filing.
#![allow(clippy::unwrap_used)]

use chrono::NaiveDate;
use sqlx::PgPool;

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::conformance::workspace_root;
use pipeline::run::FilingIdentity;
use pipeline::stages::publish::{FilingSpec, publish_filing};
use pipeline::stages::roster::{resolve_politician, seed_roster};
use pipeline::stages::seed::seed_regime;
use worker::alerts::DispatchConfig;
use worker::alerts::matcher::match_pass;

fn evidence_index_xml() -> String {
    let path = workspace_root()
        .join("docs")
        .join("regimes")
        .join("us-house")
        .join("evidence")
        .join("94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e.2026FD-slice.xml");
    std::fs::read_to_string(path).unwrap()
}

async fn migrate_and_seed_regime(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
}

/// A minimal, contract-valid `us_house` transaction candidate (mirrors
/// `e2e_local.rs`'s `candidate()` helper); identity fields are unbound (nil
/// ULIDs) exactly as the adapter emits them — `publish_filing` binds the
/// real ids.
fn candidate() -> GoldCandidate {
    GoldCandidate {
        filing_id: "00000000000000000000000000".parse().unwrap(),
        politician_id: "00000000000000000000000000".parse().unwrap(),
        regime_id: "00000000000000000000000000".parse().unwrap(),
        instrument_id: None,
        asset_description_raw: "Test Asset (TST) [ST]".to_owned(),
        record_type: RecordType::Transaction,
        asset_class: AssetClass::Equity,
        side: Some(Side::Buy),
        transaction_date: Some(NaiveDate::from_ymd_opt(2013, 6, 1).unwrap()),
        as_of_date: None,
        notified_date: Some(NaiveDate::from_ymd_opt(2013, 6, 2).unwrap()),
        value: Some(
            ValueInterval::new(
                rust_decimal::Decimal::new(100_100, 2),
                Some(rust_decimal::Decimal::new(1_500_000, 2)),
                Currency::USD,
            )
            .unwrap(),
        ),
        owner: Some(Owner::Self_),
        extraction_confidence: Some(0.98),
        extracted_by: "us_house_ptr/text@1".to_owned(),
        fingerprint: None,
        details: serde_json::json!({
            "doc_id": "88880001",
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
            "signed_date": "2013-06-01"
        }),
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn backfill_mode_publishes_gold_but_suppresses_the_outbox_dispatch(pool: PgPool) {
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
         values ('01BX5ZZKBKACTAV9WEVGEMMVD9', 'file:///tmp/backfill.pdf', \
                 '4444444444444444444444444444444444444444444444444444444444444444', \
                 'application/pdf', now())",
    )
    .execute(&pool)
    .await
    .unwrap();

    let identity = FilingIdentity {
        external_id: "88880001".to_owned(),
        filer_name: "Hon. Nicholas Begich III".to_owned(),
        district: "AK00".to_owned(),
        filing_type: "P".to_owned(),
        filed_date: Some(NaiveDate::from_ymd_opt(2013, 6, 12).unwrap()),
    };
    let spec = FilingSpec {
        regime_id: &regime.regime_id,
        regime_code: "us_house",
        politician_id: &politician_id,
        raw_document_id: "01BX5ZZKBKACTAV9WEVGEMMVD9",
        identity: &identity,
        discovered_at: chrono::Utc::now(),
        backfill: true,
    };

    let stats = publish_filing(&pool, &spec, &[candidate()], &|_| Vec::new())
        .await
        .unwrap();
    assert!(
        stats.gold_inserted > 0,
        "backfill mode still writes real Gold rows"
    );

    // Gold + review_tasks are unaffected: nothing about backfill mode changes
    // what got published, only whether it dispatches.
    let gold_count: i64 = sqlx::query_scalar("select count(*) from disclosure_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gold_count, i64::try_from(stats.gold_inserted).unwrap());

    // The outbox row was written already dispatched, not left NULL.
    let dispatched: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select dispatched_at from outbox_event")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        dispatched.is_some(),
        "backfill mode binds dispatched_at = now() in the same INSERT"
    );

    // A subsequent matcher pass finds nothing undispatched: zero real
    // subscriber alerts for a historical backfill.
    let matched = match_pass(&pool, &DispatchConfig::default()).await.unwrap();
    assert_eq!(
        matched.events, 0,
        "backfill-suppressed outbox events are never picked up by the matcher"
    );
    assert_eq!(matched.deliveries, 0);
}
