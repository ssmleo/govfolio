//! Regression coverage for the cross-time/cross-body politician-identity fix
//! (`docs/decisions/politician-identity-resolution-design.md`), grounded in
//! the two confirmed live `br` collisions: JULIO CESAR DOS SANTOS (same-pass)
//! and CARLOS ALBERTO DE SOUZA (cross-pass, 8-year gap). DB-gated like
//! `roster_historical.rs`: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use sqlx::PgPool;

use pipeline::run::RegimeBinding;
use pipeline::stages::roster::{RosterMember, resolve_politician, seed_roster};
use pipeline::stages::seed::seed_regime;

async fn migrate_and_seed_regime(pool: &PgPool) -> RegimeBinding {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
    us_house::seed::regime_binding()
}

fn member(
    alias: &str,
    district: &str,
    year: i32,
    external_identifier: Option<&str>,
) -> RosterMember {
    RosterMember {
        canonical_name: alias.to_owned(),
        filed_alias: alias.to_owned(),
        district: district.to_owned(),
        role: "Deputado Federal".to_owned(),
        active_year: year,
        external_identifier: external_identifier.map(str::to_owned),
    }
}

/// JULIO CESAR DOS SANTOS shape: two different real people, same alias/
/// district/body, discovered in the SAME `seed_roster` call (same batch,
/// same transaction) — must seed as two distinct politicians, each
/// resolvable by their own id, not collapse onto one row.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn same_pass_two_candidates_with_different_ids_seed_distinctly(pool: PgPool) {
    let regime = migrate_and_seed_regime(&pool).await;
    let members = vec![
        member("JULIO CESAR DOS SANTOS", "BA", 2018, Some("80673872653")),
        member("JULIO CESAR DOS SANTOS", "BA", 2018, Some("67701124500")),
    ];
    let inserted = seed_roster(&pool, &regime, &members).await.unwrap();
    assert_eq!(
        inserted, 2,
        "two distinct real people must seed as two politicians"
    );

    let first = resolve_politician(
        &pool,
        &regime,
        "JULIO CESAR DOS SANTOS",
        "BA",
        Some("80673872653"),
        Some(2018),
    )
    .await
    .unwrap();
    let second = resolve_politician(
        &pool,
        &regime,
        "JULIO CESAR DOS SANTOS",
        "BA",
        Some("67701124500"),
        Some(2018),
    )
    .await
    .unwrap();
    assert!(first.is_some() && second.is_some());
    assert_ne!(
        first, second,
        "each real person resolves to their own politician"
    );
}

/// CARLOS ALBERTO DE SOUZA shape: the bug this goal fixes. Two different
/// real people, same alias/district/body, discovered in TWO SEPARATE
/// `seed_roster` calls 8 years apart (the actual real-world gap) — before
/// the fix, the second call's `resolve_hits` found "already seeded" and
/// silently reused the first person's politician row. Must now mint a new
/// politician for the second person instead.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn cross_pass_two_candidates_with_different_ids_seed_distinctly(pool: PgPool) {
    let regime = migrate_and_seed_regime(&pool).await;

    let inserted_2014 = seed_roster(
        &pool,
        &regime,
        &[member(
            "CARLOS ALBERTO DE SOUZA",
            "SP",
            2014,
            Some("29168317972"),
        )],
    )
    .await
    .unwrap();
    assert_eq!(inserted_2014, 1);

    // A SEPARATE call, mirroring a later year's real backfill re-running
    // seed_candidates_year independently.
    let inserted_2022 = seed_roster(
        &pool,
        &regime,
        &[member(
            "CARLOS ALBERTO DE SOUZA",
            "SP",
            2022,
            Some("09867774809"),
        )],
    )
    .await
    .unwrap();
    assert_eq!(
        inserted_2022, 1,
        "the 2022 candidate is a DIFFERENT real person (different CPF) — must mint a new \
         politician, not silently reuse the 2014 candidate's row"
    );

    let p2014 = resolve_politician(
        &pool,
        &regime,
        "CARLOS ALBERTO DE SOUZA",
        "SP",
        Some("29168317972"),
        Some(2014),
    )
    .await
    .unwrap();
    let p2022 = resolve_politician(
        &pool,
        &regime,
        "CARLOS ALBERTO DE SOUZA",
        "SP",
        Some("09867774809"),
        Some(2022),
    )
    .await
    .unwrap();
    assert!(p2014.is_some() && p2022.is_some());
    assert_ne!(
        p2014, p2022,
        "the two real people resolve to two different politicians"
    );
}

/// Backward compatibility: a politician seeded before this mechanism existed
/// (`external_identifier = NULL`, exactly every pre-fix `br` row) still
/// resolves for a plain follow-up filing that also carries no id — nothing
/// already correctly resolved regresses.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn legacy_row_with_no_stored_id_still_resolves(pool: PgPool) {
    let regime = migrate_and_seed_regime(&pool).await;
    let inserted = seed_roster(&pool, &regime, &[member("LEGACY FILER", "RJ", 2018, None)])
        .await
        .unwrap();
    assert_eq!(inserted, 1);

    let resolved = resolve_politician(&pool, &regime, "LEGACY FILER", "RJ", None, Some(2022))
        .await
        .unwrap();
    assert!(
        resolved.is_some(),
        "a legacy (no-id) politician must still resolve for a plausible-gap follow-up filing"
    );
}

/// An implausible year gap with no id on either side fails closed
/// (`unresolved_filer`, invariant 3) rather than silently merging.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn implausible_year_gap_with_no_id_fails_closed(pool: PgPool) {
    let regime = migrate_and_seed_regime(&pool).await;
    seed_roster(&pool, &regime, &[member("ANCIENT FILER", "TX", 1950, None)])
        .await
        .unwrap();

    let resolved = resolve_politician(&pool, &regime, "ANCIENT FILER", "TX", None, Some(2022))
        .await
        .unwrap();
    assert_eq!(
        resolved, None,
        "a 72-year gap with no id on either side must fail closed, not silently merge"
    );
}
