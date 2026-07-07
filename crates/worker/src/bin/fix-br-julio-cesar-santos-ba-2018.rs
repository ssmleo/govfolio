//! One-off, narrowly-scoped fix for the single live `br` politician-identity
//! collision documented in
//! `docs/decisions/br-identity-collision-remediation.md` (the "plan" every
//! `§` reference below points at — read it in full before touching this
//! file; every constant and query here is copied from it, not
//! re-derived independently).
//!
//! Two real people, both named `JULIO CESAR DOS SANTOS`, both 2018
//! `DEPUTADO FEDERAL` candidates in Bahia (`BA`), currently share one
//! `politician` row (`01KWXE3M4J18YNCD5R1V7NTGQ3`, plan §1). This fixes
//! EXACTLY that one case: it mints a fresh `politician`/`politician_alias`/
//! `mandate` for the smaller side (CPF `67701124500`, filing
//! `01KWXEDGZQ8K0E9ZA75PB5C0ZS`, one `disclosure_record`) and repoints that
//! filing's and record's foreign keys onto it. The larger side (CPF
//! `80673872653`, 4 records) keeps the existing `politician_id` untouched —
//! zero rows move for that person (plan §5's row-count tie-breaker).
//!
//! Deliberately NOT a general "fix any collision" tool (plan §8's own
//! instruction) — do not generalize this file. If a second case surfaces,
//! factor shared logic out then, not speculatively now.
//!
//! Dry-run by default (read-only checks + a "would write" report — mirrors
//! `bin/backfill-real-br.rs`'s own dry-run/`--execute` convention);
//! `--execute` gates the one real transaction (plan §8 steps 4-6). Per the
//! plan's own recommended procedure (§10 "Execution authority"), the FIRST
//! `--execute` run against the shared dev DB must happen only after an
//! independent auditor has code-reviewed this file.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin fix-br-julio-cesar-santos-ba-2018 [-- --execute]
//! ```
//!
//! Env: `DATABASE_URL` (required).

use std::collections::HashSet;
use std::path::Path;

use anyhow::Context as _;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use br::BrAdapter;
use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RawDocRef, RunCtx};
use pipeline::conformance::workspace_root;
use worker::backfill::{FilingBaseline, candidate_fingerprints};

// --- narrow, case-specific constants (plan §1/§5/§8 — do NOT generalize) ---

/// The shared `politician.id` both real people currently sit under (plan §1).
const OLD_POLITICIAN_ID: &str = "01KWXE3M4J18YNCD5R1V7NTGQ3";
/// CPF of the person who KEEPS `OLD_POLITICIAN_ID` (4 `disclosure_record`s —
/// plan §5's "more Gold rows" tie-breaker; zero rows move for this person).
const CPF_STAYS: &str = "80673872653";
/// CPF of the person who gets a freshly minted `politician_id` (1
/// `disclosure_record` — the smaller side, plan §5).
const CPF_MOVES: &str = "67701124500";
/// The one `filing.id` that moves to the new politician (plan §1/§6).
const MOVING_FILING_ID: &str = "01KWXEDGZQ8K0E9ZA75PB5C0ZS";

fn parse_args() -> anyhow::Result<bool> {
    let mut execute = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--execute" => execute = true,
            other => anyhow::bail!("unknown argument {other:?} (expected --execute)"),
        }
    }
    Ok(execute)
}

// ---------------------------------------------------------------------------
// Step 0: the plan's §2 exhaustive CPF-collision sweep (read-only).
// ---------------------------------------------------------------------------

/// One row of the plan's §2 CPF-collision sweep: a politician whose filings'
/// `stg_br` rows carry more than one distinct CPF.
#[derive(sqlx::FromRow)]
struct SweepRow {
    politician_id: String,
    canonical_name: String,
    distinct_cpfs: i64,
    cpfs: Vec<String>,
}

/// Copied verbatim from plan §2 — an exhaustive, whole-dataset sweep (every
/// year, every body), not scoped to `OLD_POLITICIAN_ID`.
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
        .context("running the plan §2 CPF-collision sweep")
}

fn print_sweep(rows: &[SweepRow]) {
    if rows.is_empty() {
        println!("  (no politician has more than one distinct CPF across its filings)");
    }
    for row in rows {
        println!(
            "  politician {} ({:?}): {} distinct CPFs {:?}",
            row.politician_id, row.canonical_name, row.distinct_cpfs, row.cpfs
        );
    }
}

/// What the Step 0 sweep implies about whether it is safe to proceed. Pure
/// and DB-free — see the `classify_sweep` tests at the bottom of this file.
#[derive(Debug, PartialEq, Eq)]
enum ScopeCheck {
    /// Exactly the one expected collision (plan §2) — proceed.
    AsExpected,
    /// No row for [`OLD_POLITICIAN_ID`]. Either this exact fix was already
    /// applied (plan §7 item 6's idempotency case — the sweep going quiet
    /// for this politician is the intended detection signal) or something
    /// else changed. This function alone cannot tell the two apart (it only
    /// sees the sweep); the caller must check the moving filing's CURRENT
    /// `politician_id` live to disambiguate.
    NoRowForOldPolitician,
    /// A row exists but does not match what the plan diagnosed — halt, do
    /// not proceed on a stale assumption (plan §8 Step 0).
    PremiseChanged(String),
}

fn classify_sweep(rows: &[SweepRow]) -> ScopeCheck {
    if rows.is_empty() {
        return ScopeCheck::NoRowForOldPolitician;
    }
    if rows.len() > 1 {
        return ScopeCheck::PremiseChanged(format!(
            "sweep returned {} collision rows; plan §2 found exactly 1",
            rows.len()
        ));
    }
    let row = &rows[0];
    if row.politician_id != OLD_POLITICIAN_ID {
        return ScopeCheck::PremiseChanged(format!(
            "sweep's one row is politician {}, expected {OLD_POLITICIAN_ID}",
            row.politician_id
        ));
    }
    let mut got: Vec<&str> = row.cpfs.iter().map(String::as_str).collect();
    got.sort_unstable();
    let mut want = [CPF_MOVES, CPF_STAYS];
    want.sort_unstable();
    if got != want {
        return ScopeCheck::PremiseChanged(format!(
            "sweep's CPFs for {OLD_POLITICIAN_ID} are {got:?}, expected {want:?}"
        ));
    }
    ScopeCheck::AsExpected
}

async fn fetch_filing_politician_id(
    pool: &PgPool,
    filing_id: &str,
) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar("select politician_id from filing where id = $1")
        .bind(filing_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up filing {filing_id}'s current politician_id"))
}

/// Step 0's verdict: either proceed, or the fix is already applied (and to
/// which `politician_id` it already moved).
enum ScopeOutcome {
    Proceed,
    AlreadyApplied { moved_to: String },
}

/// Classifies the sweep and, when it comes back quiet for
/// `OLD_POLITICIAN_ID`, disambiguates "already applied" from "something else
/// changed" via one live lookup (see [`ScopeCheck::NoRowForOldPolitician`]).
async fn check_scope(pool: &PgPool, sweep: &[SweepRow]) -> anyhow::Result<ScopeOutcome> {
    match classify_sweep(sweep) {
        ScopeCheck::AsExpected => Ok(ScopeOutcome::Proceed),
        ScopeCheck::PremiseChanged(reason) => anyhow::bail!(
            "Step 0 HALT: {reason} — the premise this fix was built on has changed; do not \
             proceed on a stale assumption (plan §8 Step 0)."
        ),
        ScopeCheck::NoRowForOldPolitician => {
            match fetch_filing_politician_id(pool, MOVING_FILING_ID).await? {
                None => anyhow::bail!(
                    "Step 0 HALT: sweep is empty AND filing {MOVING_FILING_ID} was not found — \
                     cannot tell an already-applied fix from a vanished filing; do not proceed."
                ),
                Some(id) if id == OLD_POLITICIAN_ID => anyhow::bail!(
                    "Step 0 HALT: sweep no longer shows {OLD_POLITICIAN_ID}, but filing \
                     {MOVING_FILING_ID} is still attached to it — unexplained change; do not \
                     proceed on a stale assumption."
                ),
                Some(id) => Ok(ScopeOutcome::AlreadyApplied { moved_to: id }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Step 1: the plan's §6 blast-radius rows + the pre-change snapshot.
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct PoliticianRow {
    id: String,
    canonical_name: String,
    wikidata_qid: Option<String>,
    details: serde_json::Value,
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
    district: Option<String>,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
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
    details: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct DisclosureRecordRow {
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
    event_date: Option<NaiveDate>,
    value_low: Option<Decimal>,
    value_high: Option<Decimal>,
    currency: Option<String>,
    owner: Option<String>,
    verification_state: String,
    extraction_confidence: Option<f32>,
    extracted_by: String,
    fingerprint: String,
    supersedes_record_id: Option<String>,
    details: serde_json::Value,
    created_at: DateTime<Utc>,
}

/// Every row the plan's §6 blast radius names for `OLD_POLITICIAN_ID` — the
/// same scope as the plan's §8 step 1 `\copy` queries.
struct BlastRadius {
    politician: PoliticianRow,
    aliases: Vec<AliasRow>,
    mandates: Vec<MandateRow>,
    filings: Vec<FilingRow>,
    records: Vec<DisclosureRecordRow>,
}

async fn fetch_politician(pool: &PgPool, id: &str) -> anyhow::Result<PoliticianRow> {
    sqlx::query_as("select id, canonical_name, wikidata_qid, details from politician where id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up politician {id}"))?
        .with_context(|| format!("politician {id} not found"))
}

async fn fetch_aliases(pool: &PgPool, politician_id: &str) -> anyhow::Result<Vec<AliasRow>> {
    sqlx::query_as(
        "select politician_id, alias, lang from politician_alias where politician_id = $1",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up politician_alias rows for {politician_id}"))
}

async fn fetch_mandates(pool: &PgPool, politician_id: &str) -> anyhow::Result<Vec<MandateRow>> {
    sqlx::query_as(
        "select id, politician_id, jurisdiction_id, body, role, party, district, start_date, \
         end_date from mandate where politician_id = $1",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up mandate rows for {politician_id}"))
}

async fn fetch_filings_for_politician(
    pool: &PgPool,
    politician_id: &str,
) -> anyhow::Result<Vec<FilingRow>> {
    sqlx::query_as(
        "select id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
         filed_date, published_at, discovered_at, supersedes_filing_id, details \
         from filing where politician_id = $1 order by id",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up filing rows for politician {politician_id}"))
}

async fn fetch_records_for_politician(
    pool: &PgPool,
    politician_id: &str,
) -> anyhow::Result<Vec<DisclosureRecordRow>> {
    sqlx::query_as(
        "select id, filing_id, politician_id, regime_id, instrument_id, asset_description_raw, \
         record_type, asset_class, side, transaction_date, as_of_date, notified_date, \
         event_date, value_low, value_high, currency, owner, verification_state, \
         extraction_confidence, extracted_by, fingerprint, supersedes_record_id, details, \
         created_at from disclosure_record where politician_id = $1 order by id",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up disclosure_record rows for politician {politician_id}"))
}

async fn fetch_raw_document_sha256(pool: &PgPool, raw_document_id: &str) -> anyhow::Result<String> {
    sqlx::query_scalar("select sha256 from raw_document where id = $1")
        .bind(raw_document_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up raw_document {raw_document_id}"))?
        .with_context(|| format!("raw_document {raw_document_id} not found"))
}

/// Loads + sanity-checks every row the plan's §6 blast radius names for
/// `OLD_POLITICIAN_ID`: exactly 1 `politician_alias`, 1 mandate (plan §6), 2
/// filings, 5 `disclosure_record`s (plan §1: the moving 1 + the staying 4).
/// Any count mismatch is a halt — the premise changed since the plan was
/// written.
async fn load_blast_radius(pool: &PgPool) -> anyhow::Result<BlastRadius> {
    println!("Step 1: loading the blast-radius rows for {OLD_POLITICIAN_ID}...");
    let politician = fetch_politician(pool, OLD_POLITICIAN_ID).await?;
    let aliases = fetch_aliases(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        aliases.len() == 1,
        "expected exactly 1 politician_alias row for {OLD_POLITICIAN_ID} (plan §6), found {}",
        aliases.len()
    );
    let mandates = fetch_mandates(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        mandates.len() == 1,
        "expected exactly 1 mandate row for {OLD_POLITICIAN_ID} (plan §6), found {}",
        mandates.len()
    );
    let filings = fetch_filings_for_politician(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        filings.len() == 2,
        "expected exactly 2 filing rows for {OLD_POLITICIAN_ID} (plan §1), found {}",
        filings.len()
    );
    let records = fetch_records_for_politician(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        records.len() == 5,
        "expected exactly 5 disclosure_record rows for {OLD_POLITICIAN_ID} (plan §1: 1+4), \
         found {}",
        records.len()
    );
    Ok(BlastRadius {
        politician,
        aliases,
        mandates,
        filings,
        records,
    })
}

fn opt<T: ToString>(value: Option<&T>) -> String {
    value.map(ToString::to_string).unwrap_or_default()
}

fn write_csv<I: IntoIterator<Item = Vec<String>>>(
    path: &Path,
    header: &[&str],
    rows: I,
) -> anyhow::Result<usize> {
    let mut writer =
        csv::Writer::from_path(path).with_context(|| format!("opening {}", path.display()))?;
    writer
        .write_record(header)
        .with_context(|| format!("writing header for {}", path.display()))?;
    let mut count = 0usize;
    for row in rows {
        writer
            .write_record(row)
            .with_context(|| format!("writing a row for {}", path.display()))?;
        count += 1;
    }
    writer
        .flush()
        .with_context(|| format!("flushing {}", path.display()))?;
    Ok(count)
}

fn write_politician_csv(dir: &Path, politician: &PoliticianRow) -> anyhow::Result<usize> {
    write_csv(
        &dir.join("snapshot_politician.csv"),
        &["id", "canonical_name", "wikidata_qid", "details"],
        vec![vec![
            politician.id.clone(),
            politician.canonical_name.clone(),
            opt(politician.wikidata_qid.as_ref()),
            politician.details.to_string(),
        ]],
    )
}

fn write_alias_csv(dir: &Path, aliases: &[AliasRow]) -> anyhow::Result<usize> {
    write_csv(
        &dir.join("snapshot_alias.csv"),
        &["politician_id", "alias", "lang"],
        aliases
            .iter()
            .map(|r| {
                vec![
                    r.politician_id.clone(),
                    r.alias.clone(),
                    opt(r.lang.as_ref()),
                ]
            })
            .collect::<Vec<_>>(),
    )
}

fn write_mandate_csv(dir: &Path, mandates: &[MandateRow]) -> anyhow::Result<usize> {
    write_csv(
        &dir.join("snapshot_mandate.csv"),
        &[
            "id",
            "politician_id",
            "jurisdiction_id",
            "body",
            "role",
            "party",
            "district",
            "start_date",
            "end_date",
        ],
        mandates
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.politician_id.clone(),
                    r.jurisdiction_id.clone(),
                    r.body.clone(),
                    r.role.clone(),
                    opt(r.party.as_ref()),
                    opt(r.district.as_ref()),
                    r.start_date.to_string(),
                    opt(r.end_date.as_ref()),
                ]
            })
            .collect::<Vec<_>>(),
    )
}

fn write_filing_csv(dir: &Path, filings: &[FilingRow]) -> anyhow::Result<usize> {
    write_csv(
        &dir.join("snapshot_filing.csv"),
        &[
            "id",
            "regime_id",
            "politician_id",
            "raw_document_id",
            "external_id",
            "filing_type",
            "filed_date",
            "published_at",
            "discovered_at",
            "supersedes_filing_id",
            "details",
        ],
        filings
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.regime_id.clone(),
                    r.politician_id.clone(),
                    r.raw_document_id.clone(),
                    opt(r.external_id.as_ref()),
                    r.filing_type.clone(),
                    opt(r.filed_date.as_ref()),
                    r.published_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                    r.discovered_at.to_rfc3339(),
                    opt(r.supersedes_filing_id.as_ref()),
                    r.details.to_string(),
                ]
            })
            .collect::<Vec<_>>(),
    )
}

fn write_records_csv(dir: &Path, records: &[DisclosureRecordRow]) -> anyhow::Result<usize> {
    write_csv(
        &dir.join("snapshot_records.csv"),
        &[
            "id",
            "filing_id",
            "politician_id",
            "regime_id",
            "instrument_id",
            "asset_description_raw",
            "record_type",
            "asset_class",
            "side",
            "transaction_date",
            "as_of_date",
            "notified_date",
            "event_date",
            "value_low",
            "value_high",
            "currency",
            "owner",
            "verification_state",
            "extraction_confidence",
            "extracted_by",
            "fingerprint",
            "supersedes_record_id",
            "details",
            "created_at",
        ],
        records
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.filing_id.clone(),
                    r.politician_id.clone(),
                    r.regime_id.clone(),
                    opt(r.instrument_id.as_ref()),
                    r.asset_description_raw.clone(),
                    r.record_type.clone(),
                    r.asset_class.clone(),
                    opt(r.side.as_ref()),
                    opt(r.transaction_date.as_ref()),
                    opt(r.as_of_date.as_ref()),
                    opt(r.notified_date.as_ref()),
                    opt(r.event_date.as_ref()),
                    opt(r.value_low.as_ref()),
                    opt(r.value_high.as_ref()),
                    opt(r.currency.as_ref()),
                    opt(r.owner.as_ref()),
                    r.verification_state.clone(),
                    opt(r.extraction_confidence.as_ref()),
                    r.extracted_by.clone(),
                    r.fingerprint.clone(),
                    opt(r.supersedes_record_id.as_ref()),
                    r.details.to_string(),
                    r.created_at.to_rfc3339(),
                ]
            })
            .collect::<Vec<_>>(),
    )
}

/// Writes the plan's §8 step 1 pre-change snapshot: one CSV per table,
/// scoped exactly like the plan's own `\copy` queries — a real, inspectable
/// artifact on disk before any write happens.
fn write_snapshot(dir: &Path, radius: &BlastRadius) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let n_politician = write_politician_csv(dir, &radius.politician)?;
    let n_alias = write_alias_csv(dir, &radius.aliases)?;
    let n_mandate = write_mandate_csv(dir, &radius.mandates)?;
    let n_filing = write_filing_csv(dir, &radius.filings)?;
    let n_records = write_records_csv(dir, &radius.records)?;

    println!(
        "  snapshot: {n_politician} politician, {n_alias} politician_alias, {n_mandate} \
         mandate, {n_filing} filing, {n_records} disclosure_record row(s) -> {}",
        dir.display()
    );
    Ok(())
}

/// Finds the one moving filing + its one `disclosure_record` inside the
/// already-loaded blast radius (plan §1/§6) — no new query, just the
/// in-memory scope the plan's §8 execution steps operate on.
fn find_moving(radius: &BlastRadius) -> anyhow::Result<(&FilingRow, &DisclosureRecordRow)> {
    let filing = radius
        .filings
        .iter()
        .find(|f| f.id == MOVING_FILING_ID)
        .with_context(|| {
            format!("filing {MOVING_FILING_ID} not found among {OLD_POLITICIAN_ID}'s filings")
        })?;
    anyhow::ensure!(
        filing.regime_id == br::seed::REGIME_ID,
        "moving filing {MOVING_FILING_ID} has regime_id {}, expected the Câmara regime {} \
         (plan §1)",
        filing.regime_id,
        br::seed::REGIME_ID
    );
    let moving_records: Vec<&DisclosureRecordRow> = radius
        .records
        .iter()
        .filter(|r| r.filing_id == MOVING_FILING_ID)
        .collect();
    anyhow::ensure!(
        moving_records.len() == 1,
        "expected exactly 1 disclosure_record for the moving filing {MOVING_FILING_ID}, found {}",
        moving_records.len()
    );
    Ok((filing, moving_records[0]))
}

// ---------------------------------------------------------------------------
// Step 2/3: re-derive from Bronze, self-check, then the corrected fingerprint.
// ---------------------------------------------------------------------------

/// Plan §8 Step 2: reads the raw declaration by sha256 via `BronzeStore::get`
/// (NOT `raw_document.storage_uri`, which is known-stale for these two rows —
/// see the plan's own note), re-parses + re-normalizes it, and asserts the
/// re-derived fingerprint (under the OLD `politician_id`) matches the LIVE
/// stored `disclosure_record.fingerprint` exactly. A mismatch is a hard halt
/// — the re-derivation would not be proven exact.
async fn rederive_and_check(
    pool: &PgPool,
    filing: &FilingRow,
    record: &DisclosureRecordRow,
) -> anyhow::Result<(Vec<GoldCandidate>, FilingBaseline)> {
    let raw_document_sha256 = fetch_raw_document_sha256(pool, &filing.raw_document_id).await?;
    println!("Step 2: re-deriving candidates from Bronze (sha256 {raw_document_sha256})...");

    let adapter = BrAdapter::default();
    let bronze_root = workspace_root()
        .join("target")
        .join("bronze-backfill-real-br");
    let ctx = RunCtx::new(
        BronzeStore::open(bronze_root)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;
    let raw_doc = RawDocRef {
        sha256: raw_document_sha256,
    };
    let staging_rows = adapter
        .parse(&raw_doc, &ctx)
        .await
        .context("re-parsing the moving filing's raw declaration")?;
    let candidates = adapter
        .normalize(&staging_rows, &ctx)
        .await
        .context("re-normalizing the moving filing's declaration")?;
    anyhow::ensure!(
        candidates.len() == 1,
        "expected exactly 1 re-derived GoldCandidate for {}, got {}",
        filing.id,
        candidates.len()
    );

    let old_baseline = FilingBaseline {
        filing_id: filing.id.clone(),
        politician_id: OLD_POLITICIAN_ID.to_owned(),
        regime_id: filing.regime_id.clone(),
        fingerprints: HashSet::new(),
    };
    let rederived = candidate_fingerprints("br", &old_baseline, &candidates)
        .context("recomputing the fingerprint under the OLD politician_id")?;
    anyhow::ensure!(
        rederived.len() == 1,
        "expected exactly 1 recomputed fingerprint, got {}",
        rederived.len()
    );
    anyhow::ensure!(
        rederived[0] == record.fingerprint,
        "Step 2 HALT: re-derived fingerprint {} does not match the currently stored \
         disclosure_record.fingerprint {} for {} — the re-derivation is not proven exact; \
         nothing downstream may be trusted (plan §8 Step 2).",
        rederived[0],
        record.fingerprint,
        record.id
    );
    println!(
        "Step 2 OK: re-derived fingerprint matches the stored fingerprint {}",
        record.fingerprint
    );
    Ok((candidates, old_baseline))
}

/// Plan §8 Step 3: the corrected fingerprint under a freshly minted
/// `politician_id` (generated at RUN TIME, never hardcoded).
fn compute_new_fingerprint(
    candidates: &[GoldCandidate],
    old_baseline: FilingBaseline,
) -> anyhow::Result<(String, String, String)> {
    let new_politician_id = govfolio_core::ids::PoliticianId::generate().to_string();
    let new_mandate_id = ulid::Ulid::new().to_string();
    let new_baseline = FilingBaseline {
        politician_id: new_politician_id.clone(),
        ..old_baseline
    };
    let new_fps = candidate_fingerprints("br", &new_baseline, candidates)
        .context("computing the corrected fingerprint under the new politician_id")?;
    anyhow::ensure!(
        new_fps.len() == 1,
        "expected exactly 1 corrected fingerprint, got {}",
        new_fps.len()
    );
    println!(
        "Step 3: new politician_id {new_politician_id} (mandate {new_mandate_id}) -> corrected \
         fingerprint {}",
        new_fps[0]
    );
    Ok((new_politician_id, new_mandate_id, new_fps[0].clone()))
}

// ---------------------------------------------------------------------------
// Steps 4-6: the one write transaction (--execute only).
// ---------------------------------------------------------------------------

/// Plan §8 steps 4-6, in ONE transaction: mint the new politician/alias/
/// mandate rows (byte-identical copies of the old politician's, plan §6),
/// then repoint the moving filing and its one `disclosure_record`.
///
/// Idempotency: `check_scope` (Step 0, called before this function) already
/// proved — before this transaction opened — that the sweep matched exactly
/// the expected collision and `filing.politician_id` was still
/// `OLD_POLITICIAN_ID`. That means this call IS the first application; a
/// second invocation never reaches here (it short-circuits at Step 0's
/// `ScopeOutcome::AlreadyApplied`, plan §7 item 6). The `WHERE` guards on the
/// two `UPDATE`s below are still included verbatim per the plan's own SQL
/// (§8 steps 5-6) as defense-in-depth: `rows_affected() != 1` past this
/// point means a concurrent writer raced this transaction since Step 0 — a
/// genuine hazard to fail loudly on, not a normal idempotent no-op.
async fn apply_fix(
    pool: &PgPool,
    radius: &BlastRadius,
    filing: &FilingRow,
    record: &DisclosureRecordRow,
    new_politician_id: &str,
    new_mandate_id: &str,
    new_fingerprint: &str,
) -> anyhow::Result<()> {
    let politician = &radius.politician;
    let alias = &radius.aliases[0];
    let mandate = &radius.mandates[0];

    let mut tx = pool.begin().await.context("opening the fix transaction")?;

    sqlx::query(
        "insert into politician (id, canonical_name, wikidata_qid, details) values ($1, $2, $3, $4)",
    )
    .bind(new_politician_id)
    .bind(&politician.canonical_name)
    .bind(&politician.wikidata_qid)
    .bind(&politician.details)
    .execute(&mut *tx)
    .await
    .context("Step 4: inserting the new politician row")?;

    sqlx::query("insert into politician_alias (politician_id, alias, lang) values ($1, $2, $3)")
        .bind(new_politician_id)
        .bind(&alias.alias)
        .bind(&alias.lang)
        .execute(&mut *tx)
        .await
        .context("Step 4: inserting the new politician_alias row")?;

    sqlx::query(
        "insert into mandate \
           (id, politician_id, jurisdiction_id, body, role, party, district, start_date, end_date) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(new_mandate_id)
    .bind(new_politician_id)
    .bind(&mandate.jurisdiction_id)
    .bind(&mandate.body)
    .bind(&mandate.role)
    .bind(&mandate.party)
    .bind(&mandate.district)
    .bind(mandate.start_date)
    .bind(mandate.end_date)
    .execute(&mut *tx)
    .await
    .context("Step 4: inserting the new mandate row")?;

    let filing_result =
        sqlx::query("update filing set politician_id = $1 where id = $2 and politician_id = $3")
            .bind(new_politician_id)
            .bind(&filing.id)
            .bind(OLD_POLITICIAN_ID)
            .execute(&mut *tx)
            .await
            .context("Step 5: repointing the filing")?;
    anyhow::ensure!(
        filing_result.rows_affected() == 1,
        "Step 5: expected to repoint exactly 1 filing row, affected {} — a concurrent write \
         raced this transaction since Step 0; aborting rather than proceeding blind",
        filing_result.rows_affected()
    );

    let record_result = sqlx::query(
        "update disclosure_record set politician_id = $1, fingerprint = $2 \
         where id = $3 and politician_id = $4 and fingerprint = $5",
    )
    .bind(new_politician_id)
    .bind(new_fingerprint)
    .bind(&record.id)
    .bind(OLD_POLITICIAN_ID)
    .bind(&record.fingerprint)
    .execute(&mut *tx)
    .await
    .context("Step 6: repointing the disclosure_record")?;
    anyhow::ensure!(
        record_result.rows_affected() == 1,
        "Step 6: expected to repoint exactly 1 disclosure_record row, affected {} — a \
         concurrent write raced this transaction since Step 0; aborting rather than \
         proceeding blind",
        record_result.rows_affected()
    );

    tx.commit()
        .await
        .context("committing the fix transaction")?;
    println!(
        "Steps 4-6 committed: politician {new_politician_id} minted; filing {} and \
         disclosure_record {} repointed.",
        filing.id, record.id
    );
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let execute = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool)
        .await
        .context("applying migrations")?;

    println!(
        "== fix-br-julio-cesar-santos-ba-2018 ({}) ==",
        if execute { "EXECUTE" } else { "DRY-RUN" }
    );
    println!("Step 0: re-running the plan §2 CPF-collision sweep (read-only)...");
    let sweep = run_sweep(&pool).await?;
    print_sweep(&sweep);

    if let ScopeOutcome::AlreadyApplied { moved_to } = check_scope(&pool, &sweep).await? {
        println!(
            "Step 0: already applied — filing {MOVING_FILING_ID} is already repointed to \
             {moved_to}. No-op (plan §7 item 6 idempotency)."
        );
        println!("Step 7 (no-op check): current sweep state:");
        print_sweep(&sweep);
        return Ok(());
    }
    println!(
        "Step 0 OK: exactly the expected collision (politician {OLD_POLITICIAN_ID}, CPFs \
         {CPF_STAYS}/{CPF_MOVES})."
    );

    let radius = load_blast_radius(&pool).await?;
    let snapshot_dir = workspace_root().join("target").join(format!(
        "fix-br-julio-cesar-santos-ba-2018-snapshot-{}",
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    write_snapshot(&snapshot_dir, &radius)?;
    println!(
        "Step 1: pre-change snapshot written to {}",
        snapshot_dir.display()
    );

    let (filing, record) = find_moving(&radius)?;
    let (candidates, old_baseline) = rederive_and_check(&pool, filing, record).await?;
    let (new_politician_id, new_mandate_id, new_fingerprint) =
        compute_new_fingerprint(&candidates, old_baseline)?;

    if !execute {
        println!("--- DRY-RUN: nothing written. Would, in ONE transaction: ---");
        println!(
            "  INSERT politician {new_politician_id} (copy of {OLD_POLITICIAN_ID}: {:?})",
            radius.politician.canonical_name
        );
        println!(
            "  INSERT politician_alias ({new_politician_id}, {:?})",
            radius.aliases[0].alias
        );
        println!(
            "  INSERT mandate {new_mandate_id} ({new_politician_id}, copy of mandate {})",
            radius.mandates[0].id
        );
        println!(
            "  UPDATE filing {} politician_id -> {new_politician_id}",
            filing.id
        );
        println!(
            "  UPDATE disclosure_record {} politician_id -> {new_politician_id}, fingerprint \
             -> {new_fingerprint}",
            record.id
        );
        println!(
            "Step 7: sweep is unchanged (dry-run wrote nothing) — still shows the one collision:"
        );
        print_sweep(&sweep);
        return Ok(());
    }

    apply_fix(
        &pool,
        &radius,
        filing,
        record,
        &new_politician_id,
        &new_mandate_id,
        &new_fingerprint,
    )
    .await?;

    let after = run_sweep(&pool).await?;
    println!("Step 7: post-fix sweep:");
    print_sweep(&after);
    anyhow::ensure!(
        after.is_empty(),
        "Step 7: sweep is NOT empty after the fix committed — investigate before trusting this run"
    );
    println!("Step 7 OK: collision resolved (sweep returns zero rows).");

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Proves the Step 0 / idempotency decision logic (plan §7 item 6: "the
    /// fix script itself must be safe to invoke twice ... must detect the
    /// split is already done ... and no-op rather than erroring or
    /// double-inserting"). This is the pure, DB-free half of that guarantee:
    /// [`check_scope`]'s live disambiguation (is a quiet sweep "already
    /// applied" or "something else changed"?) is exercised by hand against
    /// the real dev DB via dry-run re-invocation, not unit-testable without
    /// one.
    fn row(politician_id: &str, cpfs: &[&str]) -> SweepRow {
        SweepRow {
            politician_id: politician_id.to_owned(),
            canonical_name: "JULIO CESAR DOS SANTOS".to_owned(),
            distinct_cpfs: i64::try_from(cpfs.len()).unwrap(),
            cpfs: cpfs.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn exactly_the_expected_collision_proceeds() {
        let rows = [row(OLD_POLITICIAN_ID, &[CPF_STAYS, CPF_MOVES])];
        assert_eq!(classify_sweep(&rows), ScopeCheck::AsExpected);
    }

    #[test]
    fn cpf_order_does_not_matter() {
        let rows = [row(OLD_POLITICIAN_ID, &[CPF_MOVES, CPF_STAYS])];
        assert_eq!(classify_sweep(&rows), ScopeCheck::AsExpected);
    }

    #[test]
    fn empty_sweep_is_the_idempotency_signal_not_a_direct_halt() {
        // check_scope (not classify_sweep) turns this into either
        // "already applied" or a halt, via one live lookup.
        assert_eq!(classify_sweep(&[]), ScopeCheck::NoRowForOldPolitician);
    }

    #[test]
    fn more_than_one_collision_row_halts() {
        let rows = [
            row(OLD_POLITICIAN_ID, &[CPF_STAYS, CPF_MOVES]),
            row(
                "01SOMEOTHERPOLITICIAN00000",
                &["11111111111", "22222222222"],
            ),
        ];
        assert!(matches!(
            classify_sweep(&rows),
            ScopeCheck::PremiseChanged(_)
        ));
    }

    #[test]
    fn wrong_politician_id_halts() {
        let rows = [row("01SOMEOTHERPOLITICIAN00000", &[CPF_STAYS, CPF_MOVES])];
        assert!(matches!(
            classify_sweep(&rows),
            ScopeCheck::PremiseChanged(_)
        ));
    }

    #[test]
    fn different_cpfs_halt() {
        let rows = [row(OLD_POLITICIAN_ID, &["11111111111", "22222222222"])];
        assert!(matches!(
            classify_sweep(&rows),
            ScopeCheck::PremiseChanged(_)
        ));
    }

    #[test]
    fn only_one_distinct_cpf_halts() {
        // Defensive: the SQL's own `having count(...) > 1` should make this
        // impossible, but classify_sweep does not trust that — it checks the
        // actual CPF set match, not just row presence.
        let rows = [row(OLD_POLITICIAN_ID, &[CPF_STAYS])];
        assert!(matches!(
            classify_sweep(&rows),
            ScopeCheck::PremiseChanged(_)
        ));
    }
}
