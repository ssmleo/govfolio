//! The in-process pipeline runner (plan Task 9): chains
//! `discover → fetch → parse → normalize(+resolve) → publish` against one
//! [`JurisdictionAdapter`] (design §5.2). Queue semantics are emulated
//! in-process: every stage unit is claimed in `pipeline_run` under a
//! deterministic idempotency key, every write is `ON CONFLICT DO NOTHING`
//! (invariant 4), and a re-run of identical inputs inserts NOTHING.
//!
//! The §5.1 adapter trait stays untouched: adapter-specific runner glue
//! (silver staging shape, filing identity, regime review rules) lives behind
//! [`RunnerBinding`], which each adapter crate implements beside its adapter.

use std::path::PathBuf;

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;

use govfolio_core::domain::gold::GoldCandidate;

use crate::adapter::{FilingRef, JurisdictionAdapter, RawDocRef, RunCtx, StagingRow};
use crate::stages::pipeline_run::{Claim, claim, finish_failed, finish_ok};
use crate::stages::publish::{FilingSpec, PublishStats, publish_filing};
use crate::stages::{ingest, roster};

/// Binds a runner to its seeded `disclosure_regime` row (adapter constants).
#[derive(Debug, Clone)]
pub struct RegimeBinding {
    /// Seeded `disclosure_regime.id`.
    pub regime_id: String,
    /// Seeded `jurisdiction.id`.
    pub jurisdiction_id: String,
    /// Body string mandates are scoped to (roster resolution key).
    pub body: String,
}

/// Filing identity derived from a document's own silver rows (design §5.4:
/// filings name their filer). In live mode this is cross-checked against the
/// index-derived [`FilingRef`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilingIdentity {
    /// Source-native filing id (`filing.external_id` dedup key).
    pub external_id: String,
    /// Filer name exactly as filed.
    pub filer_name: String,
    /// District code as filed.
    pub district: String,
    /// Source filing-type vocabulary (e.g. `P`).
    pub filing_type: String,
    /// Filer-claimed filing date, when the document carries one.
    pub filed_date: Option<NaiveDate>,
}

/// One staged Silver row (the surviving `stg_<regime>` row id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSilver {
    /// `stg_<regime>.id` — existing row on replay, new row otherwise.
    pub stg_id: String,
}

/// Adapter-specific runner glue. The §5.1 adapter contract never grows when
/// coverage grows; this trait is runner machinery, shipped beside the adapter.
#[async_trait]
pub trait RunnerBinding: Send + Sync {
    /// The adapter's Silver staging table name (`stg_<regime>`).
    fn silver_table(&self) -> &'static str;

    /// Extracts the filing identity from this adapter's silver payloads.
    ///
    /// # Errors
    /// Rows that disagree on identity or an unreadable payload (fail closed).
    fn filing_identity(&self, rows: &[StagingRow]) -> anyhow::Result<FilingIdentity>;

    /// Stages silver rows into `stg_<regime>` (idempotent: unique on
    /// `(raw_document_id, row_ordinal)`, `ON CONFLICT DO NOTHING`) and returns
    /// the surviving row ids in document order.
    ///
    /// # Errors
    /// Database failure or a payload that is not this adapter's silver shape.
    async fn stage_silver(
        &self,
        pool: &PgPool,
        raw_document_id: &str,
        rows: &[StagingRow],
    ) -> anyhow::Result<Vec<StagedSilver>>;

    /// Loads previously staged silver rows (stage replay reads its input from
    /// the previous stage's store).
    ///
    /// # Errors
    /// Database failure.
    async fn load_silver(
        &self,
        pool: &PgPool,
        raw_document_id: &str,
    ) -> anyhow::Result<Vec<StagingRow>>;

    /// Regime-specific §7 publish-time review reasons for one candidate
    /// (e.g. `ptr_amendment_unlinked` for `FILING STATUS: Amended` rows).
    fn review_reasons(&self, candidate: &GoldCandidate) -> Vec<String>;
}

/// One offline input: a local document file (fixture `input.pdf`). Identity is
/// derived from the parsed document itself — no network, no index.
#[derive(Debug, Clone)]
pub struct LocalFiling {
    /// Path of the raw document to ingest.
    pub path: PathBuf,
}

/// What one run did — aggregated from the per-stage stats.
#[derive(Debug, Default)]
pub struct RunReport {
    /// Filings processed (attempted).
    pub filings: usize,
    /// Filings whose publish stage executed this run.
    pub published: usize,
    /// Filings skipped because their publish already succeeded (replay).
    pub replayed: usize,
    /// Gold rows inserted this run.
    pub gold_inserted: u64,
    /// Outbox events written this run (same-txn with the Gold rows).
    pub outbox_written: u64,
    /// Review tasks opened this run.
    pub review_tasks: u64,
    /// Per-filing failures (`external id or path: error`); the run continues
    /// past them — fail closed per filing, not per run.
    pub failed: Vec<String>,
}

/// The in-process runner. Construct with a pool-backed [`RunCtx`].
pub struct Runner<'a> {
    adapter: &'a dyn JurisdictionAdapter,
    binding: &'a dyn RunnerBinding,
    regime: RegimeBinding,
    ctx: RunCtx,
    pool: PgPool,
}

impl<'a> Runner<'a> {
    /// Wires a runner. The context MUST carry a pool — the runner exists to
    /// write Bronze/Silver/Gold.
    ///
    /// # Errors
    /// A pool-less context.
    pub fn new(
        adapter: &'a dyn JurisdictionAdapter,
        binding: &'a dyn RunnerBinding,
        regime: RegimeBinding,
        ctx: RunCtx,
    ) -> anyhow::Result<Self> {
        let pool = ctx
            .pool
            .clone()
            .context("Runner requires a pool-backed RunCtx")?;
        Ok(Self {
            adapter,
            binding,
            regime,
            ctx,
            pool,
        })
    }

    /// Runs the pipeline over local files (offline mode: fixtures, backfill
    /// from disk). No network is touched.
    ///
    /// # Errors
    /// Infrastructure failure (I/O on inputs, database unavailability).
    /// Per-filing processing failures land in [`RunReport::failed`] instead —
    /// one bad filing must not sink the batch.
    pub async fn run_local(&self, inputs: &[LocalFiling]) -> anyhow::Result<RunReport> {
        let mut report = RunReport::default();
        for filing in inputs {
            report.filings += 1;
            if let Err(e) = self.process_local(filing, &mut report).await {
                report
                    .failed
                    .push(format!("{}: {e:#}", filing.path.display()));
            }
        }
        Ok(report)
    }

    /// Runs the full live chain: `discover` then per-filing processing.
    ///
    /// # Errors
    /// Discovery failure; per-filing failures land in [`RunReport::failed`].
    pub async fn run_live(&self) -> anyhow::Result<RunReport> {
        let refs = self.adapter.discover(&self.ctx).await.context("discover")?;
        let mut report = RunReport::default();
        for filing_ref in &refs {
            report.filings += 1;
            if let Err(e) = self.process_remote(filing_ref, &mut report).await {
                report
                    .failed
                    .push(format!("{}: {e:#}", filing_ref.external_id));
            }
        }
        Ok(report)
    }

    /// Offline fetch: read the file, Bronze it, record the fetch stage keyed
    /// by content address (the external id is only known after parse here).
    async fn process_local(
        &self,
        filing: &LocalFiling,
        report: &mut RunReport,
    ) -> anyhow::Result<()> {
        let code = self.adapter.regime().code;
        let bytes = std::fs::read(&filing.path)
            .with_context(|| format!("reading {}", filing.path.display()))?;
        let doc = self.ctx.bronze.put(&bytes)?;
        let source = filing.path.display().to_string();
        let fetch_key = format!("{code}:fetch:{}", doc.sha256);
        let raw_document_id = self
            .fetch_bookkeeping(&fetch_key, &doc, &bytes, &source)
            .await?;
        self.process_document(&doc, &raw_document_id, None, report)
            .await
    }

    /// Live fetch: claim by external id (skippable via recorded sha), download
    /// through the adapter (politeness-wrapped), then the shared tail.
    async fn process_remote(
        &self,
        filing_ref: &FilingRef,
        report: &mut RunReport,
    ) -> anyhow::Result<()> {
        let code = self.adapter.regime().code;
        let fetch_key = format!("{code}:fetch:{}", filing_ref.external_id);
        let (doc, raw_document_id) = match claim(&self.pool, code, "fetch", &fetch_key).await? {
            Claim::Replay { stats } => {
                // PDFs are immutable per regime doc §2.4: never re-fetch a
                // stored sha. The recorded stats point back at the document.
                let sha256 = stats
                    .get("sha256")
                    .and_then(|v| v.as_str())
                    .context("fetch replay lacks a recorded sha256 — cannot skip safely")?
                    .to_owned();
                let doc = RawDocRef { sha256 };
                let id = sqlx::query_scalar("select id from raw_document where sha256 = $1")
                    .bind(&doc.sha256)
                    .fetch_one(&self.pool)
                    .await
                    .context("fetch replay: raw_document row missing")?;
                (doc, id)
            }
            Claim::New { run_id } | Claim::Retry { run_id } => {
                let fetched = self.fetch_remote(filing_ref, &run_id).await;
                match fetched {
                    Ok(ok) => ok,
                    Err(e) => {
                        finish_failed(&self.pool, &run_id, &format!("{e:#}")).await?;
                        return Err(e);
                    }
                }
            }
        };
        self.process_document(
            &doc,
            &raw_document_id,
            Some(&filing_ref.external_id),
            report,
        )
        .await
    }

    async fn fetch_remote(
        &self,
        filing_ref: &FilingRef,
        run_id: &str,
    ) -> anyhow::Result<(RawDocRef, String)> {
        let doc = self.adapter.fetch(filing_ref, &self.ctx).await?;
        let raw_document_id = ingest::ensure_raw_document(
            &self.pool,
            &doc,
            &self.storage_uri(&doc),
            "application/pdf",
            Some(&filing_ref.url),
            self.ctx.clock.now(),
            Some(run_id),
        )
        .await?;
        finish_ok(
            &self.pool,
            run_id,
            json!({ "sha256": doc.sha256, "raw_document_id": raw_document_id }),
        )
        .await?;
        Ok((doc, raw_document_id))
    }

    /// Records the (already executed, content-addressed) local fetch and
    /// ensures the `raw_document` row. Self-healing: the ensure runs on
    /// replays too, and inserts nothing when the row exists.
    async fn fetch_bookkeeping(
        &self,
        fetch_key: &str,
        doc: &RawDocRef,
        bytes: &[u8],
        source: &str,
    ) -> anyhow::Result<String> {
        let code = self.adapter.regime().code;
        let ensure = |run_id: Option<String>| async move {
            ingest::ensure_raw_document(
                &self.pool,
                doc,
                &self.storage_uri(doc),
                ingest::sniff_mime(bytes),
                Some(source),
                self.ctx.clock.now(),
                run_id.as_deref(),
            )
            .await
        };
        match claim(&self.pool, code, "fetch", fetch_key).await? {
            Claim::Replay { .. } => ensure(None).await,
            Claim::New { run_id } | Claim::Retry { run_id } => {
                match ensure(Some(run_id.clone())).await {
                    Ok(id) => {
                        finish_ok(
                            &self.pool,
                            &run_id,
                            json!({
                                "sha256": doc.sha256,
                                "raw_document_id": id,
                                "bytes": bytes.len(),
                            }),
                        )
                        .await?;
                        Ok(id)
                    }
                    Err(e) => {
                        finish_failed(&self.pool, &run_id, &format!("{e:#}")).await?;
                        Err(e)
                    }
                }
            }
        }
    }

    /// Shared tail: parse (staged, replayable) then publish (normalize +
    /// resolve + transactional Gold/outbox/review write).
    async fn process_document(
        &self,
        doc: &RawDocRef,
        raw_document_id: &str,
        expect_external: Option<&str>,
        report: &mut RunReport,
    ) -> anyhow::Result<()> {
        let code = self.adapter.regime().code;
        let rows = self.parse_stage(doc, raw_document_id).await?;

        let publish_key = format!("{code}:publish:{}", doc.sha256);
        match claim(&self.pool, code, "publish", &publish_key).await? {
            Claim::Replay { .. } => {
                report.replayed += 1;
                Ok(())
            }
            Claim::New { run_id } | Claim::Retry { run_id } => {
                let outcome = self
                    .publish_document(&rows, raw_document_id, expect_external)
                    .await;
                match outcome {
                    Ok(stats) => {
                        report.published += 1;
                        report.gold_inserted += stats.gold_inserted;
                        report.outbox_written += stats.outbox_written;
                        report.review_tasks += stats.review_tasks;
                        let stats_json =
                            serde_json::to_value(&stats).context("serializing publish stats")?;
                        finish_ok(&self.pool, &run_id, stats_json).await?;
                        Ok(())
                    }
                    Err(e) => {
                        finish_failed(&self.pool, &run_id, &format!("{e:#}")).await?;
                        Err(e)
                    }
                }
            }
        }
    }

    /// Parse stage: Bronze → Silver staging + `stg_meta` run linkage. Replays
    /// load their rows back from the staging table.
    async fn parse_stage(
        &self,
        doc: &RawDocRef,
        raw_document_id: &str,
    ) -> anyhow::Result<Vec<StagingRow>> {
        let code = self.adapter.regime().code;
        let parse_key = format!("{code}:parse:{}", doc.sha256);
        match claim(&self.pool, code, "parse", &parse_key).await? {
            Claim::Replay { .. } => {
                let rows = self
                    .binding
                    .load_silver(&self.pool, raw_document_id)
                    .await?;
                anyhow::ensure!(
                    !rows.is_empty(),
                    "parse recorded as succeeded but no silver rows staged for {} — fail closed",
                    doc.sha256
                );
                Ok(rows)
            }
            Claim::New { run_id } | Claim::Retry { run_id } => {
                let staged = self.parse_and_stage(doc, raw_document_id, &run_id).await;
                match staged {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        finish_failed(&self.pool, &run_id, &format!("{e:#}")).await?;
                        Err(e)
                    }
                }
            }
        }
    }

    async fn parse_and_stage(
        &self,
        doc: &RawDocRef,
        raw_document_id: &str,
        run_id: &str,
    ) -> anyhow::Result<Vec<StagingRow>> {
        let rows = self.adapter.parse(doc, &self.ctx).await?;
        // Zero rows never publish silently (invariant 6).
        anyhow::ensure!(
            !rows.is_empty(),
            "parse produced zero rows for {} — fail closed (invariant 6)",
            doc.sha256
        );
        let staged = self
            .binding
            .stage_silver(&self.pool, raw_document_id, &rows)
            .await?;
        ingest::link_stg_meta(
            &self.pool,
            self.binding.silver_table(),
            &staged,
            raw_document_id,
            run_id,
        )
        .await?;
        finish_ok(
            &self.pool,
            run_id,
            json!({ "rows": rows.len(), "staged": staged.len() }),
        )
        .await?;
        Ok(rows)
    }

    /// normalize + resolve + publish (design §5.2's `normalize+resolve →
    /// publish`, collapsed in-process because candidates only live in memory).
    async fn publish_document(
        &self,
        rows: &[StagingRow],
        raw_document_id: &str,
        expect_external: Option<&str>,
    ) -> anyhow::Result<PublishStats> {
        let code = self.adapter.regime().code;
        let identity = self.binding.filing_identity(rows)?;
        if let Some(expected) = expect_external {
            // Live-mode drift guard: the document must be the filing the
            // index said it was.
            anyhow::ensure!(
                identity.external_id == expected,
                "external_id mismatch: index says {expected}, document says {} — fail closed",
                identity.external_id
            );
        }

        // Politician resolution: high precision or nothing (design §5.4).
        let resolved = roster::resolve_politician(
            &self.pool,
            &self.regime,
            &identity.filer_name,
            &identity.district,
        )
        .await?;
        let Some(politician_id) = resolved else {
            let target = format!("{code}:{}", identity.external_id);
            roster::open_review_task_once(&self.pool, "filing", &target, "unresolved_filer")
                .await?;
            anyhow::bail!(
                "unresolved filer {:?} ({}) for filing {} — review_task opened, \
                 no Gold row (invariant 3: never guess)",
                identity.filer_name,
                identity.district,
                identity.external_id
            );
        };

        let candidates = self.adapter.normalize(rows, &self.ctx).await?;
        anyhow::ensure!(
            !candidates.is_empty(),
            "normalize produced zero candidates for filing {} — fail closed (invariant 6)",
            identity.external_id
        );
        let spec = FilingSpec {
            regime_id: &self.regime.regime_id,
            regime_code: code,
            politician_id: &politician_id,
            raw_document_id,
            identity: &identity,
            discovered_at: self.ctx.clock.now(),
        };
        publish_filing(&self.pool, &spec, &candidates, &|candidate| {
            self.binding.review_reasons(candidate)
        })
        .await
    }

    /// Local Bronze address recorded on `raw_document.storage_uri` (object
    /// storage arrives later behind the same shape).
    fn storage_uri(&self, doc: &RawDocRef) -> String {
        format!("file://{}", self.ctx.bronze.path(doc).display())
    }
}
