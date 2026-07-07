//! Standing, report-only detection net for the `br` politician-identity CPF
//! collision defect class documented in
//! `docs/decisions/br-identity-collision-remediation.md` (§2 for the sweep
//! query, §9 for why this bin exists). §9's closing recommendation:
//!
//! > "adopting §2's sweep query as a standing, mechanical detection net...
//! > so this defect class cannot silently reappear at nationwide scale
//! > before the harder [CPF-aware resolver] prevention mechanism exists.
//! > That is a report/alert-only addition (no schema, no behavior change)."
//!
//! This bin is exactly that addition — nothing more. It runs the same
//! exhaustive, whole-`br`-dataset sweep that found and confirmed the one
//! live `JULIO CESAR DOS SANTOS` collision (fixed by
//! `fix-br-julio-cesar-santos-ba-2018.rs`; `SWEEP_SQL` below is copied
//! verbatim from that bin/the plan, not re-derived): for every `politician`,
//! does more than one distinct CPF (`stg_br.nr_cpf_candidato`) show up
//! across its filings? A PASS means zero rows come back.
//!
//! Report-only, by design (do NOT extend this bin to fix/write/delete
//! anything — that is what narrowly-scoped one-off `fix-br-*` bins are for,
//! built case-by-case per the plan's own "do not generalize speculatively"
//! guidance). It is also `br`-only: this defect class has not been found in
//! any other regime, so this bin does not attempt to generalize across
//! regimes either.
//!
//! Not a CI gate: nothing in this repo's command chain fails the build on
//! this bin's exit code. Run it manually, or wire it into a future `br`
//! epoch milestone / sentinel pass, per the plan's own suggestion.
//!
//! Usage:
//!   cargo run -p worker --bin check-br-identity-collisions
//!
//! Env: `DATABASE_URL` (required).
//!
//! Exit code: 0 = PASS (zero collisions found). Nonzero = one or more
//! collisions found and printed as a report for a human/agent to
//! investigate — mirrors `epoch-gate`'s PASS/BLOCKED convention (nonzero
//! means "look at this"), not a fail-closed halt.

use anyhow::Context as _;
use sqlx::PgPool;

/// One row of the sweep: a politician whose filings' `stg_br` rows carry
/// more than one distinct CPF.
#[derive(sqlx::FromRow)]
struct SweepRow {
    politician_id: String,
    canonical_name: String,
    distinct_cpfs: i64,
    cpfs: Vec<String>,
}

/// Copied verbatim from `docs/decisions/br-identity-collision-remediation.md`
/// §2 (and `fix-br-julio-cesar-santos-ba-2018.rs`'s own `SWEEP_SQL`) — an
/// exhaustive, whole-dataset sweep (every year, every body), not scoped to
/// any one politician.
const SWEEP_SQL: &str = "select p.id as politician_id, p.canonical_name, \
     count(distinct s.nr_cpf_candidato) as distinct_cpfs, \
     array_agg(distinct s.nr_cpf_candidato) as cpfs \
     from politician p \
     join filing f on f.politician_id = p.id \
     join raw_document rd on rd.id = f.raw_document_id \
     join stg_br s on s.raw_document_id = rd.id \
     where s.nr_cpf_candidato is not null \
     group by p.id, p.canonical_name \
     having count(distinct s.nr_cpf_candidato) > 1";

async fn run_sweep(pool: &PgPool) -> anyhow::Result<Vec<SweepRow>> {
    sqlx::query_as(SWEEP_SQL)
        .fetch_all(pool)
        .await
        .context("running the br politician-identity CPF-collision sweep")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    println!(
        "check-br-identity-collisions: sweeping every br politician for >1 distinct CPF \
         across their filings..."
    );
    let rows = run_sweep(&pool).await?;

    if rows.is_empty() {
        println!("PASS: zero br politician-identity CPF collisions found.");
        return Ok(());
    }

    println!(
        "REPORT: {} politician(s) with more than one distinct CPF across their filings — \
         this does NOT auto-fix anything; investigate each row before building a targeted \
         fix (see docs/decisions/br-identity-collision-remediation.md for the shape of the \
         prior JULIO CESAR DOS SANTOS fix):",
        rows.len()
    );
    for row in &rows {
        println!(
            "  politician_id={} canonical_name={:?} distinct_cpfs={} cpfs={:?}",
            row.politician_id, row.canonical_name, row.distinct_cpfs, row.cpfs
        );
    }
    std::process::exit(1);
}
