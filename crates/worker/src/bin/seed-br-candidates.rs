//! Historical `br` candidate-roster seeding CLI — the precondition
//! `bin/backfill-real-br.rs`'s own doc comment says is a SEPARATE step, not
//! performed by that bin. See `br::seed`'s own module doc comment for why
//! this regime's "roster" is minted per-candidate rather than pre-loaded
//! from a separate member-list authority the way `us_house`'s Clerk index
//! provides (`bin/seed-historical-rosters.rs`, the precedent this mirrors).
//!
//! For each year in `--from..=--to`, discovers that year's in-scope
//! candidates (`BrAdapter::discover_year`, the SAME conditional-GET fetch
//! filing discovery/the real backfill use — invariant 10) and seeds a
//! `politician` + `mandate` row for every candidate whose `DS_CARGO` maps to
//! a `br::seed::RosterBody` — `DEPUTADO FEDERAL` (Câmara dos Deputados) AND,
//! as of this pass, `SENADOR`/`1º SUPLENTE`/`2º SUPLENTE` (Senado Federal)
//! (`br::seed::seed_candidates_year`; see that module's doc comment for the
//! multi-body design and the suplente-handling decision). Each year fails
//! closed INDEPENDENTLY (an unreachable/unparseable year is printed and does
//! not sink the rest of the range); within a year, each candidate seeds
//! independently too — an ambiguous match (e.g. two politicians already
//! resolve the same name+state WITHIN one body) is printed/counted as a
//! skip, not a whole-year failure.
//!
//! `--uf <CODE[,CODE...]>` is a PROOF-ONLY bound: it additionally restricts
//! seeding to the listed states. `docs/regimes/br/AUTHORITY.md`'s own
//! dry-run proof found 11423 in-scope candidates for 2022 alone — seeding
//! every one of them nationwide is a later, independently-audited
//! increment, not this pass. Omit `--uf` to seed every in-scope candidate
//! `discover_year` returns for the year.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin seed-br-candidates -- --from 2022 [--to <year>] [--uf AC,AL]
//! ```
//!
//! Env: `DATABASE_URL` (required — this bin writes `politician`/
//! `politician_alias`/`mandate`).
//!
//! Exit code: 0 even when individual years/candidates failed closed
//! (per-year/per-candidate isolation is the design, not an error) —
//! nonzero only on genuine setup failure (bad args, unreachable
//! `DATABASE_URL`).

use anyhow::Context as _;
use chrono::Datelike as _;

use br::BrAdapter;
use br::seed::seed_candidates_year;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx, ScratchDir};
use pipeline::stages::seed::seed_regime;

struct Args {
    from: i32,
    to: i32,
    ufs: Vec<String>,
}

fn parse_args() -> anyhow::Result<Args> {
    let current_year = chrono::Utc::now().year();
    let mut from: Option<i32> = None;
    let mut to: Option<i32> = None;
    let mut ufs: Vec<String> = Vec::new();

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
            "--uf" => {
                ufs = value("--uf")?
                    .split(',')
                    .map(|s| s.trim().to_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            other => anyhow::bail!("unknown argument {other:?} (expected --from/--to/--uf)"),
        }
    }

    let from = from.context("--from is required (e.g. --from 2022)")?;
    let to = to.unwrap_or(current_year);
    anyhow::ensure!(from <= to, "--from {from} is after --to {to}");
    anyhow::ensure!(
        (1933..=current_year + 1).contains(&from),
        "--from {from} is outside br's archived range (1933..={current_year})"
    );
    Ok(Args { from, to, ufs })
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
    seed_regime(&pool, &br::seed::regime_seed()).await?;
    seed_regime(&pool, &br::seed::regime_seed_senado()).await?;

    let adapter = BrAdapter::default();
    let bronze = std::env::temp_dir().join(format!(
        "govfolio-seed-br-candidates-{}",
        std::process::id()
    ));
    // Ephemeral: this pass only seeds politician/mandate rows, never
    // raw_document — removed on drop (success, error, or panic) so real `br`
    // CPF/voter-registration numbers never linger under the OS temp dir
    // (docs/regimes/br/AUTHORITY.md).
    let _scratch = ScratchDir::new(bronze.clone());
    let ctx = RunCtx::new(
        BronzeStore::open(bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;
    let mut total_inserted = 0u32;
    let mut total_errors = 0usize;
    let mut failed_years = 0usize;
    for year in args.from..=args.to {
        match seed_candidates_year(&adapter, &ctx, &pool, year, &args.ufs).await {
            Ok(result) => {
                println!(
                    "{year}: discovered {} | considered {} (uf filter {:?}) | seeded {} | \
                     skipped (other cargo) {} | errors {}",
                    result.discovered,
                    result.considered,
                    args.ufs,
                    result.inserted,
                    result.skipped_other_cargo,
                    result.errors.len()
                );
                for error in &result.errors {
                    eprintln!(
                        "{year}: SKIPPED {:?} ({}) — {}",
                        error.filed_alias, error.district, error.error
                    );
                }
                total_inserted += result.inserted;
                total_errors += result.errors.len();
            }
            Err(error) => {
                eprintln!("{year}: FAILED CLOSED — {error:#}");
                failed_years += 1;
            }
        }
    }
    println!(
        "TOTAL {}..={}: seeded {} candidate(s), {} error(s), {} year(s) failed closed",
        args.from, args.to, total_inserted, total_errors, failed_years
    );
    Ok(())
}
