//! Monthly sampling audit + precision report (goal 070, design §7.4). The pure
//! deterministic draw is unit-tested in `worker::sampler`; this DB-touching
//! suite (`--ignored`, postgres on `DATABASE_URL`) proves the end-to-end job:
//! a stratified-per-regime, seeded, idempotent draw into `sample_audit`, and a
//! precision report whose shape tracks the audited outcomes.
#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;

use sqlx::PgPool;

use worker::sampler::{precision_report, run_sampling_audit};

/// Two regimes (6 + 4 published `interest` records) so stratification and the
/// per-regime `min(per_regime, available)` cap are both exercised.
const SEED: &str = r"
insert into jurisdiction (id, name, iso_code, level) values
  ('jur-a', 'Country A', 'AA', 'national'),
  ('jur-b', 'Country B', 'BB', 'national');

insert into disclosure_regime
  (id, jurisdiction_id, body, regime_type, value_precision, effective_from) values
  ('regime-a', 'jur-a', 'Regime A', 'periodic_declaration', 'exact', '2020-01-01'),
  ('regime-b', 'jur-b', 'Regime B', 'periodic_declaration', 'exact', '2020-01-01');

insert into politician (id, canonical_name) values ('pol-1', 'Test Filer');

insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) values
  ('raw-1', 'file:///raw-1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
   'application/pdf', now());

insert into filing (id, regime_id, politician_id, raw_document_id, external_id, filing_type,
                    discovered_at) values
  ('filing-a', 'regime-a', 'pol-1', 'raw-1', 'ext-a', 'declaration', now()),
  ('filing-b', 'regime-b', 'pol-1', 'raw-1', 'ext-b', 'declaration', now());
";

/// Builds the `insert into disclosure_record ...` for `count` interest rows of
/// one regime — deterministic ids/fingerprints so the seed is reproducible.
fn records_sql(filing: &str, regime: &str, prefix: &str, count: usize) -> String {
    let mut sql = String::from(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
            asset_class, notified_date, extracted_by, fingerprint) values ",
    );
    for i in 0..count {
        if i > 0 {
            sql.push(',');
        }
        let _ = write!(
            sql,
            "('{prefix}-rec-{i:02}', '{filing}', 'pol-1', '{regime}', 'Asset {i}', 'interest', \
              'other', '2026-04-10', 'fixture:sampler@0', 'fp-{prefix}-{i:02}')"
        );
    }
    sql.push(';');
    sql
}

async fn seed(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::raw_sql(SEED).execute(pool).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(records_sql(
        "filing-a", "regime-a", "a", 6,
    )))
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(records_sql(
        "filing-b", "regime-b", "b", 4,
    )))
    .execute(pool)
    .await
    .unwrap();
}

async fn pending_ids(pool: &PgPool, month: &str, regime: &str) -> Vec<String> {
    sqlx::query_scalar(
        "select record_id from sample_audit \
         where sample_month = $1 and regime_id = $2 order by record_id",
    )
    .bind(month)
    .bind(regime)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn sampler_draws_deterministic_stratified_sample_and_reports_precision(pool: PgPool) {
    seed(&pool).await;
    let month = "2026-07";

    // Draw 2 records per regime, fixed seed.
    let report = run_sampling_audit(&pool, month, 2, 42).await.unwrap();

    // Stratified: exactly the two regimes, 2 records each, none audited yet.
    assert_eq!(report.sample_month, month);
    assert_eq!(report.regimes.len(), 2, "one stratum per regime");
    for regime in &report.regimes {
        assert_eq!(regime.sampled, 2, "per_regime cap honored");
        assert_eq!(regime.audited, 0);
        assert_eq!(regime.discrepancies, 0);
        assert_eq!(
            regime.precision_estimate, None,
            "no estimate before any record is audited"
        );
    }
    let total: i64 = sqlx::query_scalar("select count(*) from sample_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 4, "2 regimes x 2 = 4 queued rows");

    // Deterministic: capture the draw, re-run the SAME month+seed.
    let a_before = pending_ids(&pool, month, "regime-a").await;
    let b_before = pending_ids(&pool, month, "regime-b").await;
    let rerun = run_sampling_audit(&pool, month, 2, 42).await.unwrap();
    assert_eq!(rerun.regimes.len(), 2);
    let total_after: i64 = sqlx::query_scalar("select count(*) from sample_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_after, 4, "idempotent: re-run queues nothing new");
    assert_eq!(
        pending_ids(&pool, month, "regime-a").await,
        a_before,
        "same seed draws the same regime-a sample"
    );
    assert_eq!(pending_ids(&pool, month, "regime-b").await, b_before);

    // Audit one regime-a row as a discrepancy; the precision report tracks it.
    let discrepant = &a_before[0];
    sqlx::query(
        "update sample_audit set status = 'discrepancy', audited_at = now(), \
         discrepancy_note = 'value band one step low' \
         where sample_month = $1 and record_id = $2",
    )
    .bind(month)
    .bind(discrepant)
    .execute(&pool)
    .await
    .unwrap();

    let report = precision_report(&pool, month).await.unwrap();
    let regime_a = report
        .regimes
        .iter()
        .find(|r| r.regime_id == "regime-a")
        .unwrap();
    assert_eq!(regime_a.audited, 1);
    assert_eq!(regime_a.discrepancies, 1);
    assert_eq!(
        regime_a.precision_estimate,
        Some(0.0),
        "1 audited, 1 discrepancy -> precision 0.0"
    );
    let regime_b = report
        .regimes
        .iter()
        .find(|r| r.regime_id == "regime-b")
        .unwrap();
    assert_eq!(regime_b.body, "Regime B");
    assert_eq!(regime_b.audited, 0);
    assert_eq!(regime_b.precision_estimate, None);
}
