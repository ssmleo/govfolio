//! Historical `us_house` roster seeding CLI (goal 081 Task 5 prerequisite):
//! a thin wrapper around the already-fully-tested
//! [`us_house::seed::seed_historical_rosters`] (goal 081 Task 1; per-member
//! isolation added Task 5) — no new business logic here, just the CLI/DB
//! wiring `crates/worker/src/bin/backfill-real.rs`'s own doc comment says is
//! left for whoever wires Task 5's full rehearsal/execution.
//!
//! For each year in `--from..=--to`, fetches that year's Clerk index XML via
//! [`us_house::seed::LiveIndexSource`] (the SAME conditional-GET fetch filing
//! discovery uses, invariant 10) and seeds the roster
//! (`roster_from_index_xml` + `seed_roster`). Each year fails closed
//! INDEPENDENTLY — an unreachable/unparseable year is printed and counted,
//! but never sinks the rest of the range (mirrors `bin/backfill-real.rs`'s
//! per-year isolation). WITHIN a year, each roster member is seeded
//! independently too: an ambiguous member is printed/counted as a skip, not
//! a whole-year failure, and every other real member in that year still
//! seeds.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin seed-historical-rosters -- --from 2012 [--to <year>]
//! ```
//!
//! Env: `DATABASE_URL` (required — this bin writes the `politician`/
//! `politician_alias`/`mandate` tables).
//!
//! Exit code: 0 even when individual years failed closed (per-year isolation
//! is the design, not an error) — nonzero only on genuine setup failure
//! (bad args, unreachable `DATABASE_URL`).

use anyhow::Context as _;
use chrono::Datelike as _;

use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx, ScratchDir};
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::seed::{LiveIndexSource, seed_historical_rosters};

struct Args {
    from: i32,
    to: i32,
}

fn parse_args() -> anyhow::Result<Args> {
    let current_year = chrono::Utc::now().year();
    let mut from: Option<i32> = None;
    let mut to: Option<i32> = None;

    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        let mut value = |name: &str| {
            cli.next()
                .with_context(|| format!("{name} requires a value"))
        };
        match flag.as_str() {
            "--from" => {
                from = Some(value("--from")?.parse().context("--from must be a year")?);
            }
            "--to" => to = Some(value("--to")?.parse().context("--to must be a year")?),
            other => anyhow::bail!("unknown argument {other:?} (expected --from/--to)"),
        }
    }

    let from = from.context("--from is required (e.g. --from 2012)")?;
    let to = to.unwrap_or(current_year);
    anyhow::ensure!(from <= to, "--from {from} is after --to {to}");
    anyhow::ensure!(
        (2012..=current_year + 1).contains(&from),
        "--from {from} is outside the archived range (2012..={current_year})"
    );
    Ok(Args { from, to })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool)
        .await
        .context("applying migrations")?;
    seed_regime(&pool, &us_house::seed::regime_seed()).await?;

    let adapter = UsHouseAdapter::default();
    let bronze = std::env::temp_dir().join(format!(
        "govfolio-seed-historical-rosters-{}",
        std::process::id()
    ));
    // Ephemeral: this pass only seeds politician/mandate rows, never
    // raw_document — removed on drop (success, error, or panic).
    let _scratch = ScratchDir::new(bronze.clone());
    let ctx = RunCtx::new(
        BronzeStore::open(bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;

    let source = LiveIndexSource {
        adapter: &adapter,
        ctx: &ctx,
    };
    let regime = us_house::seed::regime_binding();

    let results = seed_historical_rosters(&source, &pool, &regime, args.from, args.to).await;

    let mut total_inserted = 0u32;
    let mut total_skipped_members = 0usize;
    let mut failed_years = 0usize;
    for result in &results {
        match &result.error {
            None => {
                println!(
                    "{}: seeded {} roster member(s), {} skipped (ambiguous)",
                    result.year,
                    result.inserted,
                    result.member_errors.len()
                );
                for member_error in &result.member_errors {
                    eprintln!(
                        "{}: SKIPPED {:?} ({}) — {}",
                        result.year,
                        member_error.filed_alias,
                        member_error.district,
                        member_error.error
                    );
                }
                total_inserted += result.inserted;
                total_skipped_members += result.member_errors.len();
            }
            Some(error) => {
                eprintln!("{}: FAILED CLOSED — {error}", result.year);
                failed_years += 1;
            }
        }
    }
    println!(
        "TOTAL {}..={}: seeded {} roster member(s) across {} year(s), {} member(s) skipped \
         (ambiguous), {} year(s) failed closed",
        args.from,
        args.to,
        total_inserted,
        results.len(),
        total_skipped_members,
        failed_years
    );
    Ok(())
}
