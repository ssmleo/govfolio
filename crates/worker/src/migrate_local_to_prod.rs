//! Local -> prod Gold migration (per founder-directed policy, 2026-07-09
//! session direction: historical backfills must run only against local dev
//! Postgres; prod receives data via migration of the already-collected
//! local dataset, not by re-running the backfill pipeline against prod —
//! pending write-back into a future root `CLAUDE.md` invariant). Supersedes
//! goal 081 Task 5c's original "run `backfill-real` directly against prod"
//! text — that approach is now forbidden.
//!
//! [`migrate_regime`] copies one regime's already-collected LOCAL Gold
//! dataset into PROD. Every row keeps its LOCAL id verbatim (ULIDs are
//! globally unique already), so cross-table FKs stay valid across the two
//! databases without any id remapping. PURE INSERT (invariant 1 — Gold is
//! immutable, supersede-never-update): every write is `ON CONFLICT (id) DO
//! NOTHING` (invariant 4); this module never issues an `UPDATE` against an
//! existing prod row. A single row's failure is caught, counted, and logged
//! — never sinks the rest of the migration (invariant 6, mirroring
//! `bin/backfill-real.rs`'s own per-filing fail-closed precedent).
//! [`migrate_regime`] itself only returns `Err` for a genuine connection/
//! setup failure (an unknown `--regime`, or the DB being unreachable).
//!
//! # Migration order (design §4.2 FK dependencies)
//! `jurisdiction`/`disclosure_regime` (via the already-idempotent
//! [`seed_regime`]) -> `politician`/`politician_alias`/`mandate` (see
//! [`migrate_politicians`] for scoping) -> `raw_document` (DB row + physical
//! GCS upload via [`BronzeUploader`], `storage_uri` rewritten to `gs://`) ->
//! `filing` -> `disclosure_record` -> `outbox_event` (`dispatched_at` copied
//! EXACTLY as-is, never `NULL`ed, so migration can never cause a real alert)
//! -> `review_task` (scoped to the `filing`/`disclosure_record` rows just
//! migrated) -> `review_audit` (FK `review_task_id references review_task
//! (id)`, so it must land after; scoped to the `review_task` rows just
//! migrated).
//!
//! `filing.supersedes_filing_id` and `disclosure_record.supersedes_record_id`
//! are self-referential FKs. No adapter/pipeline code populates either
//! column today (confirmed: only ever set by the one-off
//! `bin/fix-br-julio-cesar-santos-ba-2018.rs` via `UPDATE`, never by
//! `publish`/`promote`'s normal `INSERT` paths) — a superseding row, if one
//! ever exists, can only have been discovered/created strictly after the
//! row it supersedes. So a single ordered pass — `filing` by `discovered_at`
//! ascending, `disclosure_record` by `created_at` ascending — guarantees the
//! target already exists in PROD by the time the superseding row is
//! attempted. A two-pass insert-then-link approach would be strictly more
//! machinery for a column that is, today, always NULL; if the ordering
//! assumption is ever violated the single row's INSERT fails its FK check
//! and is caught by the normal per-row fail-closed path, not silently wrong.
//!
//! Deliberately NOT migrated: `pipeline_run` (local execution bookkeeping,
//! not business data — this migration path never goes through `Runner`
//! claims in prod) and `instrument`/`instrument_alias` (every adapter today
//! emits `instrument_id: None` — invariant 3, never guess — so nothing has
//! ever populated it; a `disclosure_record` row that DID carry a
//! non-null `instrument_id` would fail its own FK insert into PROD, caught
//! and counted like any other row failure).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::PgPool;

use pipeline::adapter::{BronzeStore, RawDocRef};
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed, seed_regime};

// --------------------------------------------------------------------- //
// Bronze upload seam (invariant 2 — raw is sacred).
// --------------------------------------------------------------------- //

/// Physically lands Bronze bytes in the prod object store. A trait so
/// [`migrate_regime`]'s row-copy/idempotency logic is fully testable without
/// touching real GCS/gcloud: tests inject a fake; [`GcloudUploader`] is the
/// real implementation the `migrate-local-to-prod` bin wires up.
pub trait BronzeUploader: Send + Sync {
    /// Ensures the content-addressed object for `sha256` exists, uploading
    /// `local_path`'s bytes if it doesn't. An existing object is left
    /// untouched and NOT re-verified (content-addressed: byte-identical by
    /// construction) — hash verification happens only on a genuine first
    /// upload.
    ///
    /// # Errors
    /// Upload failure, or the local file's own sha256 not matching `sha256`
    /// (a corrupted/mismatched local Bronze file — refuses to publish it).
    fn ensure_uploaded(&self, sha256: &str, local_path: &Path) -> anyhow::Result<UploadOutcome>;
}

/// Outcome of [`BronzeUploader::ensure_uploaded`] — always carries the
/// resulting `gs://` URI, whichever branch was taken.
#[derive(Debug, Clone)]
pub enum UploadOutcome {
    /// Not present before this call; uploaded now.
    Uploaded(String),
    /// Already present at the content-addressed path (idempotent rerun) —
    /// nothing uploaded.
    AlreadyPresent(String),
}

impl UploadOutcome {
    /// The resulting `gs://` URI, whichever branch was taken.
    #[must_use]
    pub fn uri(&self) -> &str {
        match self {
            Self::Uploaded(uri) | Self::AlreadyPresent(uri) => uri,
        }
    }
}

/// Real uploader: shells out to `gcloud storage` — per-file volumes here are
/// in the hundreds, not millions, and no GCS client crate is already in the
/// workspace dependency graph (checked `Cargo.lock`), so a direct,
/// scoped shell-out is the correct scope, not a new heavyweight cloud-SDK
/// dependency.
pub struct GcloudUploader {
    /// Target bucket name (no `gs://` prefix), e.g. `govfolio-bronze`.
    pub bucket: String,
}

impl BronzeUploader for GcloudUploader {
    fn ensure_uploaded(&self, sha256: &str, local_path: &Path) -> anyhow::Result<UploadOutcome> {
        let uri = format!("gs://{}/{sha256}", self.bucket);
        let exists = Command::new("gcloud")
            .args(["storage", "ls", &uri])
            .output()
            .context("running `gcloud storage ls`")?;
        if exists.status.success() {
            return Ok(UploadOutcome::AlreadyPresent(uri));
        }
        let bytes = std::fs::read(local_path)
            .with_context(|| format!("reading LOCAL bronze file {}", local_path.display()))?;
        let computed = sha256_hex(&bytes);
        anyhow::ensure!(
            computed == sha256,
            "LOCAL bronze file {} hashes to {computed}, expected {sha256} — refusing to \
             upload a mismatched document (invariant 2: raw is sacred)",
            local_path.display()
        );
        let status = Command::new("gcloud")
            .args(["storage", "cp", &local_path.display().to_string(), &uri])
            .status()
            .context("running `gcloud storage cp`")?;
        anyhow::ensure!(status.success(), "`gcloud storage cp` to {uri} failed");
        Ok(UploadOutcome::Uploaded(uri))
    }
}

/// sha256 of `bytes`, lowercase hex — the same content address
/// [`pipeline::adapter::BronzeStore`] uses internally (that helper is
/// private to `pipeline`; this is a small, deliberate, same-shape
/// recomputation for the one pre-upload verification this module needs, not
/// a cross-crate API addition for a single caller).
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex::encode(Sha256::digest(bytes))
}

// --------------------------------------------------------------------- //
// Report.
// --------------------------------------------------------------------- //

/// One table's migration tally.
#[derive(Debug, Default, Clone, Copy)]
pub struct TableCounts {
    /// Rows newly inserted into PROD this run.
    pub migrated: u64,
    /// Rows already present in PROD (idempotent rerun — expected, not an
    /// error).
    pub already_present: u64,
    /// Rows that failed to migrate (logged individually to stderr; never
    /// sinks the rest of the batch — invariant 6).
    pub failed: u64,
}

/// Full migration tally, one field per migrated table (design §4.2 order).
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// `politician` rows.
    pub politician: TableCounts,
    /// `politician_alias` rows.
    pub politician_alias: TableCounts,
    /// `mandate` rows.
    pub mandate: TableCounts,
    /// `raw_document` rows (the DB pointer; see the two GCS-side counters
    /// below for the physical upload).
    pub raw_document: TableCounts,
    /// Bronze objects physically uploaded to GCS this run.
    pub raw_document_uploaded: u64,
    /// Bronze objects already present in GCS (idempotent rerun).
    pub raw_document_gcs_already_present: u64,
    /// `filing` rows.
    pub filing: TableCounts,
    /// `disclosure_record` rows.
    pub disclosure_record: TableCounts,
    /// `outbox_event` rows.
    pub outbox_event: TableCounts,
    /// `review_task` rows.
    pub review_task: TableCounts,
    /// `review_audit` rows.
    pub review_audit: TableCounts,
}

impl MigrationReport {
    fn line(name: &str, counts: TableCounts) {
        println!(
            "{name:<18} migrated {:>6} | already present {:>6} | failed {:>4}",
            counts.migrated, counts.already_present, counts.failed
        );
    }

    /// Prints one line per table plus a final summary. Exit code is decided
    /// by the caller (`migrate-local-to-prod` bin exits 0 regardless of
    /// `failed` counts here — only a connection/setup failure is nonzero).
    pub fn print(&self) {
        Self::line("politician", self.politician);
        Self::line("politician_alias", self.politician_alias);
        Self::line("mandate", self.mandate);
        Self::line("raw_document", self.raw_document);
        println!(
            "{:<18} uploaded {:>6} | already in GCS {:>6}",
            "raw_document GCS", self.raw_document_uploaded, self.raw_document_gcs_already_present
        );
        Self::line("filing", self.filing);
        Self::line("disclosure_record", self.disclosure_record);
        Self::line("outbox_event", self.outbox_event);
        Self::line("review_task", self.review_task);
        Self::line("review_audit", self.review_audit);
        let total_failed = self.politician.failed
            + self.politician_alias.failed
            + self.mandate.failed
            + self.raw_document.failed
            + self.filing.failed
            + self.disclosure_record.failed
            + self.outbox_event.failed
            + self.review_task.failed
            + self.review_audit.failed;
        println!("TOTAL failed rows: {total_failed} (see FAILED lines above for detail)");
    }
}

// --------------------------------------------------------------------- //
// Orchestrator.
// --------------------------------------------------------------------- //

/// Migrates one regime's already-collected LOCAL Gold dataset into PROD.
/// `regime_id` is a LOCAL `disclosure_regime.id` (e.g.
/// `us_house::seed::REGIME_ID`) — NOT an adapter code. Safe to re-run: every
/// write is `ON CONFLICT (id) DO NOTHING`, so a second invocation after
/// LOCAL captures more of a regime's range transfers only the new rows.
///
/// # Errors
/// A genuine connection/setup failure: PROD unreachable/unmigratable, or
/// `regime_id` not found in LOCAL. Per-row failures during the migration
/// itself are caught, counted in the returned [`MigrationReport`], and never
/// surface here (invariant 6).
pub async fn migrate_regime(
    local: &PgPool,
    prod: &PgPool,
    regime_id: &str,
    bronze_root: &Path,
    uploader: &dyn BronzeUploader,
) -> anyhow::Result<MigrationReport> {
    govfolio_core::db::migrate(prod)
        .await
        .context("applying migrations to PROD")?;
    let seed = fetch_local_regime_seed(local, regime_id).await?;
    seed_regime(prod, &seed)
        .await
        .context("seeding regime/jurisdiction into PROD")?;

    let bronze =
        BronzeStore::open(bronze_root.to_path_buf()).context("opening LOCAL bronze store")?;

    let mut report = MigrationReport::default();
    migrate_politicians(local, prod, regime_id, &seed.jurisdiction.id, &mut report).await?;
    let filing_ids =
        migrate_filings(local, prod, regime_id, &bronze, uploader, &mut report).await?;
    let record_ids = migrate_records(local, prod, regime_id, &mut report).await?;
    migrate_outbox(local, prod, regime_id, &mut report).await?;
    let task_ids = migrate_review_tasks(local, prod, &filing_ids, &record_ids, &mut report).await?;
    migrate_review_audit(local, prod, &task_ids, &mut report).await?;

    Ok(report)
}

/// Builds the [`RegimeSeed`] `seed_regime` expects from LOCAL's own
/// `disclosure_regime`/`jurisdiction` rows — reuses `seed_regime` itself
/// rather than reconstructing its INSERT logic; the regime's `details` jsonb
/// column stays at schema default via that same reuse, matching every other
/// regime-seeding path in the codebase (no adapter populates it today).
async fn fetch_local_regime_seed(local: &PgPool, regime_id: &str) -> anyhow::Result<RegimeSeed> {
    #[derive(sqlx::FromRow)]
    struct Row {
        regime_id: String,
        jurisdiction_id: String,
        body: String,
        regime_type: String,
        value_precision: String,
        cadence: Option<String>,
        disclosure_lag_days: Option<i32>,
        source_url: Option<String>,
        effective_from: NaiveDate,
        jurisdiction_name: String,
        iso_code: Option<String>,
        level: String,
    }

    let row: Row = sqlx::query_as(
        "select r.id as regime_id, r.jurisdiction_id, r.body, r.regime_type, r.value_precision, \
           r.cadence, r.disclosure_lag_days, r.source_url, r.effective_from, \
           j.name as jurisdiction_name, j.iso_code, j.level \
         from disclosure_regime r join jurisdiction j on j.id = r.jurisdiction_id \
         where r.id = $1",
    )
    .bind(regime_id)
    .fetch_optional(local)
    .await
    .context("looking up the regime in LOCAL")?
    .with_context(|| format!("regime {regime_id:?} not found in LOCAL — nothing to migrate"))?;

    Ok(RegimeSeed {
        jurisdiction: JurisdictionSeed {
            id: row.jurisdiction_id,
            name: row.jurisdiction_name,
            iso_code: row.iso_code,
            level: row.level,
        },
        regime_id: row.regime_id,
        body: row.body,
        regime_type: row.regime_type,
        value_precision: row.value_precision,
        cadence: row.cadence,
        disclosure_lag_days: row.disclosure_lag_days,
        source_url: row.source_url,
        effective_from: row.effective_from,
    })
}

// --------------------------------------------------------------------- //
// politician / politician_alias / mandate.
// --------------------------------------------------------------------- //

#[derive(sqlx::FromRow)]
struct PoliticianRow {
    id: String,
    canonical_name: String,
    wikidata_qid: Option<String>,
    details: Value,
}

#[derive(sqlx::FromRow)]
struct AliasRow {
    politician_id: String,
    alias: String,
    lang: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MandateRow {
    id: String,
    politician_id: String,
    jurisdiction_id: String,
    body: String,
    role: String,
    party: Option<String>,
    district: String,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
}

/// Migrates politicians who actually filed under this regime — NOT every
/// roster member the historical roster seed created (that seeds every
/// member of a body for the whole range, whether or not they ever filed;
/// scoping to `filing.politician_id` keeps the migrated set minimal and
/// correct). For each such politician: the `politician` row, ALL of its
/// `politician_alias` rows (aliases are not jurisdiction-scoped), and its
/// `mandate` rows within the regime's own `jurisdiction_id` (per design —
/// a mandate the same politician holds in a DIFFERENT jurisdiction is out of
/// scope for a single-regime migration; that jurisdiction's own regime
/// migration carries it).
async fn migrate_politicians(
    local: &PgPool,
    prod: &PgPool,
    regime_id: &str,
    jurisdiction_id: &str,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let politician_ids: Vec<String> =
        sqlx::query_scalar("select distinct politician_id from filing where regime_id = $1")
            .bind(regime_id)
            .fetch_all(local)
            .await
            .context("listing politicians who filed under this regime (LOCAL)")?;

    for politician_id in &politician_ids {
        if let Err(error) =
            migrate_one_politician(local, prod, politician_id, jurisdiction_id, report).await
        {
            eprintln!("FAILED politician {politician_id}: {error:#}");
            report.politician.failed += 1;
        }
    }
    Ok(())
}

async fn migrate_one_politician(
    local: &PgPool,
    prod: &PgPool,
    politician_id: &str,
    jurisdiction_id: &str,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let politician: PoliticianRow = sqlx::query_as(
        "select id, canonical_name, wikidata_qid, details from politician where id = $1",
    )
    .bind(politician_id)
    .fetch_one(local)
    .await
    .with_context(|| format!("loading politician {politician_id} from LOCAL"))?;
    let inserted: Option<String> = sqlx::query_scalar(
        "insert into politician (id, canonical_name, wikidata_qid, details) values ($1, $2, $3, $4) \
         on conflict (id) do nothing returning id",
    )
    .bind(&politician.id)
    .bind(&politician.canonical_name)
    .bind(&politician.wikidata_qid)
    .bind(&politician.details)
    .fetch_optional(prod)
    .await
    .context("inserting politician into PROD")?;
    if inserted.is_some() {
        report.politician.migrated += 1;
    } else {
        report.politician.already_present += 1;
    }

    let aliases: Vec<AliasRow> = sqlx::query_as(
        "select politician_id, alias, lang from politician_alias where politician_id = $1",
    )
    .bind(politician_id)
    .fetch_all(local)
    .await
    .with_context(|| format!("loading politician_alias rows for {politician_id} from LOCAL"))?;
    for alias in &aliases {
        let inserted: Option<String> = sqlx::query_scalar(
            "insert into politician_alias (politician_id, alias, lang) values ($1, $2, $3) \
             on conflict (politician_id, alias) do nothing returning politician_id",
        )
        .bind(&alias.politician_id)
        .bind(&alias.alias)
        .bind(&alias.lang)
        .fetch_optional(prod)
        .await
        .with_context(|| format!("inserting politician_alias {:?} into PROD", alias.alias))?;
        if inserted.is_some() {
            report.politician_alias.migrated += 1;
        } else {
            report.politician_alias.already_present += 1;
        }
    }

    let mandates: Vec<MandateRow> = sqlx::query_as(
        "select id, politician_id, jurisdiction_id, body, role, party, district, start_date, \
           end_date \
         from mandate where politician_id = $1 and jurisdiction_id = $2",
    )
    .bind(politician_id)
    .bind(jurisdiction_id)
    .fetch_all(local)
    .await
    .with_context(|| format!("loading mandate rows for {politician_id} from LOCAL"))?;
    for mandate in &mandates {
        let inserted: Option<String> = sqlx::query_scalar(
            "insert into mandate \
               (id, politician_id, jurisdiction_id, body, role, party, district, start_date, \
                end_date) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             on conflict (id) do nothing returning id",
        )
        .bind(&mandate.id)
        .bind(&mandate.politician_id)
        .bind(&mandate.jurisdiction_id)
        .bind(&mandate.body)
        .bind(&mandate.role)
        .bind(&mandate.party)
        .bind(&mandate.district)
        .bind(mandate.start_date)
        .bind(mandate.end_date)
        .fetch_optional(prod)
        .await
        .with_context(|| format!("inserting mandate {} into PROD", mandate.id))?;
        if inserted.is_some() {
            report.mandate.migrated += 1;
        } else {
            report.mandate.already_present += 1;
        }
    }
    Ok(())
}

// --------------------------------------------------------------------- //
// raw_document / filing.
// --------------------------------------------------------------------- //

#[derive(sqlx::FromRow)]
struct RawDocumentRow {
    id: String,
    sha256: String,
    mime_type: String,
    source_url: Option<String>,
    fetched_at: DateTime<Utc>,
    fetch_run_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct FilingRow {
    id: String,
    regime_id: String,
    politician_id: String,
    raw_document_id: String,
    external_id: Option<String>,
    filing_type: String,
    filed_date: Option<NaiveDate>,
    published_at: Option<DateTime<Utc>>,
    discovered_at: DateTime<Utc>,
    supersedes_filing_id: Option<String>,
    details: Value,
}

/// Migrates every LOCAL filing for this regime, ordered `discovered_at`
/// ascending (see the module doc for why that ordering is sufficient for
/// `supersedes_filing_id`). Returns the full LOCAL filing id set (used to
/// scope `review_task` migration below) regardless of per-row outcome.
async fn migrate_filings(
    local: &PgPool,
    prod: &PgPool,
    regime_id: &str,
    bronze: &BronzeStore,
    uploader: &dyn BronzeUploader,
    report: &mut MigrationReport,
) -> anyhow::Result<Vec<String>> {
    let filings: Vec<FilingRow> = sqlx::query_as(
        "select id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
           filed_date, published_at, discovered_at, supersedes_filing_id, details \
         from filing where regime_id = $1 order by discovered_at asc, id asc",
    )
    .bind(regime_id)
    .fetch_all(local)
    .await
    .context("listing filings for this regime (LOCAL)")?;

    let filing_ids: Vec<String> = filings.iter().map(|f| f.id.clone()).collect();
    // Dedups raw_document uploads within this one run — a genuinely failed
    // upload is not retried again for a second filing sharing the same
    // document WITHIN this run (that filing's own INSERT then fails its
    // raw_document_id FK, caught the normal per-row way); a fresh
    // invocation retries it properly since this set starts empty again.
    let mut raw_documents_seen: BTreeSet<String> = BTreeSet::new();
    for filing in &filings {
        if let Err(error) = migrate_one_filing(
            local,
            prod,
            filing,
            bronze,
            uploader,
            &mut raw_documents_seen,
            report,
        )
        .await
        {
            eprintln!("FAILED filing {}: {error:#}", filing.id);
            report.filing.failed += 1;
        }
    }
    Ok(filing_ids)
}

async fn migrate_one_filing(
    local: &PgPool,
    prod: &PgPool,
    filing: &FilingRow,
    bronze: &BronzeStore,
    uploader: &dyn BronzeUploader,
    raw_documents_seen: &mut BTreeSet<String>,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    // Not collapsed into a `&&`-chained condition (avoids depending on
    // let-chains): two clearly separate questions — "have we seen this
    // document yet" and "did migrating it succeed".
    #[allow(clippy::collapsible_if)]
    if raw_documents_seen.insert(filing.raw_document_id.clone()) {
        if let Err(error) = migrate_raw_document(
            local,
            prod,
            &filing.raw_document_id,
            bronze,
            uploader,
            report,
        )
        .await
        {
            report.raw_document.failed += 1;
            anyhow::bail!(
                "raw_document {} did not migrate, filing cannot proceed: {error:#}",
                filing.raw_document_id
            );
        }
    }

    let inserted: Option<String> = sqlx::query_scalar(
        "insert into filing \
           (id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
            filed_date, published_at, discovered_at, supersedes_filing_id, details) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         on conflict (id) do nothing returning id",
    )
    .bind(&filing.id)
    .bind(&filing.regime_id)
    .bind(&filing.politician_id)
    .bind(&filing.raw_document_id)
    .bind(&filing.external_id)
    .bind(&filing.filing_type)
    .bind(filing.filed_date)
    .bind(filing.published_at)
    .bind(filing.discovered_at)
    .bind(&filing.supersedes_filing_id)
    .bind(&filing.details)
    .fetch_optional(prod)
    .await
    .context("inserting filing into PROD")?;
    if inserted.is_some() {
        report.filing.migrated += 1;
    } else {
        report.filing.already_present += 1;
    }
    Ok(())
}

/// Migrates one `raw_document` row: uploads the Bronze bytes (or confirms
/// they're already at the content-addressed path), rewrites `storage_uri`
/// to the resulting `gs://` URI, then inserts the row.
async fn migrate_raw_document(
    local: &PgPool,
    prod: &PgPool,
    raw_document_id: &str,
    bronze: &BronzeStore,
    uploader: &dyn BronzeUploader,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let row: RawDocumentRow = sqlx::query_as(
        "select id, sha256, mime_type, source_url, fetched_at, fetch_run_id \
         from raw_document where id = $1",
    )
    .bind(raw_document_id)
    .fetch_one(local)
    .await
    .with_context(|| format!("loading raw_document {raw_document_id} from LOCAL"))?;

    let local_path = bronze.path(&RawDocRef {
        sha256: row.sha256.clone(),
    });
    let outcome = uploader
        .ensure_uploaded(&row.sha256, &local_path)
        .with_context(|| format!("uploading bronze bytes for sha256 {}", row.sha256))?;
    let gs_uri = outcome.uri().to_owned();
    match outcome {
        UploadOutcome::Uploaded(_) => report.raw_document_uploaded += 1,
        UploadOutcome::AlreadyPresent(_) => report.raw_document_gcs_already_present += 1,
    }

    let inserted: Option<String> = sqlx::query_scalar(
        "insert into raw_document \
           (id, storage_uri, sha256, mime_type, source_url, fetched_at, fetch_run_id) \
         values ($1, $2, $3, $4, $5, $6, $7) \
         on conflict (id) do nothing returning id",
    )
    .bind(&row.id)
    .bind(&gs_uri)
    .bind(&row.sha256)
    .bind(&row.mime_type)
    .bind(&row.source_url)
    .bind(row.fetched_at)
    .bind(&row.fetch_run_id)
    .fetch_optional(prod)
    .await
    .context("inserting raw_document into PROD")?;
    if inserted.is_some() {
        report.raw_document.migrated += 1;
    } else {
        report.raw_document.already_present += 1;
    }
    Ok(())
}

// --------------------------------------------------------------------- //
// disclosure_record.
// --------------------------------------------------------------------- //

#[derive(sqlx::FromRow)]
struct RecordRow {
    id: String,
    filing_id: String,
    politician_id: String,
    regime_id: String,
    instrument_id: Option<String>,
    asset_description_raw: String,
    record_type: String,
    asset_class: String,
    side: Option<String>,
    transaction_date: Option<NaiveDate>,
    as_of_date: Option<NaiveDate>,
    notified_date: Option<NaiveDate>,
    value_low: Option<Decimal>,
    value_high: Option<Decimal>,
    currency: Option<String>,
    owner: Option<String>,
    verification_state: String,
    extraction_confidence: Option<f32>,
    extracted_by: String,
    fingerprint: String,
    supersedes_record_id: Option<String>,
    details: Value,
    created_at: DateTime<Utc>,
}

/// Migrates every LOCAL `disclosure_record` for this regime, ordered
/// `created_at` ascending (self-referential `supersedes_record_id` — same
/// treatment as `filing.supersedes_filing_id`, see the module doc). Excludes
/// `event_date` (a generated column — not insertable). Returns the full
/// LOCAL record id set (used to scope `review_task` migration below)
/// regardless of per-row outcome.
async fn migrate_records(
    local: &PgPool,
    prod: &PgPool,
    regime_id: &str,
    report: &mut MigrationReport,
) -> anyhow::Result<Vec<String>> {
    let records: Vec<RecordRow> = sqlx::query_as(
        "select id, filing_id, politician_id, regime_id, instrument_id, asset_description_raw, \
           record_type, asset_class, side, transaction_date, as_of_date, notified_date, \
           value_low, value_high, currency, owner, verification_state, extraction_confidence, \
           extracted_by, fingerprint, supersedes_record_id, details, created_at \
         from disclosure_record where regime_id = $1 order by created_at asc, id asc",
    )
    .bind(regime_id)
    .fetch_all(local)
    .await
    .context("listing disclosure_record rows for this regime (LOCAL)")?;

    let record_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    for record in &records {
        if let Err(error) = migrate_one_record(prod, record, report).await {
            eprintln!("FAILED disclosure_record {}: {error:#}", record.id);
            report.disclosure_record.failed += 1;
        }
    }
    Ok(record_ids)
}

async fn migrate_one_record(
    prod: &PgPool,
    record: &RecordRow,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let inserted: Option<String> = sqlx::query_scalar(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, instrument_id, asset_description_raw, \
            record_type, asset_class, side, transaction_date, as_of_date, notified_date, \
            value_low, value_high, currency, owner, verification_state, \
            extraction_confidence, extracted_by, fingerprint, supersedes_record_id, details, \
            created_at) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
                 $18, $19, $20, $21, $22, $23) \
         on conflict (id) do nothing returning id",
    )
    .bind(&record.id)
    .bind(&record.filing_id)
    .bind(&record.politician_id)
    .bind(&record.regime_id)
    .bind(&record.instrument_id)
    .bind(&record.asset_description_raw)
    .bind(&record.record_type)
    .bind(&record.asset_class)
    .bind(&record.side)
    .bind(record.transaction_date)
    .bind(record.as_of_date)
    .bind(record.notified_date)
    .bind(record.value_low)
    .bind(record.value_high)
    .bind(&record.currency)
    .bind(&record.owner)
    .bind(&record.verification_state)
    .bind(record.extraction_confidence)
    .bind(&record.extracted_by)
    .bind(&record.fingerprint)
    .bind(&record.supersedes_record_id)
    .bind(&record.details)
    .bind(record.created_at)
    .fetch_optional(prod)
    .await
    .context("inserting disclosure_record into PROD")?;
    if inserted.is_some() {
        report.disclosure_record.migrated += 1;
    } else {
        report.disclosure_record.already_present += 1;
    }
    Ok(())
}

// --------------------------------------------------------------------- //
// outbox_event / review_task.
// --------------------------------------------------------------------- //

#[derive(sqlx::FromRow)]
struct OutboxRow {
    id: String,
    kind: String,
    payload: Value,
    created_at: DateTime<Utc>,
    dispatched_at: Option<DateTime<Utc>>,
}

/// Migrates outbox events scoped by `payload->>'regime_id'` (both event
/// kinds, `disclosure_record.published` and `disclosure_record.corrected`,
/// carry `regime_id` in their payload — no FK links `outbox_event` to
/// `disclosure_record`, so this is the only scoping key available, and it's
/// exact). `dispatched_at` is copied EXACTLY as-is — never `NULL`ed out — so
/// this migration can never cause a real subscriber alert to fire.
async fn migrate_outbox(
    local: &PgPool,
    prod: &PgPool,
    regime_id: &str,
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let events: Vec<OutboxRow> = sqlx::query_as(
        "select id, kind, payload, created_at, dispatched_at from outbox_event \
         where payload ->> 'regime_id' = $1 order by created_at asc, id asc",
    )
    .bind(regime_id)
    .fetch_all(local)
    .await
    .context("listing outbox_event rows for this regime (LOCAL)")?;

    for event in &events {
        let inserted: anyhow::Result<Option<String>> = sqlx::query_scalar(
            "insert into outbox_event (id, kind, payload, created_at, dispatched_at) \
             values ($1, $2, $3, $4, $5) on conflict (id) do nothing returning id",
        )
        .bind(&event.id)
        .bind(&event.kind)
        .bind(&event.payload)
        .bind(event.created_at)
        .bind(event.dispatched_at)
        .fetch_optional(prod)
        .await
        .with_context(|| format!("inserting outbox_event {} into PROD", event.id));
        match inserted {
            Ok(Some(_)) => report.outbox_event.migrated += 1,
            Ok(None) => report.outbox_event.already_present += 1,
            Err(error) => {
                eprintln!("FAILED outbox_event {}: {error:#}", event.id);
                report.outbox_event.failed += 1;
            }
        }
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ReviewTaskRow {
    id: String,
    target_kind: String,
    target_id: String,
    reason: String,
    priority_score: f32,
    status: String,
    assignee: Option<String>,
    resolution: Option<Value>,
    created_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

/// Migrates review tasks targeting the `filing`/`disclosure_record` rows
/// just migrated. Regime-level tasks (e.g. sentinel drift freezes,
/// `target_kind = 'regime'`) are intentionally excluded — local operational
/// bookkeeping about the LOCAL run's own drift state, not filing-scoped
/// business data, mirroring the `pipeline_run` exclusion above. Returns the
/// full LOCAL task id set (used to scope `review_audit` migration below)
/// regardless of per-row outcome.
async fn migrate_review_tasks(
    local: &PgPool,
    prod: &PgPool,
    filing_ids: &[String],
    record_ids: &[String],
    report: &mut MigrationReport,
) -> anyhow::Result<Vec<String>> {
    let tasks: Vec<ReviewTaskRow> = sqlx::query_as(
        "select id, target_kind, target_id, reason, priority_score, status, assignee, \
           resolution, created_at, resolved_at \
         from review_task \
         where (target_kind = 'filing' and target_id = any($1)) \
            or (target_kind = 'disclosure_record' and target_id = any($2)) \
         order by created_at asc, id asc",
    )
    .bind(filing_ids)
    .bind(record_ids)
    .fetch_all(local)
    .await
    .context("listing review_task rows scoped to this regime's filings/records (LOCAL)")?;

    let task_ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    for task in &tasks {
        let inserted: anyhow::Result<Option<String>> = sqlx::query_scalar(
            "insert into review_task \
               (id, target_kind, target_id, reason, priority_score, status, assignee, \
                resolution, created_at, resolved_at) \
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             on conflict (id) do nothing returning id",
        )
        .bind(&task.id)
        .bind(&task.target_kind)
        .bind(&task.target_id)
        .bind(&task.reason)
        .bind(task.priority_score)
        .bind(&task.status)
        .bind(&task.assignee)
        .bind(&task.resolution)
        .bind(task.created_at)
        .bind(task.resolved_at)
        .fetch_optional(prod)
        .await
        .with_context(|| format!("inserting review_task {} into PROD", task.id));
        match inserted {
            Ok(Some(_)) => report.review_task.migrated += 1,
            Ok(None) => report.review_task.already_present += 1,
            Err(error) => {
                eprintln!("FAILED review_task {}: {error:#}", task.id);
                report.review_task.failed += 1;
            }
        }
    }
    Ok(task_ids)
}

#[derive(sqlx::FromRow)]
struct ReviewAuditRow {
    id: String,
    review_task_id: String,
    reviewer: String,
    verdict: String,
    outcome: String,
    note: Option<String>,
    affected_record_ids: Value,
    created_at: DateTime<Utc>,
}

/// Migrates the audit-trail ledger (design §7.2, `crates/core/migrations/
/// 0006_review_audit.sql`) for the `review_task` rows just migrated. FK
/// `review_task_id references review_task(id)`, so this must run after
/// [`migrate_review_tasks`] — a task that failed to migrate leaves its
/// audit rows' INSERTs to fail their own FK check, caught the normal
/// per-row way.
async fn migrate_review_audit(
    local: &PgPool,
    prod: &PgPool,
    task_ids: &[String],
    report: &mut MigrationReport,
) -> anyhow::Result<()> {
    let audits: Vec<ReviewAuditRow> = sqlx::query_as(
        "select id, review_task_id, reviewer, verdict, outcome, note, affected_record_ids, \
           created_at \
         from review_audit where review_task_id = any($1) order by created_at asc, id asc",
    )
    .bind(task_ids)
    .fetch_all(local)
    .await
    .context("listing review_audit rows scoped to this regime's review_task rows (LOCAL)")?;

    for audit in &audits {
        let inserted: anyhow::Result<Option<String>> = sqlx::query_scalar(
            "insert into review_audit \
               (id, review_task_id, reviewer, verdict, outcome, note, affected_record_ids, \
                created_at) \
             values ($1, $2, $3, $4, $5, $6, $7, $8) \
             on conflict (id) do nothing returning id",
        )
        .bind(&audit.id)
        .bind(&audit.review_task_id)
        .bind(&audit.reviewer)
        .bind(&audit.verdict)
        .bind(&audit.outcome)
        .bind(&audit.note)
        .bind(&audit.affected_record_ids)
        .bind(audit.created_at)
        .fetch_optional(prod)
        .await
        .with_context(|| format!("inserting review_audit {} into PROD", audit.id));
        match inserted {
            Ok(Some(_)) => report.review_audit.migrated += 1,
            Ok(None) => report.review_audit.already_present += 1,
            Err(error) => {
                eprintln!("FAILED review_audit {}: {error:#}", audit.id);
                report.review_audit.failed += 1;
            }
        }
    }
    Ok(())
}
