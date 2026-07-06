//! US archive backfill (goal 080, design §5.6): "backfill = the same pipeline
//! pointed at archives." This bin drives the **dry-run** half — discover the
//! Clerk's per-year `{YYYY}FD.zip` indexes back to the 2012 STOCK Act era,
//! dry-process a BOUNDED per-year sample, and print a diff report (adds /
//! changes / supersessions) WITHOUT writing Bronze/Silver/Gold.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --dry-run
//!   [--to <year>]      last archive year (default: current year)
//!   [--limit <N>]      per-year dry-process sample bound (default 5; 0 =
//!                      discover-only — count filings, do not fetch/classify)
//! ```
//!
//! `--dry-run` is REQUIRED. The real (write-to-prod) backfill is a HALT: it
//! needs the cloud substrate applied (goal 020 ADC), a founder diff-approval,
//! and a go/no-go. Without `--dry-run` this bin refuses and prints those
//! preconditions.
//!
//! Env: `DATABASE_URL` (optional). When set + reachable, the dry run classifies
//! each sampled filing against the real Gold (add/change/supersession/
//! unchanged). When unset/unreachable, it degrades to discover-only (counts),
//! honestly noted. The archive fetch is polite (invariant 10). If the network
//! is unreachable, the run exits cleanly with an "ARCHIVE UNREACHABLE" banner —
//! an honest no-op, never a false green.

use anyhow::Context as _;
use chrono::Datelike as _;

use worker::backfill::{ClerkArchive, DiffReport, GoldBaseline, NoBaseline, PgBaseline, dry_run};

/// Default per-year sample bound (honest cap; the full scope is always reported
/// in the diff's `discovered` column). A full 2012→now backfill is thousands of
/// PDFs — the real run (a HALT) does the whole set; the dry run samples.
const DEFAULT_LIMIT: usize = 5;

struct Args {
    adapter: String,
    from: i32,
    to: i32,
    limit: usize,
    dry_run: bool,
}

fn parse_args() -> anyhow::Result<Args> {
    let current_year = chrono::Utc::now().year();
    let mut adapter: Option<String> = None;
    let mut from: Option<i32> = None;
    let mut to: Option<i32> = None;
    let mut limit = DEFAULT_LIMIT;
    let mut dry_run = false;

    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        let mut value = |name: &str| {
            cli.next()
                .with_context(|| format!("{name} requires a value"))
        };
        match flag.as_str() {
            "--adapter" => adapter = Some(value("--adapter")?),
            "--from" => {
                from = Some(value("--from")?.parse().context("--from must be a year")?);
            }
            "--to" => to = Some(value("--to")?.parse().context("--to must be a year")?),
            "--limit" => {
                limit = value("--limit")?
                    .parse()
                    .context("--limit must be a number")?;
            }
            "--dry-run" => dry_run = true,
            other => anyhow::bail!(
                "unknown argument {other:?} (expected --adapter/--from/--to/--limit/--dry-run)"
            ),
        }
    }

    let adapter = adapter.context("--adapter is required (e.g. --adapter us_house)")?;
    anyhow::ensure!(
        adapter == "us_house",
        "adapter {adapter:?} has no backfill wiring — only us_house is archived to 2012 today"
    );
    let from = from.context("--from is required (e.g. --from 2012)")?;
    let to = to.unwrap_or(current_year);
    anyhow::ensure!(from <= to, "--from {from} is after --to {to}");
    anyhow::ensure!(
        (2012..=current_year + 1).contains(&from),
        "--from {from} is outside the archived range (2012..={current_year})"
    );
    Ok(Args {
        adapter,
        from,
        to,
        limit,
        dry_run,
    })
}

/// The real-run HALT — printed when `--dry-run` is absent. The write-to-prod
/// backfill has genuine human/infra preconditions and must never run
/// unattended.
fn print_real_run_halt() {
    eprintln!(
        "REFUSING: the real (write-to-prod) backfill is a HALT — it is NOT run unattended.\n\
         Preconditions (in order), per agents/goals/080-backfill-launch.md '## HALT (human/infra)':\n\
           1. Founder runs `gcloud auth application-default login` once (ADC — goal 020 halt).\n\
           2. Apply the cloud substrate: terraform -chdir=infra plan -> check-tf-plan.sh -> apply.\n\
           3. Run this bin WITHOUT --dry-run against the applied substrate.\n\
           4. Founder reviews the emitted diff (adds/changes/supersessions) and gives go/no-go\n\
              BEFORE any mass supersession is promoted (design §5.6: human-gated for mass changes).\n\
         Re-run with --dry-run to preview the diff safely (no writes)."
    );
}

/// Connects to Postgres when `DATABASE_URL` is set + reachable; otherwise
/// `None` (discover-only). A connection failure is degraded, not fatal — the
/// dry run still counts the archives.
async fn connect_optional_pool() -> Option<sqlx::PgPool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("note: DATABASE_URL unset — running discover-only (no Gold baseline).");
        return None;
    };
    match sqlx::PgPool::connect(&url).await {
        Ok(pool) => Some(pool),
        Err(error) => {
            eprintln!(
                "note: DATABASE_URL set but unreachable ({error}); running discover-only \
                 (no Gold baseline)."
            );
            None
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;

    if !args.dry_run {
        print_real_run_halt();
        std::process::exit(2);
    }

    // Scratch Bronze: fetched bytes buffer here to feed the deterministic
    // parser, then are discarded. This is NOT the durable Bronze ledger
    // (raw_document) and NOT the prod object store — the dry run writes nothing
    // to the data plane.
    let scratch = std::env::temp_dir().join(format!(
        "govfolio-backfill-dryrun-{}-{}",
        args.adapter,
        std::process::id()
    ));

    // DATABASE_URL is optional. With it, we classify against real Gold; without
    // it (or if it is unreachable), we degrade to discover-only, honestly noted.
    let pool = connect_optional_pool().await;

    // No DB => discover-only (limit 0): we cannot classify add/change without a
    // Gold baseline, so we do not fetch PDFs at all — just count the archives.
    let effective_limit = if pool.is_some() { args.limit } else { 0 };

    let source = ClerkArchive::new(pool.clone(), scratch)?;
    let baseline: Box<dyn GoldBaseline> = match &pool {
        Some(pool) => Box::new(PgBaseline::new(
            pool.clone(),
            us_house::seed::REGIME_ID.to_owned(),
        )),
        None => Box::new(NoBaseline),
    };

    let report: DiffReport = dry_run(
        &source,
        baseline.as_ref(),
        args.from,
        args.to,
        effective_limit,
    )
    .await?;

    if report.archive_unreachable() {
        // Honest degradation, NOT a false green: nothing was reachable to diff.
        let banner = "\nARCHIVE UNREACHABLE — every archive year fetch failed (network/DNS).\n\
             This is an honest no-op exit, not a diff: no filings were reachable to classify.\n\
             The dry-run machinery itself is verified offline via test fixtures\n\
             (`cargo test -p worker --test backfill`).";
        println!("{report}");
        println!("{banner}");
        eprintln!("{banner}");
        return Ok(());
    }

    println!("{report}");
    Ok(())
}
