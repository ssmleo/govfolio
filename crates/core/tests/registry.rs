//! DB-touching DDL suite for migration 0003 (design §5.8 coverage-factory registry
//! columns on `jurisdiction`): gated behind `--ignored` (CI db job / local postgres
//! on 5433). Proves the new columns exist, `coverage_phase` defaults to 'stub', and
//! the phase-vocabulary CHECK rejects out-of-vocabulary values with 23514.
#![allow(clippy::unwrap_used)]

use sqlx::PgPool;

async fn migrate_and_seed_one(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::raw_sql(
        "insert into jurisdiction (id, name, iso_code, level) values
           ('01BX5ZZKBKACTAV9WEVGEMMVA1', 'United States', 'US', 'national')",
    )
    .execute(pool)
    .await
    .unwrap();
}

fn assert_sqlstate_23514(err: sqlx::Error) {
    let sqlx::Error::Database(db_err) = err else {
        panic!("expected database error, got: {err}");
    };
    assert_eq!(db_err.code().as_deref(), Some("23514"), "{db_err}");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn coverage_columns_exist_and_phase_defaults_to_stub(pool: PgPool) {
    migrate_and_seed_one(&pool).await;
    // Every 0003 column is selectable; a bare insert lands in phase 'stub' with
    // everything else NULL (unclaimed, unscored, unblocked).
    let row: (
        Option<i16>,
        String,
        Option<f32>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select epoch, coverage_phase, priority_score, claimed_by, blocked_reason
             from jurisdiction
             where id = '01BX5ZZKBKACTAV9WEVGEMMVA1' and claimed_at is null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (None, "stub".to_owned(), None, None, None));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn every_design_phase_is_accepted(pool: PgPool) {
    migrate_and_seed_one(&pool).await;
    // Full §5.8 vocabulary: stub → scouted → surveyed → sampled → specced →
    // built → live | blocked.
    for phase in [
        "stub", "scouted", "surveyed", "sampled", "specced", "built", "live", "blocked",
    ] {
        sqlx::query("update jurisdiction set coverage_phase = $1")
            .bind(phase)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn out_of_vocabulary_phase_rejected_with_23514(pool: PgPool) {
    migrate_and_seed_one(&pool).await;
    // 'blocked:<reason>' is spelled as phase 'blocked' + blocked_reason column;
    // the composite literal must NOT enter the phase column.
    let err = sqlx::query("update jurisdiction set coverage_phase = 'blocked:no-source'")
        .execute(&pool)
        .await
        .unwrap_err();
    assert_sqlstate_23514(err);
}
