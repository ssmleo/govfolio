//! DB-touching DDL suite for migration 0001 (design §4.2): gated behind `--ignored`
//! (CI db job / local postgres on 5433). Inserts both design `GoldCandidate` examples
//! via raw SQL and proves the per-type CHECK constraints reject bad rows with 23514.
#![allow(clippy::unwrap_used)]

// sqlx 0.9: dynamic SQL strings need the AssertSqlSafe opt-in (SqlSafeStr bound).
// Fine here: every fragment is a test constant, nothing user-supplied.
use sqlx::{AssertSqlSafe, PgPool};

/// FK parents both design examples need: jurisdiction, `disclosure_regime`, politician,
/// `raw_document`, filing. Ids reuse the domain `gold.rs` fixture ULIDs where they exist.
const SEED_PARENTS: &str = r"
insert into jurisdiction (id, name, iso_code, level) values
  ('01BX5ZZKBKACTAV9WEVGEMMVA1', 'United States', 'US', 'national'),
  ('01BX5ZZKBKACTAV9WEVGEMMVA2', 'United Kingdom', 'GB', 'national');

insert into disclosure_regime
  (id, jurisdiction_id, body, regime_type, value_precision, effective_from) values
  ('01BX5ZZKBKACTAV9WEVGEMMVS0', '01BX5ZZKBKACTAV9WEVGEMMVA1', 'US House',
   'transaction_report', 'banded', '2012-04-04'),
  ('01BX5ZZKBKACTAV9WEVGEMMVS3', '01BX5ZZKBKACTAV9WEVGEMMVA2', 'UK House of Commons',
   'change_notification', 'categorical', '2015-03-30');

insert into politician (id, canonical_name) values
  ('01BX5ZZKBKACTAV9WEVGEMMVRZ', 'Test Representative'),
  ('01BX5ZZKBKACTAV9WEVGEMMVS2', 'Test Member of Parliament');

insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) values
  ('01BX5ZZKBKACTAV9WEVGEMMVD1', 'gs://govfolio-bronze-test/us-ptr.pdf',
   '1111111111111111111111111111111111111111111111111111111111111111',
   'application/pdf', now()),
  ('01BX5ZZKBKACTAV9WEVGEMMVD2', 'gs://govfolio-bronze-test/uk-register.html',
   '2222222222222222222222222222222222222222222222222222222222222222',
   'text/html', now());

insert into filing
  (id, regime_id, politician_id, raw_document_id, filing_type, filed_date, discovered_at) values
  ('01ARZ3NDEKTSV4RRFFQ69G5FAV', '01BX5ZZKBKACTAV9WEVGEMMVS0', '01BX5ZZKBKACTAV9WEVGEMMVRZ',
   '01BX5ZZKBKACTAV9WEVGEMMVD1', 'ptr', '2026-03-10', now()),
  ('01BX5ZZKBKACTAV9WEVGEMMVS1', '01BX5ZZKBKACTAV9WEVGEMMVS3', '01BX5ZZKBKACTAV9WEVGEMMVS2',
   '01BX5ZZKBKACTAV9WEVGEMMVD2', 'register_update', '2026-04-10', now());
";

const DISCLOSURE_RECORD_COLUMNS: &str = "
insert into disclosure_record
  (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, asset_class,
   side, transaction_date, notified_date, value_low, value_high, currency, owner,
   extraction_confidence, extracted_by, fingerprint)
values
";

/// US House PTR example (design §4.2): transaction, buy, 2026-03-02, 1001–15000 USD, spouse.
const US_PTR_ROW: &str = "
  ('01BX5ZZKBKACTAV9WEVGEMMVR1', '01ARZ3NDEKTSV4RRFFQ69G5FAV', '01BX5ZZKBKACTAV9WEVGEMMVRZ',
   '01BX5ZZKBKACTAV9WEVGEMMVS0', 'Microsoft Corporation - Common Stock (MSFT)',
   'transaction', 'equity', 'buy', '2026-03-02', null, 1001.00, 15000.00, 'USD', 'spouse',
   0.99, 'fixture:test@0', 'fp-us-ptr-1')
";

/// UK register-of-interests example (design §4.2): interest, notified 2026-04-10,
/// 70000–open GBP (`value_high` NULL = open-ended).
const UK_INTEREST_ROW: &str = "
  ('01BX5ZZKBKACTAV9WEVGEMMVR2', '01BX5ZZKBKACTAV9WEVGEMMVS1', '01BX5ZZKBKACTAV9WEVGEMMVS2',
   '01BX5ZZKBKACTAV9WEVGEMMVS3', 'Shareholding: XYZ Holdings Ltd (above registrable threshold)',
   'interest', 'equity', null, null, '2026-04-10', 70000.00, null, 'GBP', null,
   0.99, 'fixture:test@0', 'fp-uk-interest-1')
";

async fn migrate_and_seed(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::raw_sql(SEED_PARENTS).execute(pool).await.unwrap();
}

fn assert_sqlstate_23514(err: sqlx::Error) {
    let sqlx::Error::Database(db_err) = err else {
        panic!("expected database error, got: {err}");
    };
    assert_eq!(db_err.code().as_deref(), Some("23514"), "{db_err}");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn both_design_gold_examples_insert_and_event_date_generates(pool: PgPool) {
    migrate_and_seed(&pool).await;
    let insert = format!("{DISCLOSURE_RECORD_COLUMNS} {US_PTR_ROW}, {UK_INTEREST_ROW}");
    sqlx::raw_sql(AssertSqlSafe(insert))
        .execute(&pool)
        .await
        .unwrap();

    // event_date is generated: coalesce(transaction_date, notified_date, as_of_date).
    let event_dates: Vec<String> =
        sqlx::query_scalar("select event_date::text from disclosure_record order by fingerprint")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(event_dates, ["2026-04-10", "2026-03-02"]); // fp-uk sorts before fp-us
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn transaction_missing_side_rejected_with_23514(pool: PgPool) {
    migrate_and_seed(&pool).await;
    let sideless = US_PTR_ROW.replace("'buy'", "null");
    let insert = format!("{DISCLOSURE_RECORD_COLUMNS} {sideless}");
    let err = sqlx::raw_sql(AssertSqlSafe(insert))
        .execute(&pool)
        .await
        .unwrap_err();
    assert_sqlstate_23514(err);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn inverted_value_band_rejected_with_23514(pool: PgPool) {
    migrate_and_seed(&pool).await;
    let inverted = US_PTR_ROW.replace("1001.00, 15000.00", "15000.00, 1001.00");
    let insert = format!("{DISCLOSURE_RECORD_COLUMNS} {inverted}");
    let err = sqlx::raw_sql(AssertSqlSafe(insert))
        .execute(&pool)
        .await
        .unwrap_err();
    assert_sqlstate_23514(err);
}
