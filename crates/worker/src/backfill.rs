//! Backfill dry-run + diff report (goal 080, design §5.6): "backfill = the same
//! pipeline pointed at archives." The Clerk publishes one `{YYYY}FD.zip` index
//! per year back to the 2012 STOCK Act era; a backfill re-points `discover` at
//! those historical years and replays `fetch → parse → normalize`.
//!
//! This module is the **dry-run** half only. A dry run:
//! - discovers each archive year (full per-year filing count — no truncation),
//! - dry-processes a BOUNDED per-year sample (`per_year_limit`; the bound is
//!   reported honestly, never applied silently — a full 2012→now backfill is
//!   thousands of PDFs),
//! - classifies each sampled filing against the *current* Gold as one of
//!   add / change / supersession / unchanged (design §5.6's diff report), and
//! - writes NOTHING: no Bronze `raw_document` ledger row, no Silver, no Gold,
//!   no `pipeline_run`. Fetched bytes live only in an ephemeral scratch
//!   `BronzeStore` long enough to feed the deterministic parser; the durable
//!   data plane (Postgres + the prod object store) is read-only here.
//!
//! Reprocessing SUPERSEDES, never mutates (invariant 1): a re-run of an
//! immutable PDF under the *same* extractor produces identical fingerprints
//! (unchanged); an amended filing arrives as a NEW `DocID` (`us_house` regime doc
//! §3.7) and is surfaced as a *supersession* for human review. The **real**
//! (write-to-prod) backfill (`bin/backfill-real.rs`) no longer needs a founder
//! diff-approval + go/no-go: goal 081 Task 4's `BACKFILL_BUDGET` gate (below)
//! is the mechanical guardrail that replaces it, per-year, mirroring
//! `scripts/check-tf-plan.sh`'s numeric-count-vs-env-var-budget shape.

use std::collections::HashSet;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use async_trait::async_trait;

use govfolio_core::domain::fingerprint::fingerprint;
use govfolio_core::domain::gold::GoldCandidate;

/// One filing the archive index lists: its source id and where to fetch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFiling {
    /// Source-native filing id (`filing.external_id`), e.g. a Clerk `DocID`.
    pub external_id: String,
    /// Where a real fetch would retrieve the document.
    pub url: String,
}

/// An archive the dry run reads: discovers a year's filings and dry-processes
/// one into Gold candidates WITHOUT persisting anything. The real
/// [`ClerkArchive`] hits the network (politely, invariant 10); tests inject a
/// fixture-backed source instead.
#[async_trait]
pub trait ArchiveSource: Send + Sync {
    /// Every filing the archive holds for `year` (the full scope — the caller,
    /// not this method, applies the sample bound).
    ///
    /// # Errors
    /// Transport failure or an unparseable historical index (the caller fails
    /// that year closed and continues the range).
    async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>>;

    /// Fetch + parse + normalize ONE filing into Gold candidates. No Bronze
    /// ledger, Silver, or Gold write — bytes live only in scratch.
    ///
    /// # Errors
    /// Fetch/parse/normalize failure (the caller records it per-filing and
    /// continues — fail closed per filing, not per run).
    async fn dry_process(&self, filing: &DiscoveredFiling) -> anyhow::Result<Vec<GoldCandidate>>;
}

/// The current Gold state of one filing, read-only — the baseline a dry run
/// diffs against.
#[derive(Debug, Clone)]
pub struct FilingBaseline {
    /// Existing `filing.id` for this `external_id`.
    pub filing_id: String,
    /// Existing `filing.politician_id` (needed to reproduce record fingerprints).
    pub politician_id: String,
    /// Seeded `disclosure_regime.id`.
    pub regime_id: String,
    /// Fingerprints of the Gold rows already published under this filing.
    pub fingerprints: HashSet<String>,
}

/// Read-only Gold lookup by source id. Returns `None` when the filing has never
/// been published (a real run would ADD it).
#[async_trait]
pub trait GoldBaseline: Send + Sync {
    /// The current Gold state for `external_id`, or `None` if unpublished.
    ///
    /// # Errors
    /// Database failure.
    async fn lookup(&self, external_id: &str) -> anyhow::Result<Option<FilingBaseline>>;
}

/// What a real run WOULD do with one sampled filing (design §5.6 categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilingClass {
    /// New filing (source id absent from Gold), all rows `New` — pure insert.
    Add {
        /// Gold rows that would be inserted.
        records: usize,
    },
    /// New filing carrying an amendment (`us_house` §3.7: amendments are new
    /// `DocID`s). Surfaced for human review — reprocessing supersedes, never
    /// mutates (invariant 1). Today these publish unlinked with a
    /// `ptr_amendment_unlinked` `review_task` pending the §7 supersession machine.
    Supersession {
        /// Amended Gold rows that would be inserted.
        records: usize,
    },
    /// Filing already in Gold, but a reprocess (e.g. an extractor-version bump)
    /// would produce NEW fingerprints — superseding inserts, never mutations.
    Change {
        /// Rows whose fingerprint is not yet in Gold.
        new_records: usize,
    },
    /// Filing already in Gold; a re-run would insert nothing (idempotent
    /// replay — invariant 4). The common case for immutable archived PDFs.
    Unchanged,
}

/// Whether a candidate is an `us_house` PTR amendment (regime doc §3.7). Scoped
/// to `us_house` because it is the only backfill adapter wired today; a new
/// backfill regime extends this signal.
fn is_amendment(candidate: &GoldCandidate) -> bool {
    candidate
        .details
        .get("filing_status_raw")
        .and_then(serde_json::Value::as_str)
        == Some("Amended")
}

/// Reproduces the production `disclosure_record.fingerprint` for each candidate,
/// EXACTLY as the publish stage does (`crates/pipeline/src/stages/publish.rs`):
/// bind the resolved identity triple, then hash `(filing_id, ordinal, canonical
/// content)`. `us_house` has no redaction rule, so the publish-time redaction
/// is a no-op here; a backfill regime WITH active redaction would refine this.
///
/// # Errors
/// A baseline id that is not a valid ULID, or a candidate that will not
/// serialize.
pub fn candidate_fingerprints(
    baseline: &FilingBaseline,
    candidates: &[GoldCandidate],
) -> anyhow::Result<Vec<String>> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let ordinal = u32::try_from(index).context("ordinal overflow")?;
            let mut bound = candidate.clone();
            bound.filing_id = baseline
                .filing_id
                .parse()
                .map_err(|e| anyhow::anyhow!("baseline filing id {:?}: {e}", baseline.filing_id))?;
            bound.politician_id = baseline.politician_id.parse().map_err(|e| {
                anyhow::anyhow!("baseline politician id {:?}: {e}", baseline.politician_id)
            })?;
            bound.regime_id = baseline
                .regime_id
                .parse()
                .map_err(|e| anyhow::anyhow!("baseline regime id {:?}: {e}", baseline.regime_id))?;
            let content = serde_json::to_value(&bound).context("serializing bound candidate")?;
            Ok(fingerprint(&baseline.filing_id, ordinal, &content))
        })
        .collect()
}

/// Classifies one sampled filing against its current Gold baseline.
///
/// # Errors
/// Fingerprint reproduction failure (see [`candidate_fingerprints`]).
pub fn classify(
    baseline: Option<&FilingBaseline>,
    candidates: &[GoldCandidate],
) -> anyhow::Result<FilingClass> {
    match baseline {
        None => {
            let records = candidates.len();
            if candidates.iter().any(is_amendment) {
                Ok(FilingClass::Supersession { records })
            } else {
                Ok(FilingClass::Add { records })
            }
        }
        Some(base) => {
            let fingerprints = candidate_fingerprints(base, candidates)?;
            let new_records = fingerprints
                .iter()
                .filter(|fp| !base.fingerprints.contains(*fp))
                .count();
            if new_records == 0 {
                Ok(FilingClass::Unchanged)
            } else {
                Ok(FilingClass::Change { new_records })
            }
        }
    }
}

/// The diff for one archive year.
#[derive(Debug, Default, Clone)]
pub struct YearDiff {
    /// The archive year.
    pub year: i32,
    /// Full count of filings the year's index holds (NOT the sample size).
    pub discovered: usize,
    /// Filings actually dry-processed this run (`<= per_year_limit`).
    pub sampled: usize,
    /// New filings that would be inserted.
    pub adds: usize,
    /// Amended filings surfaced for supersession review.
    pub supersessions: usize,
    /// Already-published filings whose reprocess would insert new rows.
    pub changes: usize,
    /// Already-published filings a re-run would leave untouched.
    pub unchanged: usize,
    /// Gold rows the adds + supersessions + changes would insert (total delta).
    pub record_delta: usize,
    /// Per-filing dry-process failures (`external_id: error`) — fail closed per
    /// filing, the year continues.
    pub failed: Vec<String>,
    /// Set when the YEAR itself failed (index unreachable / unparseable) — fail
    /// closed per year, the range continues.
    pub error: Option<String>,
}

impl YearDiff {
    fn new(year: i32) -> Self {
        Self {
            year,
            ..Self::default()
        }
    }

    fn record(&mut self, class: FilingClass) {
        match class {
            FilingClass::Add { records } => {
                self.adds += 1;
                self.record_delta += records;
            }
            FilingClass::Supersession { records } => {
                self.supersessions += 1;
                self.record_delta += records;
            }
            FilingClass::Change { new_records } => {
                self.changes += 1;
                self.record_delta += new_records;
            }
            FilingClass::Unchanged => self.unchanged += 1,
        }
    }
}

/// The whole dry-run diff report (design §5.6): what a real backfill WOULD do,
/// with nothing written.
#[derive(Debug, Clone)]
pub struct DiffReport {
    /// First archive year swept.
    pub from: i32,
    /// Last archive year swept (inclusive).
    pub to: i32,
    /// The per-year sample bound: `0` means "discover only, do not sample".
    pub per_year_limit: usize,
    /// Whether classification ran against a real Gold baseline (`false` =
    /// discover-only / no DB: counts are reported, add/change cannot be).
    pub classified: bool,
    /// One entry per year in `from..=to`.
    pub years: Vec<YearDiff>,
}

impl DiffReport {
    /// Total filings discovered across all reachable years (the full scope).
    #[must_use]
    pub fn total_discovered(&self) -> usize {
        self.years.iter().map(|y| y.discovered).sum()
    }

    /// Total filings dry-processed (the sampled subset).
    #[must_use]
    pub fn total_sampled(&self) -> usize {
        self.years.iter().map(|y| y.sampled).sum()
    }

    /// Years whose index could not be reached / parsed (fail-closed per year).
    #[must_use]
    pub fn errored_years(&self) -> Vec<i32> {
        self.years
            .iter()
            .filter(|y| y.error.is_some())
            .map(|y| y.year)
            .collect()
    }

    /// True when EVERY year failed AND nothing was discovered — the "archive
    /// unreachable" honest-degradation signal (no false green).
    #[must_use]
    pub fn archive_unreachable(&self) -> bool {
        !self.years.is_empty()
            && self.years.iter().all(|y| y.error.is_some())
            && self.total_discovered() == 0
    }

    fn total(&self, pick: impl Fn(&YearDiff) -> usize) -> usize {
        self.years.iter().map(pick).sum()
    }
}

impl fmt::Display for DiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "== us_house backfill DRY-RUN — archive years {}..={} ==",
            self.from, self.to
        )?;
        if self.per_year_limit == 0 {
            writeln!(
                f,
                "bound: discover-only (no sampling); no add/change/supersession classification"
            )?;
        } else {
            writeln!(
                f,
                "bound: sample <= {} filing(s)/year (honest cap — the full scope is the \
                 'discovered' column; NOT silently truncated)",
                self.per_year_limit
            )?;
        }
        writeln!(
            f,
            "{:<6} {:>10} {:>8} {:>5} {:>8} {:>13} {:>9} {:>7}",
            "year",
            "discovered",
            "sampled",
            "adds",
            "changes",
            "supersessions",
            "unchanged",
            "failed"
        )?;
        for y in &self.years {
            if let Some(error) = &y.error {
                writeln!(
                    f,
                    "{:<6} {:>10}  YEAR FAILED (closed): {error}",
                    y.year, "-"
                )?;
                continue;
            }
            writeln!(
                f,
                "{:<6} {:>10} {:>8} {:>5} {:>8} {:>13} {:>9} {:>7}",
                y.year,
                y.discovered,
                y.sampled,
                y.adds,
                y.changes,
                y.supersessions,
                y.unchanged,
                y.failed.len()
            )?;
        }
        writeln!(
            f,
            "{:<6} {:>10} {:>8} {:>5} {:>8} {:>13} {:>9} {:>7}",
            "TOTAL",
            self.total_discovered(),
            self.total_sampled(),
            self.total(|y| y.adds),
            self.total(|y| y.changes),
            self.total(|y| y.supersessions),
            self.total(|y| y.unchanged),
            self.total(|y| y.failed.len()),
        )?;
        self.fmt_footer(f)
    }
}

impl DiffReport {
    /// The report footer: classification / unreachable notes, the fail-closed
    /// list (invariant 6 — never silent), and the no-writes reminder.
    fn fmt_footer(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.classified {
            writeln!(
                f,
                "NOTE: no Gold baseline (discover-only / DATABASE_URL unset) — filings were \
                 counted, not classified against Gold."
            )?;
        }
        let errored = self.errored_years();
        if !errored.is_empty() {
            writeln!(
                f,
                "NOTE: {} year(s) failed closed (index unreachable/unparseable): {}",
                errored.len(),
                errored
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        let failed_total: usize = self.total(|y| y.failed.len());
        if failed_total > 0 {
            writeln!(
                f,
                "FAIL-CLOSED filings ({failed_total} — a real run would freeze + review these):"
            )?;
            for y in &self.years {
                for failure in &y.failed {
                    writeln!(f, "  [{}] {failure}", y.year)?;
                }
            }
        }
        write!(
            f,
            "DRY-RUN: 0 rows written to Bronze/Silver/Gold. A real (write-to-prod) run is a \
             HALT — see agents/goals/080-backfill-launch.md '## HALT (human/infra)'."
        )
    }
}

/// Runs the bounded, no-write dry run over `from..=to`. Each year fails closed
/// independently (a broken historical index does not sink the range); each
/// filing fails closed independently (a bad PDF does not sink the year).
/// `per_year_limit == 0` discovers only (no fetch/parse); `> 0` samples up to
/// that many filings per year.
///
/// # Errors
/// A baseline lookup that fails at the database level (infrastructure, not a
/// per-filing data problem) aborts the run.
pub async fn dry_run(
    source: &dyn ArchiveSource,
    baseline: &dyn GoldBaseline,
    from: i32,
    to: i32,
    per_year_limit: usize,
) -> anyhow::Result<DiffReport> {
    let mut report = DiffReport {
        from,
        to,
        per_year_limit,
        classified: per_year_limit > 0,
        years: Vec::new(),
    };
    for year in from..=to {
        let mut year_diff = YearDiff::new(year);
        match source.discover_year(year).await {
            Err(error) => year_diff.error = Some(format!("{error:#}")),
            Ok(filings) => {
                year_diff.discovered = filings.len();
                if per_year_limit > 0 {
                    for filing in filings.iter().take(per_year_limit) {
                        year_diff.sampled += 1;
                        match dry_process_one(source, baseline, filing).await? {
                            Ok(class) => year_diff.record(class),
                            Err(failure) => year_diff.failed.push(failure),
                        }
                    }
                }
            }
        }
        report.years.push(year_diff);
    }
    Ok(report)
}

/// Dry-processes + classifies one filing. Returns `Ok(Err(msg))` for a
/// per-filing (fail-closed) data problem; the outer `Err` is reserved for a
/// baseline DB failure that must abort the whole run.
async fn dry_process_one(
    source: &dyn ArchiveSource,
    baseline: &dyn GoldBaseline,
    filing: &DiscoveredFiling,
) -> anyhow::Result<Result<FilingClass, String>> {
    let candidates = match source.dry_process(filing).await {
        Ok(candidates) => candidates,
        Err(error) => return Ok(Err(format!("{}: {error:#}", filing.external_id))),
    };
    if candidates.is_empty() {
        // Zero candidates never publish silently (invariant 6) — a real run
        // would freeze; the dry run flags it as a per-filing failure.
        return Ok(Err(format!(
            "{}: normalize produced zero candidates — would fail closed (invariant 6)",
            filing.external_id
        )));
    }
    let base = baseline.lookup(&filing.external_id).await?;
    match classify(base.as_ref(), &candidates) {
        Ok(class) => Ok(Ok(class)),
        Err(error) => Ok(Err(format!("{}: {error:#}", filing.external_id))),
    }
}

// ---------------------------------------------------------------------------
// Production wiring: the real Clerk archive + Postgres Gold baseline.
// ---------------------------------------------------------------------------

mod live {
    use std::path::PathBuf;

    use anyhow::Context as _;
    use async_trait::async_trait;
    use sqlx::PgPool;

    use govfolio_core::domain::gold::GoldCandidate;
    use pipeline::adapter::{BronzeStore, Clock, FilingRef, JurisdictionAdapter as _, RunCtx};
    use us_house::UsHouseAdapter;

    use super::{ArchiveSource, DiscoveredFiling, FilingBaseline, GoldBaseline};

    /// The real `us_house` archive: `UsHouseAdapter` over the Clerk's per-year
    /// `{YYYY}FD.zip`, polite (invariant 10 — the adapter's 2 s min-interval,
    /// concurrency 1, identified UA), shared across the whole sweep. Fetched
    /// bytes land in an EPHEMERAL scratch `BronzeStore`, never the durable
    /// Bronze ledger; `pool` is present only so `normalize` runs in unbound
    /// (production-shaped) mode — this source never writes through it.
    pub struct ClerkArchive {
        adapter: UsHouseAdapter,
        ctx: RunCtx,
    }

    impl ClerkArchive {
        /// Wires the archive. `pool` (when `Some`) puts `normalize` in unbound
        /// mode so real filers resolve; `scratch` is the throwaway Bronze dir.
        ///
        /// # Errors
        /// HTTP client / Bronze scratch construction failure.
        pub fn new(pool: Option<PgPool>, scratch: PathBuf) -> anyhow::Result<Self> {
            let adapter = UsHouseAdapter::default();
            let ctx = RunCtx::new(
                BronzeStore::open(scratch)?,
                pool,
                Clock::System,
                &adapter.politeness(),
            )?;
            Ok(Self { adapter, ctx })
        }
    }

    #[async_trait]
    impl ArchiveSource for ClerkArchive {
        async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>> {
            Ok(self
                .adapter
                .discover_year(year, &self.ctx)
                .await?
                .into_iter()
                .map(|r| DiscoveredFiling {
                    external_id: r.external_id,
                    url: r.url,
                })
                .collect())
        }

        async fn dry_process(
            &self,
            filing: &DiscoveredFiling,
        ) -> anyhow::Result<Vec<GoldCandidate>> {
            let filing_ref = FilingRef {
                external_id: filing.external_id.clone(),
                url: filing.url.clone(),
            };
            // fetch → scratch Bronze (NOT the raw_document ledger) → parse →
            // normalize. No pipeline_run, Silver, or Gold write.
            let doc = self.adapter.fetch(&filing_ref, &self.ctx).await?;
            let rows = self.adapter.parse(&doc, &self.ctx).await?;
            self.adapter.normalize(&rows, &self.ctx).await
        }
    }

    /// Read-only Gold baseline over Postgres. Only ever SELECTs.
    pub struct PgBaseline {
        pool: PgPool,
        regime_id: String,
    }

    impl PgBaseline {
        /// Binds a baseline to one regime's Gold.
        #[must_use]
        pub fn new(pool: PgPool, regime_id: String) -> Self {
            Self { pool, regime_id }
        }
    }

    #[async_trait]
    impl GoldBaseline for PgBaseline {
        async fn lookup(&self, external_id: &str) -> anyhow::Result<Option<FilingBaseline>> {
            let filing: Option<(String, String)> = sqlx::query_as(
                "select id, politician_id from filing \
                 where regime_id = $1 and external_id = $2",
            )
            .bind(&self.regime_id)
            .bind(external_id)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("baseline lookup for filing {external_id}"))?;
            let Some((filing_id, politician_id)) = filing else {
                return Ok(None);
            };
            let fingerprints: Vec<String> = sqlx::query_scalar(
                "select fingerprint from disclosure_record where filing_id = $1",
            )
            .bind(&filing_id)
            .fetch_all(&self.pool)
            .await
            .with_context(|| format!("baseline fingerprints for filing {filing_id}"))?;
            Ok(Some(FilingBaseline {
                filing_id,
                politician_id,
                regime_id: self.regime_id.clone(),
                fingerprints: fingerprints.into_iter().collect(),
            }))
        }
    }

    /// A Gold baseline that reports every filing as unpublished — used when no
    /// `DATABASE_URL` is available (discover-only mode). Everything a real run
    /// would touch is an ADD; nothing is classified against real Gold.
    pub struct NoBaseline;

    #[async_trait]
    impl GoldBaseline for NoBaseline {
        async fn lookup(&self, _external_id: &str) -> anyhow::Result<Option<FilingBaseline>> {
            Ok(None)
        }
    }
}

pub use live::{ClerkArchive, NoBaseline, PgBaseline};

// ---------------------------------------------------------------------------
// Goal 081 Task 4: BACKFILL_BUDGET — the mechanical guardrail that replaces
// goal 080's founder go/no-go HALT. Mirrors `scripts/check-tf-plan.sh`'s
// numeric-count-vs-env-var-budget shape, chunked by archive year: no new
// prediction/classification code — just a plain compare against the record
// delta the EXISTING `dry_run` already computes.
// ---------------------------------------------------------------------------

/// Default `BACKFILL_BUDGET` (Gold-row cap per year) — an explicit starting
/// point per goal 080's peak-year finding (2018 ≈ 830 filings/year),
/// widenable later via the env var.
pub const DEFAULT_BACKFILL_BUDGET: usize = 500;

/// Reads `BACKFILL_BUDGET` (default [`DEFAULT_BACKFILL_BUDGET`]) — mirrors
/// `scripts/check-tf-plan.sh`'s `DESTROY_BUDGET` env-var shape.
#[must_use]
pub fn backfill_budget() -> usize {
    std::env::var("BACKFILL_BUDGET")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_BACKFILL_BUDGET)
}

/// This crate's own workspace root, resolved from its manifest (same
/// pattern as `pipeline::conformance::workspace_root`) — used only to locate
/// `agents/JOURNAL.md` for [`log_budget_skip`].
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// One archive year's go/no-go verdict against `BACKFILL_BUDGET`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    /// `record_delta <= budget` — proceed to the real write for this year.
    Proceed {
        /// The dry run's `record_delta` for this year.
        record_delta: usize,
    },
    /// `record_delta > budget` — skip this year; nothing blocks the range, a
    /// later invocation naturally retries it.
    Skip {
        /// The dry run's `record_delta` for this year.
        record_delta: usize,
    },
}

/// The gate itself: a plain numeric compare, mirroring
/// `scripts/check-tf-plan.sh`'s `[ "$DELETES" -gt "$BUDGET" ]`.
#[must_use]
pub fn budget_verdict(record_delta: usize, budget: usize) -> BudgetVerdict {
    if record_delta > budget {
        BudgetVerdict::Skip { record_delta }
    } else {
        BudgetVerdict::Proceed { record_delta }
    }
}

/// Runs the budget gate for ONE archive year: calls the EXISTING [`dry_run`]
/// over `year..=year` with no sample bound (`usize::MAX` — every filing
/// dry-processed) and reads `report.years[0].record_delta` (already
/// computed — no new prediction/classification code), then applies
/// [`budget_verdict`].
///
/// # Errors
/// The underlying `dry_run` call fails (a baseline DB failure — an
/// infrastructure error, not a per-year skip), or it produced no year entry
/// (cannot happen for a `year..=year` sweep; surfaced defensively rather
/// than indexed/unwrapped).
pub async fn gate_year(
    source: &dyn ArchiveSource,
    baseline: &dyn GoldBaseline,
    year: i32,
    budget: usize,
) -> anyhow::Result<BudgetVerdict> {
    let report = dry_run(source, baseline, year, year, usize::MAX).await?;
    let record_delta = report
        .years
        .first()
        .with_context(|| format!("dry_run produced no year entry for {year}"))?
        .record_delta;
    Ok(budget_verdict(record_delta, budget))
}

/// Appends one skip line to `<root>/agents/JOURNAL.md`, matching the
/// existing halt-entry convention: `date | item | outcome | blockers`.
/// Called only on [`BudgetVerdict::Skip`] — nothing blocks the range, so this
/// is a log line, not a `## BLOCKED (human)` halt.
///
/// # Errors
/// The journal file cannot be created/appended (filesystem failure).
pub fn log_budget_skip(
    root: &Path,
    year: i32,
    record_delta: usize,
    budget: usize,
) -> anyhow::Result<()> {
    let agents_dir = root.join("agents");
    std::fs::create_dir_all(&agents_dir)
        .with_context(|| format!("creating {}", agents_dir.display()))?;
    let path = agents_dir.join("JOURNAL.md");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    let date = chrono::Utc::now().date_naive();
    writeln!(
        file,
        "{date} | 081/T4 | BACKFILL_BUDGET skip: us_house {year} record_delta={record_delta} \
         exceeds budget={budget} | none — nothing blocks; a later invocation retries {year}"
    )
    .with_context(|| format!("appending to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A minimal `us_house` PTR candidate (unbound identity — as `normalize`
    /// emits it) with the given filing status.
    fn candidate(status: &str) -> GoldCandidate {
        serde_json::from_value(json!({
            "filing_id": "00000000000000000000000000",
            "politician_id": "00000000000000000000000000",
            "regime_id": "00000000000000000000000000",
            "instrument_id": null,
            "asset_description_raw": "Apple Inc. (AAPL) [ST]",
            "record_type": "transaction",
            "asset_class": "equity",
            "side": "buy",
            "transaction_date": "2026-03-02",
            "as_of_date": null,
            "notified_date": "2026-03-02",
            "value": {"low": "1001.00", "high": "15000.00", "currency": "USD"},
            "owner": "self",
            "extraction_confidence": 0.98,
            "extracted_by": "us_house_ptr/text@1",
            "fingerprint": null,
            "details": {"filing_status_raw": status}
        }))
        .unwrap()
    }

    fn baseline_for(candidates: &[GoldCandidate]) -> FilingBaseline {
        let mut base = FilingBaseline {
            filing_id: "0HSEFNG0000000000020020055".to_owned(),
            politician_id: "0HSEMBR0000000000000000001".to_owned(),
            regime_id: "0HSEREG0000000000000000001".to_owned(),
            fingerprints: HashSet::new(),
        };
        base.fingerprints = candidate_fingerprints(&base, candidates)
            .unwrap()
            .into_iter()
            .collect();
        base
    }

    #[test]
    fn new_filing_all_new_rows_is_an_add() {
        let cands = [candidate("New")];
        assert_eq!(
            classify(None, &cands).unwrap(),
            FilingClass::Add { records: 1 }
        );
    }

    #[test]
    fn new_amended_filing_is_a_supersession() {
        // us_house §3.7: amendments arrive as new DocIDs (baseline None) with an
        // `Amended` row — surfaced for supersession review, never mutation.
        let cands = [candidate("Amended")];
        assert_eq!(
            classify(None, &cands).unwrap(),
            FilingClass::Supersession { records: 1 }
        );
    }

    #[test]
    fn republished_identical_filing_is_unchanged() {
        // Immutable PDF + same extractor => identical fingerprints => replay
        // would insert nothing (invariant 4).
        let cands = [candidate("New")];
        let base = baseline_for(&cands);
        assert_eq!(
            classify(Some(&base), &cands).unwrap(),
            FilingClass::Unchanged
        );
    }

    #[test]
    fn reprocess_with_new_content_is_a_change_not_a_mutation() {
        // Baseline has the OLD fingerprints; a reprocess that changed a value
        // yields a new fingerprint => Change (a superseding insert, invariant 1).
        let old = [candidate("New")];
        let base = baseline_for(&old);
        let mut reprocessed = candidate("New");
        reprocessed.value = serde_json::from_value(
            json!({"low": "15001.00", "high": "50000.00", "currency": "USD"}),
        )
        .unwrap();
        assert_eq!(
            classify(Some(&base), &[reprocessed]).unwrap(),
            FilingClass::Change { new_records: 1 }
        );
    }

    // --- source/baseline fakes for the orchestration tests (no DB, no net) ---

    struct FakeArchive {
        /// year -> filings; a year absent from the map fails closed.
        by_year: std::collections::BTreeMap<i32, Vec<DiscoveredFiling>>,
        /// `external_id` -> candidates a dry-process would yield.
        candidates: std::collections::BTreeMap<String, Vec<GoldCandidate>>,
    }

    #[async_trait]
    impl ArchiveSource for FakeArchive {
        async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>> {
            self.by_year
                .get(&year)
                .cloned()
                .with_context(|| format!("historical index for {year} unreachable/unparseable"))
        }
        async fn dry_process(
            &self,
            filing: &DiscoveredFiling,
        ) -> anyhow::Result<Vec<GoldCandidate>> {
            Ok(self
                .candidates
                .get(&filing.external_id)
                .cloned()
                .unwrap_or_default())
        }
    }

    fn filings(n: usize) -> Vec<DiscoveredFiling> {
        (0..n)
            .map(|i| DiscoveredFiling {
                external_id: format!("doc{i}"),
                url: format!("file://doc{i}"),
            })
            .collect()
    }

    #[tokio::test]
    async fn bounded_sample_reports_full_scope_and_the_honest_cap() {
        let mut candidates = std::collections::BTreeMap::new();
        for f in filings(10) {
            candidates.insert(f.external_id, vec![candidate("New")]);
        }
        let source = FakeArchive {
            by_year: [(2012, filings(10))].into_iter().collect(),
            candidates,
        };
        let report = dry_run(&source, &NoBaseline, 2012, 2012, 3).await.unwrap();
        let year = &report.years[0];
        assert_eq!(year.discovered, 10, "full scope reported, not truncated");
        assert_eq!(year.sampled, 3, "bounded to the honest cap");
        assert_eq!(year.adds, 3);
        assert!(!report.archive_unreachable());
    }

    #[tokio::test]
    async fn per_year_index_failure_fails_closed_without_sinking_the_range() {
        // 2012 present, 2013 missing (unparseable historical index), 2014 present.
        let mut candidates = std::collections::BTreeMap::new();
        for f in filings(1) {
            candidates.insert(f.external_id, vec![candidate("New")]);
        }
        let source = FakeArchive {
            by_year: [(2012, filings(1)), (2014, filings(1))]
                .into_iter()
                .collect(),
            candidates,
        };
        let report = dry_run(&source, &NoBaseline, 2012, 2014, 5).await.unwrap();
        assert_eq!(report.errored_years(), vec![2013], "only 2013 fails closed");
        assert_eq!(report.years[0].adds, 1, "2012 still processed");
        assert_eq!(report.years[2].adds, 1, "2014 still processed");
        assert!(report.years[1].error.is_some());
    }

    #[tokio::test]
    async fn all_years_unreachable_is_the_honest_degradation_signal() {
        let source = FakeArchive {
            by_year: std::collections::BTreeMap::new(),
            candidates: std::collections::BTreeMap::new(),
        };
        let report = dry_run(&source, &NoBaseline, 2012, 2013, 5).await.unwrap();
        assert!(
            report.archive_unreachable(),
            "no year reachable, nothing discovered => unreachable (no false green)"
        );
        assert_eq!(report.total_discovered(), 0);
    }

    #[tokio::test]
    async fn zero_candidate_filing_is_a_per_filing_failure_not_a_crash() {
        let source = FakeArchive {
            by_year: [(2012, filings(1))].into_iter().collect(),
            candidates: std::collections::BTreeMap::new(), // doc0 -> zero candidates
        };
        let report = dry_run(&source, &NoBaseline, 2012, 2012, 5).await.unwrap();
        assert_eq!(
            report.years[0].failed.len(),
            1,
            "zero-row filing fails closed"
        );
        assert_eq!(report.years[0].adds, 0);
    }
}
