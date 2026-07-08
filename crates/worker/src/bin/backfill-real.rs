//! Real (write-to-prod) US House archive backfill (goal 081 Task 3): "backfill
//! = the same pipeline pointed at archives" — but ACTUALLY WRITING, unlike
//! `bin/backfill.rs`'s dry-run-only half (kept separate, untouched — it still
//! hard-refuses to run without `--dry-run`).
//!
//! For each year in `--from..=--to`: [`UsHouseAdapter::discover_year`] (the
//! FULL per-year `FilingRef` list — no `--limit`/sampling) then every
//! discovered filing through the real write chain
//! ([`pipeline::run::Runner::run_over`], reused as-is), in backfill mode
//! (goal 081 Task 2's `FilingSpec::backfill` flag): Gold rows are real, but
//! each `outbox_event` is written already `dispatched_at`-stamped, so the
//! matcher never fires a real subscriber alert for a historical filing.
//! `pipeline_run` claim/idempotency makes a kill-and-resume, or a repeat
//! invocation, safe — an already-fetched/parsed/published filing replays
//! instead of rewriting (invariant 4).
//!
//! Goal 081 Task 4: before each year's real write pass, a `BACKFILL_BUDGET`
//! gate (`worker::backfill::gate_year`) reads the EXISTING dry-run's
//! `record_delta` for that year (no new prediction/classification code) and
//! either proceeds (`record_delta <= BACKFILL_BUDGET`, default 500) or skips
//! that year — logged to `agents/JOURNAL.md`, nothing blocks the range, a
//! later invocation naturally retries the skipped year. This mirrors
//! `scripts/check-tf-plan.sh`'s numeric-count-vs-env-var-budget shape and
//! replaces the founder go/no-go goal 080 left as a HALT.
//!
//! The historical politician roster for the year range must already be
//! seeded (`us_house::seed::seed_historical_rosters`, goal 081 Task 1) —
//! this bin does not seed it. An unresolved filer fails that ONE filing
//! closed (`review_task` opened, invariant 3: never guess) without sinking
//! the rest of the range.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin backfill-real -- --from 2012 [--to <year>] [--limit <N>]
//! ```
//!
//! `--limit <N>` (goal 082, founder-directed 2026-07-08): BOUNDED per-year sample —
//! each year's discovered `FilingRef` list is truncated to N before the write pass
//! (`sampled N of M` printed; never silently). Under `--limit` the full-year
//! `gate_year` pre-pass (which dry-processes the ENTIRE year to measure
//! `record_delta`) is replaced by the mechanical upper bound `sampled_count <=
//! BACKFILL_BUDGET`: with at most N filings written, the worst case counts every
//! sampled filing as an add, so the bound IS the gate — same fail-closed shape,
//! no second full-year fetch pass. Without `--limit`, behavior is unchanged.
//! Intended for LOCAL verification runs (`DATABASE_URL` = the dev Postgres on
//! 5433); the goal-080/081 prod HALTs are untouched by this flag.
//!
//! Env: `DATABASE_URL` (required — this bin writes Bronze/Silver/Gold, unlike
//! `bin/backfill.rs`'s optional connection for its no-write dry run).

use anyhow::Context as _;
use chrono::Datelike as _;

use pipeline::adapter::{BronzeStore, Clock, FilingRef, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::workspace_root;
use pipeline::run::{RunReport, Runner};
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;
use worker::backfill::{
    BackfillRunKind, BackfillRunRecord, BackfillRunStatus, counter_i64, record_backfill_run,
};

struct Args {
    from: i32,
    to: i32,
    limit: Option<usize>,
}

fn parse_args() -> anyhow::Result<Args> {
    let current_year = chrono::Utc::now().year();
    let mut from: Option<i32> = None;
    let mut to: Option<i32> = None;
    let mut limit: Option<usize> = None;

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
            "--limit" => {
                let n: usize = value("--limit")?
                    .parse()
                    .context("--limit must be a positive integer")?;
                anyhow::ensure!(n > 0, "--limit must be >= 1 (omit it for the full run)");
                limit = Some(n);
            }
            other => anyhow::bail!("unknown argument {other:?} (expected --from/--to/--limit)"),
        }
    }

    let from = from.context("--from is required (e.g. --from 2012)")?;
    let to = to.unwrap_or(current_year);
    anyhow::ensure!(from <= to, "--from {from} is after --to {to}");
    anyhow::ensure!(
        (2012..=current_year + 1).contains(&from),
        "--from {from} is outside the archived range (2012..={current_year})"
    );
    Ok(Args { from, to, limit })
}

/// Accumulates one run's totals across every year processed.
fn add_report(total: &mut RunReport, year: i32, report: RunReport) {
    total.filings += report.filings;
    total.published += report.published;
    total.replayed += report.replayed;
    total.gold_inserted += report.gold_inserted;
    total.outbox_written += report.outbox_written;
    total.review_tasks += report.review_tasks;
    total
        .failed
        .extend(report.failed.into_iter().map(|f| format!("[{year}] {f}")));
}

#[tokio::main]
// `log_budget_skip`'s new required `regime_code` parameter (added when `br`
// gained its own real-write bin, `bin/backfill-real-br.rs`) reformats this
// call across more lines, pushing this otherwise-unchanged function 6 lines
// past the limit — not a genuine complexity regression.
#[allow(clippy::too_many_lines)]
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
    // NOT under an OS temp directory, and NOT wrapped in ScratchDir: this ctx
    // backs the REAL write pass below (Runner::run_over) — its Bronze root is
    // durably referenced via raw_document.storage_uri (invariant 2, raw is
    // sacred) and must never live anywhere a generic temp-dir sweep (disk
    // cleanup utility, reboot policy, or an ad-hoc manual cleanup — see
    // agents/JOURNAL.md's 2026-07-07 incident) could reach it. `target/` is
    // gitignored but not OS-temp, and already this project's convention for
    // durable-but-not-committed local state (`bin/local.rs`'s
    // `target/bronze-local`); a fixed (non-PID) path is correct here since
    // BronzeStore is content-addressed — re-invocations accumulate/reuse the
    // same store rather than leaking a new directory per run. Unlike the
    // gate's own scratch Bronze (ClerkArchive, wrapped in ScratchDir) a few
    // lines down.
    let bronze = workspace_root().join("target").join("bronze-backfill-real");
    let ctx = RunCtx::new(
        BronzeStore::open(bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;

    // Discover FIRST, over every year, sharing this ONE `ctx` (one politeness
    // throttle, invariant 10) — `ctx` moves into the `Runner` below, so every
    // year's full `FilingRef` list is collected before any real write starts.
    let mut by_year: Vec<(i32, Vec<FilingRef>)> = Vec::new();
    for year in args.from..=args.to {
        let mut refs = adapter
            .discover_year(year, &ctx)
            .await
            .with_context(|| format!("discovering {year}"))?;
        match args.limit {
            Some(n) if refs.len() > n => {
                println!(
                    "{year}: {} filing(s) discovered — sampling {n} (--limit; goal 082)",
                    refs.len()
                );
                refs.truncate(n);
            }
            _ => println!("{year}: {} filing(s) discovered", refs.len()),
        }
        by_year.push((year, refs));
    }

    let binding = UsHouseBinding;
    let runner =
        Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx)?.with_backfill(true);

    // Goal 081 Task 4: BACKFILL_BUDGET gate. A SEPARATE ClerkArchive + real Gold
    // baseline (its own scratch Bronze + adapter instance, own conditional-GET
    // cache) drives the EXISTING dry_run per year to read record_delta before
    // committing to that year's real write.
    let budget = worker::backfill::backfill_budget();
    let gate_scratch = std::env::temp_dir().join(format!(
        "govfolio-backfill-real-gate-{}",
        std::process::id()
    ));
    let gate_source = worker::backfill::ClerkArchive::new(Some(pool.clone()), gate_scratch)?;
    let gate_baseline =
        worker::backfill::PgBaseline::new(pool.clone(), us_house::seed::REGIME_ID.to_owned());
    let journal_root = worker::backfill::workspace_root();

    let mut total = RunReport::default();
    for (year, refs) in by_year {
        // Admin observability (plan §Architecture 3): every arm below —
        // skipped_budget / succeeded / failed — records one per-year
        // `backfill_run` row; `started_at` marks where this year's
        // processing (gate included) began.
        let started_at = chrono::Utc::now();
        // The scope note the row carries: the --limit bound when sampling
        // (goal 082), full-year otherwise.
        let scope = args.limit.map(|n| format!("--limit {n}"));
        // Goal 082: under --limit the sample bound is the gate — at most
        // refs.len() filings can write, so the worst case (every sampled
        // filing an add) is compared to BACKFILL_BUDGET directly instead of
        // dry-processing the ENTIRE year to measure record_delta.
        let verdict = if args.limit.is_some() {
            println!(
                "{year}: sampled gate — {} filing(s) sampled; worst-case delta {} <= \
                 BACKFILL_BUDGET {budget} required (full-year gate_year skipped under \
                 --limit, goal 082)",
                refs.len(),
                refs.len()
            );
            worker::backfill::budget_verdict(refs.len(), budget)
        } else {
            worker::backfill::gate_year(&gate_source, &gate_baseline, "us_house", year, budget)
                .await?
        };
        let record_delta = match verdict {
            worker::backfill::BudgetVerdict::Skip { record_delta } => {
                println!(
                    "{year}: SKIPPED — record_delta {record_delta} exceeds BACKFILL_BUDGET \
                         {budget}; logged to agents/JOURNAL.md, continuing (nothing blocks)"
                );
                worker::backfill::log_budget_skip(
                    &journal_root,
                    "us_house",
                    year,
                    record_delta,
                    budget,
                )?;
                record_backfill_run(
                    &pool,
                    &BackfillRunRecord {
                        scope: scope.clone(),
                        record_delta: counter_i64(record_delta),
                        budget: Some(counter_i64(budget)),
                        ..BackfillRunRecord::new(
                            "us_house",
                            year,
                            BackfillRunKind::Backfill,
                            "backfill-real",
                            BackfillRunStatus::SkippedBudget,
                            started_at,
                        )
                    },
                )
                .await?;
                continue;
            }
            worker::backfill::BudgetVerdict::Proceed { record_delta } => record_delta,
        };
        println!(
            "{year}: budget OK (record_delta {record_delta} <= BACKFILL_BUDGET {budget}) — \
             proceeding to the real write"
        );
        let report = match runner
            .run_over(&refs)
            .await
            .with_context(|| format!("real write pass for {year}"))
        {
            Ok(report) => report,
            Err(error) => {
                // Failed row first, then propagate. The row INSERT itself is
                // best-effort here: it must never mask the year's real error
                // (which is about to abort the run anyway).
                let failed_row = BackfillRunRecord {
                    scope: scope.clone(),
                    record_delta: counter_i64(record_delta),
                    budget: Some(counter_i64(budget)),
                    error: Some(format!("{error:#}")),
                    ..BackfillRunRecord::new(
                        "us_house",
                        year,
                        BackfillRunKind::Backfill,
                        "backfill-real",
                        BackfillRunStatus::Failed,
                        started_at,
                    )
                };
                if let Err(record_error) = record_backfill_run(&pool, &failed_row).await {
                    eprintln!(
                        "WARNING: backfill_run row for failed {year} not recorded: \
                         {record_error:#}"
                    );
                }
                return Err(error);
            }
        };
        println!(
            "{year}: published {} | replayed {} | gold inserted {} | outbox written {} | \
             review tasks {} | failed {}",
            report.published,
            report.replayed,
            report.gold_inserted,
            report.outbox_written,
            report.review_tasks,
            report.failed.len()
        );
        record_backfill_run(
            &pool,
            &BackfillRunRecord {
                scope: scope.clone(),
                record_delta: counter_i64(record_delta),
                budget: Some(counter_i64(budget)),
                ..BackfillRunRecord::new(
                    "us_house",
                    year,
                    BackfillRunKind::Backfill,
                    "backfill-real",
                    BackfillRunStatus::Succeeded,
                    started_at,
                )
                .with_report(&report)
            },
        )
        .await?;
        add_report(&mut total, year, report);
    }

    println!(
        "TOTAL {}..={}: filings {} | published {} | replayed {} | gold inserted {} | \
         outbox written {} | review tasks {} | failed {}",
        args.from,
        args.to,
        total.filings,
        total.published,
        total.replayed,
        total.gold_inserted,
        total.outbox_written,
        total.review_tasks,
        total.failed.len()
    );
    if !total.failed.is_empty() {
        // Fail-closed per filing, not per run (invariant 6): loud, non-fatal.
        for failure in &total.failed {
            eprintln!("FAILED {failure}");
        }
    }
    Ok(())
}
