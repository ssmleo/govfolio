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

// ── goal 065: worldwide registry seed + coverage scorecard ──────────────────
// Non-DB acceptance (`cargo test -p core registry`) over the static seed data,
// plus DB acceptance (`--ignored`) over the seeded rows.

#[test]
fn registry_seed_lists_at_least_190_sovereigns() {
    // Acceptance: >= 190 jurisdictions (design §5.7: "all countries").
    assert!(
        govfolio_core::seed::COUNTRIES.len() >= 190,
        "seed lists {} sovereigns, need >= 190",
        govfolio_core::seed::COUNTRIES.len()
    );
}

#[test]
fn registry_lists_eight_built_regimes_each_with_a_source() {
    // The eight built launch regimes (design §5.7) are real, sourced regimes.
    let live = govfolio_core::seed::LIVE_REGIMES;
    assert_eq!(live.len(), 8);
    for regime in live {
        assert_ne!(regime.regime_type, "none", "{} is built", regime.code);
        assert!(
            !regime.source_url.is_empty(),
            "{} needs a source",
            regime.code
        );
    }
}

async fn migrate_and_seed_registry(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    govfolio_core::seed::seed_registry(pool).await.unwrap();
}

async fn registry_counts(pool: &PgPool) -> (i64, i64) {
    let jurisdictions = sqlx::query_scalar("select count(*) from jurisdiction")
        .fetch_one(pool)
        .await
        .unwrap();
    let regimes = sqlx::query_scalar("select count(*) from disclosure_regime")
        .fetch_one(pool)
        .await
        .unwrap();
    (jurisdictions, regimes)
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn registry_seeds_every_jurisdiction_with_a_regime(pool: PgPool) {
    migrate_and_seed_registry(&pool).await;

    let (jurisdictions, _regimes) = registry_counts(&pool).await;
    assert!(jurisdictions >= 190, "seeded {jurisdictions} jurisdictions");

    // Every jurisdiction has >= 1 disclosure_regime row (possibly type='none').
    let orphans: i64 = sqlx::query_scalar(
        "select count(*) from jurisdiction j where not exists \
           (select 1 from disclosure_regime r where r.jurisdiction_id = j.id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphans, 0, "every jurisdiction must carry a regime row");

    // The eight built regimes: non-'none' type WITH a source_url.
    let built: i64 = sqlx::query_scalar(
        "select count(*) from disclosure_regime \
           where regime_type <> 'none' and source_url is not null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(built, 8, "eight built launch regimes (design §5.7)");

    // coverage_phase: seven live jurisdictions (us appears once here — us_house
    // + us_senate share it), the rest stub. Matches the sentinel live_targets.
    let live: i64 =
        sqlx::query_scalar("select count(*) from jurisdiction where coverage_phase = 'live'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(live, 7, "seven live jurisdictions (us,gb,ca,au,eu,fr,de)");
    let stubs: i64 =
        sqlx::query_scalar("select count(*) from disclosure_regime where regime_type = 'none'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stubs,
        jurisdictions - 7,
        "one 'none' stub per non-live jurisdiction"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn registry_seed_is_idempotent(pool: PgPool) {
    migrate_and_seed_registry(&pool).await;
    let before = registry_counts(&pool).await;
    // Re-run: ON CONFLICT DO NOTHING inserts nothing (invariant 4).
    govfolio_core::seed::seed_registry(&pool).await.unwrap();
    let after = registry_counts(&pool).await;
    assert_eq!(before, after, "re-seeding must insert nothing");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn registry_built_regimes_carry_their_real_metadata(pool: PgPool) {
    migrate_and_seed_registry(&pool).await;
    for regime in govfolio_core::seed::LIVE_REGIMES {
        let row: (String, String, Option<String>) = sqlx::query_as(
            "select regime_type, value_precision, source_url \
               from disclosure_regime where id = $1",
        )
        .bind(regime.regime_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, regime.regime_type, "{}", regime.code);
        assert_eq!(row.1, regime.value_precision, "{}", regime.code);
        assert_eq!(row.2.as_deref(), Some(regime.source_url), "{}", regime.code);
    }
}
