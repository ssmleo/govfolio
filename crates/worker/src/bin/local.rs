//! Offline local pipeline run (plan Task 9): drives the in-process runner
//! over the `us_house` fixture PDFs with NO network — the roster seeds from
//! the archived index evidence slice, inputs are local files, and every write
//! is idempotent (run it twice: the second run inserts nothing).
//!
//! Usage:
//!   cargo run -p worker --bin local -- \
//!     [--fixtures <dir>] [--bronze <dir>] [--index-xml <file>]
//!
//! `DATABASE_URL` must point at Postgres (e.g. the portable local PG 16 on
//! 5433; see `.env.example`).

use std::path::PathBuf;

use anyhow::Context as _;

use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::run::{LocalFiling, Runner};
use pipeline::stages::roster::seed_roster;
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;

/// The archived index slice (T8b evidence) — the offline official member list.
const EVIDENCE_SLICE: &str =
    "94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e.2026FD-slice.xml";

struct Args {
    fixtures: PathBuf,
    bronze: PathBuf,
    index_xml: PathBuf,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args {
        fixtures: fixtures_dir("us_house"),
        bronze: workspace_root().join("target").join("bronze-local"),
        index_xml: workspace_root()
            .join("docs")
            .join("regimes")
            .join("us-house")
            .join("evidence")
            .join(EVIDENCE_SLICE),
    };
    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        let mut value = |name: &str| {
            cli.next()
                .with_context(|| format!("{name} requires a value"))
                .map(PathBuf::from)
        };
        match flag.as_str() {
            "--fixtures" => args.fixtures = value("--fixtures")?,
            "--bronze" => args.bronze = value("--bronze")?,
            "--index-xml" => args.index_xml = value("--index-xml")?,
            other => anyhow::bail!(
                "unknown argument {other:?} (expected --fixtures/--bronze/--index-xml)"
            ),
        }
    }
    Ok(args)
}

/// `<fixtures>/<case>/input.pdf` for every case directory, sorted.
fn collect_inputs(fixtures: &PathBuf) -> anyhow::Result<Vec<LocalFiling>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(fixtures)
        .with_context(|| format!("reading fixtures dir {}", fixtures.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    let inputs: Vec<LocalFiling> = dirs
        .into_iter()
        .map(|dir| LocalFiling {
            path: dir.join("input.pdf"),
        })
        .filter(|filing| filing.path.is_file())
        .collect();
    anyhow::ensure!(
        !inputs.is_empty(),
        "no <case>/input.pdf files under {} — nothing to run",
        fixtures.display()
    );
    Ok(inputs)
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

    // Seed the regime row + politician roster (official member list, §5.4).
    seed_regime(&pool, &us_house::seed::regime_seed()).await?;
    let index_xml = std::fs::read_to_string(&args.index_xml)
        .with_context(|| format!("reading index XML {}", args.index_xml.display()))?;
    let roster = us_house::seed::roster_from_index_xml(&index_xml)?;
    let seeded = seed_roster(&pool, &us_house::seed::regime_binding(), &roster).await?;
    println!(
        "roster: {} members ({seeded} newly seeded) from {}",
        roster.len(),
        args.index_xml.display()
    );

    let inputs = collect_inputs(&args.fixtures)?;
    println!(
        "running {} local filings from {} (bronze: {})",
        inputs.len(),
        args.fixtures.display(),
        args.bronze.display()
    );

    let adapter = UsHouseAdapter::default();
    let binding = UsHouseBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(&args.bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;
    let runner = Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx)?;
    let report = runner.run_local(&inputs).await?;

    println!(
        "filings: {} | published: {} | replayed: {} | gold inserted: {} | \
         outbox written: {} | review tasks: {}",
        report.filings,
        report.published,
        report.replayed,
        report.gold_inserted,
        report.outbox_written,
        report.review_tasks
    );
    if !report.failed.is_empty() {
        for failure in &report.failed {
            eprintln!("FAILED {failure}");
        }
        anyhow::bail!("{} filing(s) failed closed", report.failed.len());
    }
    Ok(())
}
