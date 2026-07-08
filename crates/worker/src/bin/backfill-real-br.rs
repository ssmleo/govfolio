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
//! 3. **Two `Runner`s, one per `br::seed::RosterBody`** (widened this pass,
//!    `docs/regimes/br/AUTHORITY.md` Quirks log): `pipeline::run::Runner`
//!    resolves politicians against exactly one `RegimeBinding`/body for its
//!    whole lifetime (`crates/pipeline/src/run.rs`), so a SINGLE Runner
//!    cannot correctly resolve a discovered year's candidates when they span
//!    BOTH `br` bodies (Câmara dos Deputados + Senado Federal). This bin
//!    discovers each year exactly ONCE (never a second network fetch,
//!    invariant 10), splits the resulting `FilingRef`s by
//!    `br::seed::roster_body_for_cargo`, then runs each body's refs through
//!    ITS OWN `Runner` (`runner_camara`/`runner_senado`, sharing one
//!    `BrAdapter` instance and Bronze root — see `main`). **Known,
//!    explicitly flagged limitation**: the `BACKFILL_BUDGET` gate below
//!    still estimates `record_delta` from ONE combined dry-run
//!    (`UfScopedArchive`) against ONE `PgBaseline` scoped to the Câmara
//!    `regime_id` only — a previously-published Senado filing (`regime_id`
//!    differs) will never be found by that baseline lookup, so a future
//!    re-run's gate will over-count Senado candidates as "Add" rather than
//!    "Unchanged". This makes the gate MORE conservative (more likely to
//!    SKIP), never less safe — a deliberate, documented tradeoff rather than
//!    a hidden defect, left as a follow-up alongside the actual re-run this
//!    task's own scope explicitly excludes.
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
use pipeline::conformance::workspace_root;
use pipeline::run::{RunReport, Runner};
use pipeline::stages::seed::seed_regime;
use worker::backfill::{
    ArchiveSource, BackfillRunKind, BackfillRunRecord, BackfillRunStatus, BudgetVerdict,
    DiscoveredFiling, PgBaseline, counter_i64, record_backfill_run,
};

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

/// One year's discovered `FilingRef`s, already `--uf`-bounded and split by
/// `br::seed::RosterBody` (module doc comment, point 3) — the shape both
/// `runner_camara.run_over`/`runner_senado.run_over` need.
struct YearFilings {
    year: i32,
    camara: Vec<FilingRef>,
    senado: Vec<FilingRef>,
}

/// Discovers every year in `from..=to`, applying the `--uf` bound AND the
/// Câmara/Senado body split, BEFORE any real write starts (one politeness
/// throttle for the whole discovery pass, invariant 10 — ONE
/// `discover_candidates_year` call per year serves BOTH bodies).
async fn discover_by_year(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    from: i32,
    to: i32,
    ufs: &[String],
) -> anyhow::Result<Vec<YearFilings>> {
    let mut by_year = Vec::new();
    for year in from..=to {
        let candidates = discover_candidates_year(adapter, ctx, year)
            .await
            .with_context(|| format!("discovering {year}"))?;
        let mut camara = Vec::new();
        let mut senado = Vec::new();
        let mut unmapped = 0usize;
        for candidate in candidates {
            if !ufs.is_empty() && !ufs.contains(&candidate.sg_uf) {
                continue;
            }
            match br::seed::roster_body_for_cargo(&candidate.ds_cargo) {
                Some(br::seed::RosterBody::Camara) => camara.push(candidate.filing_ref),
                Some(br::seed::RosterBody::Senado) => senado.push(candidate.filing_ref),
                // Defensive only — every cargo IN_SCOPE_CARGOS admits maps
                // to a RosterBody (see that function's own doc comment).
                None => unmapped += 1,
            }
        }
        println!(
            "{year}: {} Câmara + {} Senado filing(s) in scope (uf filter {ufs:?}){}",
            camara.len(),
            senado.len(),
            if unmapped > 0 {
                format!(" — WARNING: {unmapped} candidate(s) matched no RosterBody")
            } else {
                String::new()
            }
        );
        by_year.push(YearFilings {
            year,
            camara,
            senado,
        });
    }
    Ok(by_year)
}

/// Merges two `RunReport`s (one per body) into one — plain field-wise sums,
/// `failed` messages concatenated (the OUTER `add_report` call still applies
/// the `[{year}]` prefix exactly once, so this must not prefix again).
fn merge_reports(a: RunReport, b: RunReport) -> RunReport {
    RunReport {
        filings: a.filings + b.filings,
        published: a.published + b.published,
        replayed: a.replayed + b.replayed,
        gold_inserted: a.gold_inserted + b.gold_inserted,
        outbox_written: a.outbox_written + b.outbox_written,
        review_tasks: a.review_tasks + b.review_tasks,
        failed: a.failed.into_iter().chain(b.failed).collect(),
    }
}

/// Gates one year against `BACKFILL_BUDGET` (scoped to the same `--uf`
/// bound, see module/`UfScopedArchive` doc comments — the gate's own
/// baseline scoping caveat is flagged in the module doc comment's point 3)
/// then, on [`BudgetVerdict::Proceed`], runs the real write pass over BOTH
/// bodies' refs (`runner_camara`/`runner_senado`, module doc comment point
/// 3). Returns `None` on a budget skip (logged to `agents/JOURNAL.md`,
/// nothing blocks the range). Every arm — `skipped_budget` / `succeeded` /
/// `failed` — records one per-year `backfill_run` row through `pool`
/// (admin observability, plan §Architecture 3); `scope` is the row's
/// `--uf`/nationwide note.
// The observability additions (pool + scope parameters, three per-arm
// `backfill_run` rows) push this otherwise-unchanged gate-then-write
// orchestration past clippy's 7-argument and 100-line bounds — not a
// genuine design regression; bundling/splitting would be pure ceremony for
// a single private call site.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn gate_and_write_year(
    runner_camara: &Runner<'_>,
    runner_senado: &Runner<'_>,
    gate_source: &UfScopedArchive,
    gate_baseline: &PgBaseline,
    journal_root: &std::path::Path,
    budget: usize,
    filings: &YearFilings,
    pool: &sqlx::PgPool,
    scope: Option<&str>,
) -> anyhow::Result<Option<RunReport>> {
    let year = filings.year;
    let started_at = chrono::Utc::now();
    let record_delta =
        match worker::backfill::gate_year(gate_source, gate_baseline, "br", year, budget).await? {
            BudgetVerdict::Skip { record_delta } => {
                println!(
                    "{year}: SKIPPED — record_delta {record_delta} exceeds BACKFILL_BUDGET \
                         {budget}; logged to agents/JOURNAL.md, continuing (nothing blocks)"
                );
                worker::backfill::log_budget_skip(journal_root, "br", year, record_delta, budget)?;
                record_backfill_run(
                    pool,
                    &BackfillRunRecord {
                        scope: scope.map(str::to_owned),
                        record_delta: counter_i64(record_delta),
                        budget: Some(counter_i64(budget)),
                        ..BackfillRunRecord::new(
                            "br",
                            year,
                            BackfillRunKind::Backfill,
                            "backfill-real-br",
                            BackfillRunStatus::SkippedBudget,
                            started_at,
                        )
                    },
                )
                .await?;
                return Ok(None);
            }
            BudgetVerdict::Proceed { record_delta } => record_delta,
        };
    println!(
        "{year}: budget OK (record_delta {record_delta} <= BACKFILL_BUDGET {budget}) — \
         proceeding to the real write"
    );
    let write_result: anyhow::Result<RunReport> = async {
        let camara_report = runner_camara
            .run_over(&filings.camara)
            .await
            .with_context(|| format!("real write pass (Câmara) for {year}"))?;
        let senado_report = runner_senado
            .run_over(&filings.senado)
            .await
            .with_context(|| format!("real write pass (Senado) for {year}"))?;
        Ok(merge_reports(camara_report, senado_report))
    }
    .await;
    let report = match write_result {
        Ok(report) => report,
        Err(error) => {
            // Failed row first, then propagate. The row INSERT itself is
            // best-effort here: it must never mask the year's real error
            // (which is about to abort the run anyway).
            let failed_row = BackfillRunRecord {
                scope: scope.map(str::to_owned),
                record_delta: counter_i64(record_delta),
                budget: Some(counter_i64(budget)),
                error: Some(format!("{error:#}")),
                ..BackfillRunRecord::new(
                    "br",
                    year,
                    BackfillRunKind::Backfill,
                    "backfill-real-br",
                    BackfillRunStatus::Failed,
                    started_at,
                )
            };
            if let Err(record_error) = record_backfill_run(pool, &failed_row).await {
                eprintln!(
                    "WARNING: backfill_run row for failed {year} not recorded: {record_error:#}"
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
        pool,
        &BackfillRunRecord {
            scope: scope.map(str::to_owned),
            record_delta: counter_i64(record_delta),
            budget: Some(counter_i64(budget)),
            ..BackfillRunRecord::new(
                "br",
                year,
                BackfillRunKind::Backfill,
                "backfill-real-br",
                BackfillRunStatus::Succeeded,
                started_at,
            )
            .with_report(&report)
        },
    )
    .await?;
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
    seed_regime(&pool, &br::seed::regime_seed_senado()).await?;

    let adapter = BrAdapter::default();
    // NOT under an OS temp directory, and NOT wrapped in ScratchDir: this ctx
    // backs the REAL write pass below (Runner::run_over) — its Bronze root is
    // durably referenced via raw_document.storage_uri (invariant 2, raw is
    // sacred) and must never live anywhere a generic temp-dir sweep (disk
    // cleanup utility, reboot policy, or an ad-hoc manual cleanup — see
    // agents/JOURNAL.md's 2026-07-07 incident, which deleted exactly this
    // directory family) could reach it. `target/` is gitignored but not
    // OS-temp, and already this project's convention for durable-but-not-committed
    // local state (`bin/local_br.rs`'s `target/bronze-local-br`); a fixed
    // (non-PID) path is correct here since BronzeStore is content-addressed —
    // re-invocations accumulate/reuse the same store rather than leaking a new
    // directory per run. Unlike the gate's own scratch Bronze (UfScopedArchive,
    // wrapped in ScratchDir) a few lines down.
    let bronze = workspace_root()
        .join("target")
        .join("bronze-backfill-real-br");
    // Two RunCtx instances sharing the SAME (content-addressed, safe to open
    // twice) Bronze path and pool clone — one per RosterBody Runner (module
    // doc comment point 3). `BrAdapter::discover_year`'s joined-declaration
    // cache lives on the shared `adapter` instance below, not on either ctx,
    // so discovering once and fetching through either Runner both hit that
    // SAME cache (never a second network round trip).
    let ctx_camara = RunCtx::new(
        BronzeStore::open(bronze.clone())?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;
    let ctx_senado = RunCtx::new(
        BronzeStore::open(bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;

    let by_year = discover_by_year(&adapter, &ctx_camara, args.from, args.to, &args.ufs).await?;

    let binding = BrBinding;
    let runner_camara = Runner::new(&adapter, &binding, br::seed::regime_binding(), ctx_camara)?
        .with_backfill(true);
    let runner_senado = Runner::new(
        &adapter,
        &binding,
        br::seed::regime_binding_senado(),
        ctx_senado,
    )?
    .with_backfill(true);

    // BACKFILL_BUDGET gate, scoped to the SAME --uf bound (see module doc
    // comment / UfScopedArchive doc comment for why). Baseline scoping
    // caveat: module doc comment point 3.
    let budget = worker::backfill::backfill_budget();
    let gate_scratch = std::env::temp_dir().join(format!(
        "govfolio-backfill-real-br-gate-{}",
        std::process::id()
    ));
    let gate_source = UfScopedArchive::new(Some(pool.clone()), gate_scratch, args.ufs.clone())?;
    let gate_baseline = PgBaseline::new(pool.clone(), br::seed::REGIME_ID.to_owned());
    let journal_root = worker::backfill::workspace_root();

    // The scope note every backfill_run row carries (migration 0011's own
    // vocabulary): the --uf bound when set, nationwide otherwise.
    let scope = if args.ufs.is_empty() {
        "nationwide".to_owned()
    } else {
        format!("--uf {}", args.ufs.join(","))
    };

    let mut total = RunReport::default();
    for year_filings in &by_year {
        if let Some(report) = gate_and_write_year(
            &runner_camara,
            &runner_senado,
            &gate_source,
            &gate_baseline,
            &journal_root,
            budget,
            year_filings,
            &pool,
            Some(&scope),
        )
        .await?
        {
            add_report(&mut total, year_filings.year, report);
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
