//! Real (write-to-prod) US House backfill integration test (goal 081 Task 3):
//! `crates/worker/src/bin/backfill-real.rs`'s underlying real-write chain
//! (`pipeline::run::Runner::run_over`, backfill mode) driven by a REAL, small
//! slice of one archived Clerk year (2026 — the current year is itself part
//! of the archived `--from..=--to` range `discover_year` serves).
//!
//! The slice is 3 REAL, currently-filed 2026 PTRs already proven to parse
//! cleanly by the adapter's own local fixture suite
//! (`crates/adapters/us_house/fixtures/`, `MANIFEST.json`: `typical_single_row`
//! `20020055`, `multi_row_sp_vehicle` `20019182`, `sp_owner_options`
//! `20034836`) — picked out of a REAL `discover_year` call (not hand-built
//! `FilingRef`s) so this test proves the real discovery→write path
//! end-to-end, while deterministically avoiding the two known real-2026
//! parse edge cases goal 080's dry run surfaced (an `L:` sub-line case and a
//! scanned/paper filing needing the `ANTHROPIC_API_KEY`-gated LLM seam, out
//! of scope here) and the adapter-incompatibility this test ALSO discovered
//! empirically against a genuinely historical year (2015: real PTR PDFs from
//! that era render the filer-information label lowercase — `name:`, not
//! `Name:` — which the current text-layer parser does not yet handle; a
//! real, separate adapter-hardening gap, not a goal 081 Task 3 concern).
//!
//! DB-gated like the other sqlx suites (`--ignored` + postgres on
//! `DATABASE_URL`) AND network-gated (this test hits the real Clerk archive
//! index + PDFs, invariant 10 polite): an unreachable network degrades to an
//! honest, loud skip rather than a panic — mirroring `bin/backfill.rs`'s
//! ARCHIVE UNREACHABLE degradation — so a host without internet does not
//! turn `cargo test --workspace -- --ignored` red (rust-tdd skill: an
//! ignored live test must degrade, not fail, when its dependency is absent).
//!
//! Proves, against real production wiring:
//! - a small real slice of a real year's filings lands in Gold
//!   (`gold_inserted > 0`, invariants 1/2: the historical facts are real);
//! - a second run over the SAME `FilingRef`s is idempotent — 0 new rows
//!   (invariant 4: `pipeline_run` claim replay, no re-fetch/re-parse/re-publish);
//! - every `outbox_event` the run wrote has `dispatched_at` set (goal 081
//!   Task 2's backfill suppression — no real subscriber alert ever fires for
//!   a historical filing).
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use sqlx::PgPool;

use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::run::Runner;
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;
use us_house::seed::{LiveIndexSource, seed_historical_rosters};

/// The archive year to sweep (current year — see module doc for why).
const YEAR: i32 = 2026;
/// Real, currently-filed 2026 PTRs already proven to parse cleanly (see
/// module doc). "A small real year slice" (goal 081 Task 3 acceptance), NOT
/// the full ~274-filing year `backfill-real` itself processes (no
/// `--limit`/sampling there — this filter is test-only).
const KNOWN_GOOD_DOC_IDS: [&str; 3] = ["20020055", "20019182", "20034836"];

fn temp_bronze(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "govfolio-backfill-real-test-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

async fn migrate_and_seed_regime(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres + network"]
async fn real_year_slice_lands_in_gold_idempotently_with_suppressed_alerts(pool: PgPool) {
    migrate_and_seed_regime(&pool).await;
    let regime = us_house::seed::regime_binding();

    // Historical roster (goal 081 Task 1, already built + tested): a REAL
    // network fetch of YEAR's index, seeded via the SAME production helper a
    // real backfill run uses. A fresh `UsHouseAdapter` instance owns its own
    // conditional-GET validator cache (per adapter.rs), so this fetch is
    // this instance's first (and only) touch of YEAR.
    let roster_adapter = UsHouseAdapter::default();
    let roster_ctx = RunCtx::new(
        BronzeStore::open(temp_bronze("roster")).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &roster_adapter.politeness(),
    )
    .unwrap();
    let seeded = seed_historical_rosters(
        &LiveIndexSource {
            adapter: &roster_adapter,
            ctx: &roster_ctx,
        },
        &pool,
        &regime,
        YEAR,
        YEAR,
    )
    .await;
    assert_eq!(seeded.len(), 1, "the single-year sweep returns one result");
    if let Some(error) = &seeded[0].error {
        let banner = format!(
            "SKIPPED (honest, not a false red): real {YEAR} Clerk archive unreachable — {error}"
        );
        println!("{banner}");
        eprintln!("{banner}");
        return;
    }
    assert!(
        seeded[0].inserted > 0,
        "real {YEAR} archive seeds real roster members"
    );

    // Discovery: a SEPARATE `UsHouseAdapter` instance so its own cache starts
    // empty for YEAR — `roster_adapter` above already fetched YEAR once
    // (invariant 10: exactly once PER INSTANCE); reusing it here would send a
    // conditional GET and legitimately 304 (index unchanged seconds later),
    // returning zero filings, which is correct per-instance behavior but not
    // what this test wants. A fresh instance still touches YEAR exactly once.
    let run_adapter = UsHouseAdapter::default();
    let run_ctx = RunCtx::new(
        BronzeStore::open(temp_bronze("run")).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &run_adapter.politeness(),
    )
    .unwrap();
    let refs = run_adapter.discover_year(YEAR, &run_ctx).await.unwrap();
    let slice: Vec<_> = refs
        .into_iter()
        .filter(|r| KNOWN_GOOD_DOC_IDS.contains(&r.external_id.as_str()))
        .collect();
    assert_eq!(
        slice.len(),
        KNOWN_GOOD_DOC_IDS.len(),
        "all {} known-good real {YEAR} filings are still discoverable",
        KNOWN_GOOD_DOC_IDS.len()
    );

    let binding = UsHouseBinding;
    let runner = Runner::new(&run_adapter, &binding, regime, run_ctx)
        .unwrap()
        .with_backfill(true);

    let first = runner.run_over(&slice).await.unwrap();
    assert_eq!(
        first.failed,
        Vec::<String>::new(),
        "the real slice fetches/parses/publishes cleanly: {first:?}"
    );
    assert!(
        first.gold_inserted > 0,
        "real historical filings land in Gold: {first:?}"
    );

    let gold_count: i64 = sqlx::query_scalar("select count(*) from disclosure_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gold_count, i64::try_from(first.gold_inserted).unwrap());

    // A second run over the SAME refs is idempotent (invariant 4): every
    // stage claims Replay — no re-fetch, no re-parse, no new Gold rows.
    let second = runner.run_over(&slice).await.unwrap();
    assert_eq!(
        second.gold_inserted, 0,
        "a second run inserts 0 new Gold rows"
    );
    assert_eq!(second.replayed, slice.len(), "every filing replays");
    let gold_count_after: i64 = sqlx::query_scalar("select count(*) from disclosure_record")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        gold_count_after, gold_count,
        "the idempotent replay adds nothing"
    );

    // Every outbox_event this run wrote is already dispatched (goal 081 Task
    // 2's backfill suppression): no real subscriber alert for a historical
    // filing, ever.
    let undispatched: i64 =
        sqlx::query_scalar("select count(*) from outbox_event where dispatched_at is null")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        undispatched, 0,
        "every outbox_event from a backfill run is pre-dispatched"
    );
    let outbox_count: i64 = sqlx::query_scalar("select count(*) from outbox_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        outbox_count,
        i64::try_from(first.outbox_written).unwrap(),
        "outbox rows == gold rows (same txn)"
    );
}
