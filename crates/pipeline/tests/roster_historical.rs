//! Historical roster seeding across archive years (goal 081 Task 1): loops
//! the EXISTING, unchanged `roster_from_index_xml` + `seed_roster` over each
//! archive year via a fixture-backed `IndexXmlSource` (mirrors
//! `worker::backfill::dry_run`'s per-year fail-closed isolation —
//! `crates/worker/src/backfill.rs`'s `FakeArchive`/`dry_run` tests). DB-gated
//! like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
//!
//! Proves:
//! - a real, pre-2015, no-longer-in-Congress filer (Hon. John Boehner,
//!   OH-08, Speaker of the House 2011-2015, resigned from Congress
//!   2015-10-30) seeded from that year's archive index resolves via
//!   `resolve_politician`, not `None`;
//! - an ambiguous roster on one year (seed data corruption) is recorded in
//!   that year's `member_errors` WITHOUT sinking the other years in the same
//!   sweep;
//! - (goal 081 Task 5) an ambiguous member WITHIN a year does not sink the
//!   OTHER real members of that same year — only the ambiguous one is
//!   skipped/recorded, every other member in the batch still seeds.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::PgPool;

use pipeline::run::RegimeBinding;
use pipeline::stages::roster::resolve_politician;
use pipeline::stages::seed::seed_regime;
use us_house::seed::{IndexXmlSource, seed_historical_rosters};

/// Real member, pre-2015, no longer in Congress today: Ron Paul (R, TX-14),
/// did not seek reelection in 2012, left Congress January 2013.
const RON_PAUL_2012_SLICE: &str = "<FinancialDisclosure>\
    <Member><Prefix>Hon.</Prefix><Last>Paul</Last><First>Ron</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>TX14</StateDst><Year>2012</Year> \
      <FilingDate>5/15/2012</FilingDate><DocID>10000001</DocID></Member>\
    </FinancialDisclosure>";

/// Real member, pre-2015, no longer in Congress today: John Boehner (R,
/// OH-08), Speaker of the House 2011-2015, resigned from Congress
/// 2015-10-30.
const BOEHNER_2013_SLICE: &str = "<FinancialDisclosure>\
    <Member><Prefix>Hon.</Prefix><Last>Boehner</Last><First>John</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>OH08</StateDst><Year>2013</Year> \
      <FilingDate>5/15/2013</FilingDate><DocID>10000002</DocID></Member>\
    </FinancialDisclosure>";

/// Real member, pre-2015, no longer in Congress today: Eric Cantor (R,
/// VA-07), House Majority Leader, resigned from Congress 2014-08-18 after a
/// primary loss.
const CANTOR_2014_SLICE: &str = "<FinancialDisclosure>\
    <Member><Prefix>Hon.</Prefix><Last>Cantor</Last><First>Eric</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>VA07</StateDst><Year>2014</Year> \
      <FilingDate>5/15/2014</FilingDate><DocID>10000003</DocID></Member>\
    </FinancialDisclosure>";

/// FOUR real, distinct pre-2015 members filed in the SAME year (2013), used
/// by the per-member isolation test below (goal 081 Task 5). Boehner is
/// pre-corrupted ambiguous by `seed_duplicate_boehner`; Paul, Cantor, and
/// Bachmann have no matching seed data at all and must seed independently of
/// Boehner's failure. (Michele Bachmann, R MN-06, did not seek reelection in
/// 2014, left Congress January 2015 — active in 2013.)
const FOUR_MEMBER_2013_SLICE: &str = "<FinancialDisclosure>\
    <Member><Prefix>Hon.</Prefix><Last>Boehner</Last><First>John</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>OH08</StateDst><Year>2013</Year> \
      <FilingDate>5/15/2013</FilingDate><DocID>10000002</DocID></Member>\
    <Member><Prefix>Hon.</Prefix><Last>Paul</Last><First>Ron</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>TX14</StateDst><Year>2013</Year> \
      <FilingDate>5/16/2013</FilingDate><DocID>10000004</DocID></Member>\
    <Member><Prefix>Hon.</Prefix><Last>Cantor</Last><First>Eric</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>VA07</StateDst><Year>2013</Year> \
      <FilingDate>5/17/2013</FilingDate><DocID>10000005</DocID></Member>\
    <Member><Prefix>Hon.</Prefix><Last>Bachmann</Last><First>Michele</First><Suffix /> \
      <FilingType>P</FilingType><StateDst>MN06</StateDst><Year>2013</Year> \
      <FilingDate>5/18/2013</FilingDate><DocID>10000006</DocID></Member>\
    </FinancialDisclosure>";

/// An offline [`IndexXmlSource`] over fixed per-year XML — a year absent from
/// the map reports "unchanged" (`None`), mirroring `worker::backfill`'s
/// `FakeArchive` (`by_year` map; a year absent there fails closed instead —
/// here an absent year is simply a legitimate empty/no-op year, since a real
/// 304 is a valid outcome, not a failure).
struct FixtureIndexSource {
    by_year: BTreeMap<i32, String>,
}

#[async_trait]
impl IndexXmlSource for FixtureIndexSource {
    async fn fetch_year(&self, year: i32) -> anyhow::Result<Option<String>> {
        Ok(self.by_year.get(&year).cloned())
    }
}

async fn migrate_and_seed_regime(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
}

/// Pre-corrupts seed data: TWO politicians already match Boehner's as-filed
/// alias + district before the sweep runs, so `seed_roster` finds the roster
/// ambiguous for 2013 and bails (fail closed) — the setup for the
/// isolation test below.
async fn seed_duplicate_boehner(pool: &PgPool, regime: &RegimeBinding) {
    for _ in 0..2 {
        let politician_id = ulid::Ulid::new().to_string();
        sqlx::query("insert into politician (id, canonical_name) values ($1, $2)")
            .bind(&politician_id)
            .bind("Duplicate Boehner (seed corruption fixture)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("insert into politician_alias (politician_id, alias) values ($1, $2)")
            .bind(&politician_id)
            .bind("Hon. John Boehner")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "insert into mandate \
               (id, politician_id, jurisdiction_id, body, role, district, start_date) \
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(&politician_id)
        .bind(&regime.jurisdiction_id)
        .bind(&regime.body)
        .bind("Representative")
        .bind("OH08")
        .bind(NaiveDate::from_ymd_opt(2013, 1, 1).unwrap())
        .execute(pool)
        .await
        .unwrap();
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn pre_2015_filer_resolves_against_the_seeded_historical_roster(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let regime = us_house::seed::regime_binding();
    let source = FixtureIndexSource {
        by_year: [(2013, BOEHNER_2013_SLICE.to_owned())]
            .into_iter()
            .collect(),
    };

    let results = seed_historical_rosters(&source, &pool, &regime, 2012, 2014).await;
    assert_eq!(results.len(), 3, "the whole 2012..=2014 range was swept");
    let year_2013 = results.iter().find(|r| r.year == 2013).unwrap();
    assert!(
        year_2013.error.is_none(),
        "2013 seeds cleanly: {year_2013:?}"
    );
    assert_eq!(year_2013.inserted, 1, "Boehner seeded");
    // 2012 and 2014 had no archive entry in this fixture (a legitimate
    // no-op year, not a failure).
    for year in [2012, 2014] {
        let r = results.iter().find(|r| r.year == year).unwrap();
        assert!(r.error.is_none());
        assert_eq!(r.inserted, 0);
    }

    let politician_id = resolve_politician(&pool, &regime, "Hon. John Boehner", "OH08", None, None)
        .await
        .unwrap();
    assert!(
        politician_id.is_some(),
        "the pre-2015 filer resolves against the seeded historical roster, not None"
    );

    // Not a real filer, no district match today — never resolves (invariant
    // 3: no guessing).
    assert_eq!(
        resolve_politician(&pool, &regime, "Hon. Nobody Real", "ZZ99", None, None)
            .await
            .unwrap(),
        None
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn one_years_ambiguous_roster_bail_does_not_sink_the_others(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let regime = us_house::seed::regime_binding();
    seed_duplicate_boehner(&pool, &regime).await;

    let source = FixtureIndexSource {
        by_year: [
            (2012, RON_PAUL_2012_SLICE.to_owned()),
            (2013, BOEHNER_2013_SLICE.to_owned()),
            (2014, CANTOR_2014_SLICE.to_owned()),
        ]
        .into_iter()
        .collect(),
    };

    let results = seed_historical_rosters(&source, &pool, &regime, 2012, 2014).await;
    let by_year: BTreeMap<i32, &us_house::seed::YearSeedResult> =
        results.iter().map(|r| (r.year, r)).collect();

    // 2013's index itself fetched/parsed fine — only Boehner, the one
    // ambiguous member, is recorded (per-member isolation, goal 081 Task 5);
    // the YEAR is not marked failed.
    assert!(
        by_year[&2013].error.is_none(),
        "2013's index itself is fine — only the one ambiguous member is recorded"
    );
    assert_eq!(
        by_year[&2013].inserted, 0,
        "the only member in 2013 was ambiguous"
    );
    assert_eq!(
        by_year[&2013].member_errors.len(),
        1,
        "Boehner is recorded as an ambiguous member, not a whole-year failure"
    );
    assert_eq!(
        by_year[&2013].member_errors[0].filed_alias,
        "Hon. John Boehner"
    );
    assert_eq!(by_year[&2013].member_errors[0].district, "OH08");

    // 2012 and 2014 still seed cleanly — one bad year does not sink the rest.
    assert!(by_year[&2012].error.is_none());
    assert_eq!(by_year[&2012].inserted, 1, "Ron Paul seeded");
    assert!(by_year[&2014].error.is_none());
    assert_eq!(by_year[&2014].inserted, 1, "Eric Cantor seeded");

    assert!(
        resolve_politician(&pool, &regime, "Hon. Ron Paul", "TX14", None, None)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        resolve_politician(&pool, &regime, "Hon. Eric Cantor", "VA07", None, None)
            .await
            .unwrap()
            .is_some()
    );
}

/// Goal 081 Task 5: the fix under test. Before it, ONE ambiguous member
/// anywhere in a year's roster batch sank `seed_roster`'s whole transaction,
/// so every other real member in that same year was lost too, not just the
/// ambiguous one. This proves that no longer holds: a 4-member batch in the
/// SAME year, one member (Boehner) genuinely ambiguous, the other three
/// (Paul, Cantor, Bachmann) untouched seed data — all three must still seed.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn one_members_ambiguous_match_does_not_sink_the_rest_of_the_same_years_batch(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let regime = us_house::seed::regime_binding();
    // Pre-corrupt ONLY Boehner's alias/district; Paul/Cantor/Bachmann have no
    // matching seed data at all.
    seed_duplicate_boehner(&pool, &regime).await;

    let source = FixtureIndexSource {
        by_year: [(2013, FOUR_MEMBER_2013_SLICE.to_owned())]
            .into_iter()
            .collect(),
    };

    let results = seed_historical_rosters(&source, &pool, &regime, 2013, 2013).await;
    assert_eq!(results.len(), 1);
    let year_2013 = &results[0];

    // The YEAR itself is not marked failed — its index fetched/parsed fine.
    assert!(
        year_2013.error.is_none(),
        "one ambiguous member must not fail the whole year: {year_2013:?}"
    );
    // Exactly the one ambiguous member is recorded...
    assert_eq!(
        year_2013.member_errors.len(),
        1,
        "only Boehner is ambiguous: {:?}",
        year_2013.member_errors
    );
    assert_eq!(year_2013.member_errors[0].filed_alias, "Hon. John Boehner");
    assert_eq!(year_2013.member_errors[0].district, "OH08");
    // ...and the OTHER THREE members in the SAME batch still seeded — this
    // is exactly what did NOT hold before the fix (the whole batch would
    // have bailed on Boehner's ambiguity and inserted 0).
    assert_eq!(
        year_2013.inserted, 3,
        "Paul, Cantor, and Bachmann all seed despite Boehner's ambiguity in the same batch"
    );

    for (alias, district) in [
        ("Hon. Ron Paul", "TX14"),
        ("Hon. Eric Cantor", "VA07"),
        ("Hon. Michele Bachmann", "MN06"),
    ] {
        assert!(
            resolve_politician(&pool, &regime, alias, district, None, None)
                .await
                .unwrap()
                .is_some(),
            "{alias} ({district}) seeded and resolves despite Boehner's ambiguity in the same batch"
        );
    }

    // Boehner himself still does not resolve — the pre-existing ambiguity is
    // real and unchanged (never guess, invariant 3).
    assert_eq!(
        resolve_politician(&pool, &regime, "Hon. John Boehner", "OH08", None, None)
            .await
            .unwrap(),
        None
    );
}
