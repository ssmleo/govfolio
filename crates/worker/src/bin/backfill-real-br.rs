//! Real (write-to-prod) `br` archive backfill — the `br` equivalent of
//! `bin/backfill-real.rs`'s `us_house` real write path (see that file's own
//! doc comment for the shared pattern this mirrors, unchanged there): for
//! each year in `--from..=--to`, every discovered candidate goes through the
//! real write chain ([`pipeline::run::Runner::run_over`], reused as-is), in
//! backfill mode (`FilingSpec::backfill` suppresses `outbox_event`
//! dispatch — Gold rows are real, but each `outbox_event` is written already
//! `dispatched_at`-stamped, so the matcher never fires a real subscriber
//! alert for a historical filing), gated per year by
//! `worker::backfill::gate_year`'s `BACKFILL_BUDGET` check.
//! `pipeline_run` claim/idempotency makes a kill-and-resume, or a repeat
//! invocation, safe.
//!
//! Differs from `bin/backfill-real.rs` in two ways `us_house` does not need:
//!
//! 1. **Roster seeding is a SEPARATE precondition**
//!    (`cargo run -p worker --bin seed-br-candidates`, NOT this bin) — see
//!    that bin's and `br::seed`'s own doc comments for why `br` has no
//!    durable member-list roster the way `us_house`'s Clerk index provides.
//!    An unseeded candidate still fails that ONE candidacy closed
//!    (`unresolved_filer` `review_task`, invariant 3) without sinking the
//!    rest of the range — exactly like an unseeded `us_house` filer would.
//!
//! 2. **`--uf <CODE[,CODE...]>`** — a PROOF-ONLY bound. A single `br`
//!    federal-election year holds thousands of candidates nationwide
//!    (11423 for 2022 alone, per `docs/regimes/br/AUTHORITY.md`'s own
//!    dry-run proof) — far more than a first real-write proof should
//!    attempt. Omitting `--uf` processes the WHOLE discovered year, exactly
//!    like `backfill-real.rs`'s unbounded `us_house` path; a genuine full
//!    historical `br` backfill is a later, independently-audited increment
//!    (this bin's own doc comment does not claim otherwise).
//!
//! The `BACKFILL_BUDGET` gate itself is scoped to match: it measures
//! `record_delta` for the SAME `--uf`-bounded candidate set this invocation
//! will actually write (see [`UfScopedArchive`] below), not the whole
//! nationwide year — otherwise a small, deliberate `--uf`-bounded proof run
//! would always measure the full year's ~11000-record delta against the
//! default 500-record budget and skip every year outright.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin backfill-real-br -- --from 2022 [--to <year>] [--uf AC,AL]
//! ```
//!
//! Env: `DATABASE_URL` (required — this bin writes Bronze/Silver/Gold).

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::Datelike as _;

use br::BrAdapter;
use br::binding::BrBinding;
use br::seed::discover_candidates_year;
use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    BronzeStore, Clock, FilingRef, JurisdictionAdapter as _, RunCtx, ScratchDir,
};
use pipeline::run::{RunReport, Runner};
use pipeline::stages::seed::seed_regime;
use worker::backfill::{ArchiveSource, BudgetVerdict, DiscoveredFiling, PgBaseline};

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
    let to = to.unwrap_or(from);
    anyhow::ensure!(from <= to, "--from {from} is after --to {to}");
    anyhow::ensure!(
        (1933..=current_year + 1).contains(&from),
        "--from {from} is outside br's archived range (1933..={current_year})"
    );
    Ok(Args { from, to, ufs })
}

/// A [`worker::backfill::ArchiveSource`] scoped to the SAME `--uf` bound the
/// real write pass below uses (see module doc comment for why the
/// `BACKFILL_BUDGET` gate must be measured against this bounded set, not the
/// whole nationwide year). Wraps its own `BrAdapter` + scratch `RunCtx`
/// (mirrors `worker::backfill::TseArchive`'s own shape) — a separate process
/// instance from the real-write `Runner`'s adapter below, so this never
/// shares (or interferes with) that adapter's `joined_cache`.
struct UfScopedArchive {
    adapter: BrAdapter,
    ctx: RunCtx,
    ufs: Vec<String>,
    /// Removes `scratch` on drop (success, error, or panic) — this gate's
    /// Bronze root is a throwaway buffer (never durably referenced; `br`
    /// documents carry real CPF/voter-registration numbers,
    /// `docs/regimes/br/AUTHORITY.md`), unlike the real-write `ctx` this
    /// bin's own `main` builds separately below (that one's Bronze root
    /// stays unwrapped — it IS durably referenced via
    /// `raw_document.storage_uri`, invariant 2).
    _scratch: ScratchDir,
}

impl UfScopedArchive {
    fn new(
        pool: Option<sqlx::PgPool>,
        scratch: std::path::PathBuf,
        ufs: Vec<String>,
    ) -> anyhow::Result<Self> {
        let adapter = BrAdapter::default();
        let guard = ScratchDir::new(scratch.clone());
        let ctx = RunCtx::new(
            BronzeStore::open(scratch)?,
            pool,
            Clock::System,
            &adapter.politeness(),
        )?;
        Ok(Self {
            adapter,
            ctx,
            ufs,
            _scratch: guard,
        })
    }
}

#[async_trait]
impl ArchiveSource for UfScopedArchive {
    async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>> {
        let candidates = discover_candidates_year(&self.adapter, &self.ctx, year).await?;
        Ok(candidates
            .into_iter()
            .filter(|c| self.ufs.is_empty() || self.ufs.contains(&c.sg_uf))
            .map(|c| DiscoveredFiling {
                external_id: c.filing_ref.external_id,
                url: c.filing_ref.url,
            })
            .collect())
    }

    async fn dry_process(&self, filing: &DiscoveredFiling) -> anyhow::Result<Vec<GoldCandidate>> {
        let filing_ref = FilingRef {
            external_id: filing.external_id.clone(),
            url: filing.url.clone(),
        };
        let doc = self.adapter.fetch(&filing_ref, &self.ctx).await?;
        let rows = self.adapter.parse(&doc, &self.ctx).await?;
        self.adapter.normalize(&rows, &self.ctx).await
    }
}

/// Accumulates one run's totals across every year processed (mirrors
/// `bin/backfill-real.rs`'s own `add_report`).
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

/// Discovers every year in `from..=to`, applying the `--uf` bound, BEFORE
/// any real write starts (one politeness throttle for the whole discovery
/// pass, invariant 10).
async fn discover_by_year(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    from: i32,
    to: i32,
    ufs: &[String],
) -> anyhow::Result<Vec<(i32, Vec<FilingRef>)>> {
    let mut by_year = Vec::new();
    for year in from..=to {
        let candidates = discover_candidates_year(adapter, ctx, year)
            .await
            .with_context(|| format!("discovering {year}"))?;
        let scoped: Vec<FilingRef> = candidates
            .into_iter()
            .filter(|c| ufs.is_empty() || ufs.contains(&c.sg_uf))
            .map(|c| c.filing_ref)
            .collect();
        println!(
            "{year}: {} filing(s) in scope (uf filter {ufs:?})",
            scoped.len()
        );
        by_year.push((year, scoped));
    }
    Ok(by_year)
}

/// Gates one year against `BACKFILL_BUDGET` (scoped to the same `--uf`
/// bound, see module/`UfScopedArchive` doc comments) then, on
/// [`BudgetVerdict::Proceed`], runs the real write pass over `refs`.
/// Returns `None` on a budget skip (logged to `agents/JOURNAL.md`, nothing
/// blocks the range).
async fn gate_and_write_year(
    runner: &Runner<'_>,
    gate_source: &UfScopedArchive,
    gate_baseline: &PgBaseline,
    journal_root: &std::path::Path,
    budget: usize,
    year: i32,
    refs: &[FilingRef],
) -> anyhow::Result<Option<RunReport>> {
    let record_delta =
        match worker::backfill::gate_year(gate_source, gate_baseline, "br", year, budget).await? {
            BudgetVerdict::Skip { record_delta } => {
                println!(
                    "{year}: SKIPPED — record_delta {record_delta} exceeds BACKFILL_BUDGET \
                         {budget}; logged to agents/JOURNAL.md, continuing (nothing blocks)"
                );
                worker::backfill::log_budget_skip(journal_root, "br", year, record_delta, budget)?;
                return Ok(None);
            }
            BudgetVerdict::Proceed { record_delta } => record_delta,
        };
    println!(
        "{year}: budget OK (record_delta {record_delta} <= BACKFILL_BUDGET {budget}) — \
         proceeding to the real write"
    );
    let report = runner
        .run_over(refs)
        .await
        .with_context(|| format!("real write pass for {year}"))?;
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
    Ok(Some(report))
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

    let adapter = BrAdapter::default();
    // NOT wrapped in ScratchDir: this ctx backs the REAL write pass below
    // (Runner::run_over) — its Bronze root is durably referenced via
    // raw_document.storage_uri (invariant 2, raw is sacred), unlike the
    // gate's own scratch Bronze (UfScopedArchive, wrapped) a few lines down.
    let bronze =
        std::env::temp_dir().join(format!("govfolio-backfill-real-br-{}", std::process::id()));
    let ctx = RunCtx::new(
        BronzeStore::open(bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;

    let by_year = discover_by_year(&adapter, &ctx, args.from, args.to, &args.ufs).await?;

    let binding = BrBinding;
    let runner =
        Runner::new(&adapter, &binding, br::seed::regime_binding(), ctx)?.with_backfill(true);

    // BACKFILL_BUDGET gate, scoped to the SAME --uf bound (see module doc
    // comment / UfScopedArchive doc comment for why).
    let budget = worker::backfill::backfill_budget();
    let gate_scratch = std::env::temp_dir().join(format!(
        "govfolio-backfill-real-br-gate-{}",
        std::process::id()
    ));
    let gate_source = UfScopedArchive::new(Some(pool.clone()), gate_scratch, args.ufs.clone())?;
    let gate_baseline = PgBaseline::new(pool.clone(), br::seed::REGIME_ID.to_owned());
    let journal_root = worker::backfill::workspace_root();

    let mut total = RunReport::default();
    for (year, refs) in by_year {
        if let Some(report) = gate_and_write_year(
            &runner,
            &gate_source,
            &gate_baseline,
            &journal_root,
            budget,
            year,
            &refs,
        )
        .await?
        {
            add_report(&mut total, year, report);
        }
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
