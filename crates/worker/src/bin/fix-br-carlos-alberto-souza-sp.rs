//! One-off, narrowly-scoped fix for the second live `br` politician-identity
//! collision, found via `check-br-identity-collisions` after the 2014 real
//! write (`agents/JOURNAL.md` 2026-07-09) and grounded in
//! `docs/decisions/politician-identity-resolution-design.md` §4 (which
//! explicitly adopts `docs/decisions/br-identity-collision-remediation.md`'s
//! JULIO CESAR fix as its template — read both before touching this file).
//!
//! Two real people, both named `CARLOS ALBERTO DE SOUZA`, both `DEPUTADO
//! FEDERAL` candidates in São Paulo (`SP`) 8 years apart (2014 and 2022),
//! currently share one `politician` row (`01KWXA32E7PMQ6D7CBEZJWCA9F`). This
//! fixes EXACTLY that one case: it mints a fresh `politician`/
//! `politician_alias`/`mandate` for the smaller side (CPF `09867774809`, the
//! 2022 filing, 3 `disclosure_record`s) and repoints that filing's and
//! records' foreign keys onto it. The larger side (CPF `29168317972`, the
//! 2014 filing, 8 `disclosure_record`s) keeps the existing `politician_id`
//! untouched — zero rows move for that person (design §4 / plan §5's
//! row-count tie-breaker).
//!
//! Unlike JULIO CESAR's fix (exactly 1 moving `disclosure_record`), this
//! case's moving filing carries 3 — the only structural difference from the
//! `fix-br-julio-cesar-santos-ba-2018.rs` template; every other step mirrors
//! it field for field. Deliberately NOT a general "fix any collision" tool —
//! do not generalize this file.
//!
//! Dry-run by default; `--execute` gates the one real transaction. Per the
//! same elevated review-before-run gate the JULIO CESAR fix used for the
//! first-ever politician-identity split, this file must be independently
//! reviewed before its first `--execute` run against the shared dev DB.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin fix-br-carlos-alberto-souza-sp [-- --execute]
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

// --- narrow, case-specific constants (do NOT generalize) ---

/// The shared `politician.id` both real people currently sit under.
const OLD_POLITICIAN_ID: &str = "01KWXA32E7PMQ6D7CBEZJWCA9F";
/// CPF of the person who KEEPS `OLD_POLITICIAN_ID` (2014 filing, 8
/// `disclosure_record`s — the larger side; zero rows move for this person).
const CPF_STAYS: &str = "29168317972";
/// CPF of the person who gets a freshly minted `politician_id` (2022 filing,
/// 3 `disclosure_record`s — the smaller side).
const CPF_MOVES: &str = "09867774809";
/// The one `filing.id` that moves to the new politician (the 2022 filing).
const MOVING_FILING_ID: &str = "01KWXBGA27C4D6MSQHXBXAGHNB";

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
// Step 0: the standing CPF-collision sweep (read-only) — same SQL as
// `check-br-identity-collisions.rs`/the JULIO CESAR plan §2.
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SweepRow {
    politician_id: String,
    canonical_name: String,
    distinct_cpfs: i64,
    cpfs: Vec<String>,
}

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
        .context("running the CPF-collision sweep")
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

/// What the Step 0 sweep implies about whether it is safe to proceed — same
/// shape as the JULIO CESAR fix's own `ScopeCheck`.
#[derive(Debug, PartialEq, Eq)]
enum ScopeCheck {
    AsExpected,
    NoRowForOldPolitician,
    PremiseChanged(String),
}

fn classify_sweep(rows: &[SweepRow]) -> ScopeCheck {
    let matching: Vec<&SweepRow> = rows
        .iter()
        .filter(|row| row.politician_id == OLD_POLITICIAN_ID)
        .collect();
    if matching.is_empty() {
        return ScopeCheck::NoRowForOldPolitician;
    }
    if matching.len() > 1 || rows.len() != 1 {
        return ScopeCheck::PremiseChanged(format!(
            "sweep returned {} collision row(s) total ({} for {OLD_POLITICIAN_ID}); expected \
             exactly 1 total, exactly 1 for this politician",
            rows.len(),
            matching.len()
        ));
    }
    let row = matching[0];
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

enum ScopeOutcome {
    Proceed,
    AlreadyApplied { moved_to: String },
}

async fn check_scope(pool: &PgPool, sweep: &[SweepRow]) -> anyhow::Result<ScopeOutcome> {
    match classify_sweep(sweep) {
        ScopeCheck::AsExpected => Ok(ScopeOutcome::Proceed),
        ScopeCheck::PremiseChanged(reason) => anyhow::bail!(
            "Step 0 HALT: {reason} — the premise this fix was built on has changed; do not \
             proceed on a stale assumption."
        ),
        ScopeCheck::NoRowForOldPolitician => {
            match fetch_filing_politician_id(pool, MOVING_FILING_ID).await? {
                None => anyhow::bail!(
                    "Step 0 HALT: sweep is empty for {OLD_POLITICIAN_ID} AND filing \
                     {MOVING_FILING_ID} was not found — cannot tell an already-applied fix \
                     from a vanished filing; do not proceed."
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
// Step 1: blast-radius rows + pre-change snapshot.
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct PoliticianRow {
    id: String,
    canonical_name: String,
    wikidata_qid: Option<String>,
    external_identifier: Option<String>,
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

#[derive(sqlx::FromRow, Clone)]
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

struct BlastRadius {
    politician: PoliticianRow,
    aliases: Vec<AliasRow>,
    mandates: Vec<MandateRow>,
    filings: Vec<FilingRow>,
    records: Vec<DisclosureRecordRow>,
}

async fn fetch_politician(pool: &PgPool, id: &str) -> anyhow::Result<PoliticianRow> {
    sqlx::query_as(
        "select id, canonical_name, wikidata_qid, external_identifier, details from politician \
         where id = $1",
    )
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

/// Loads + sanity-checks every row for `OLD_POLITICIAN_ID`: exactly 1
/// `politician_alias`, 1 mandate, 2 filings, 11 `disclosure_record`s (8
/// staying + 3 moving). Any count mismatch halts — the premise changed.
async fn load_blast_radius(pool: &PgPool) -> anyhow::Result<BlastRadius> {
    println!("Step 1: loading the blast-radius rows for {OLD_POLITICIAN_ID}...");
    let politician = fetch_politician(pool, OLD_POLITICIAN_ID).await?;
    let aliases = fetch_aliases(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        aliases.len() == 1,
        "expected exactly 1 politician_alias row for {OLD_POLITICIAN_ID}, found {}",
        aliases.len()
    );
    let mandates = fetch_mandates(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        mandates.len() == 1,
        "expected exactly 1 mandate row for {OLD_POLITICIAN_ID}, found {}",
        mandates.len()
    );
    let filings = fetch_filings_for_politician(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        filings.len() == 2,
        "expected exactly 2 filing rows for {OLD_POLITICIAN_ID}, found {}",
        filings.len()
    );
    let records = fetch_records_for_politician(pool, OLD_POLITICIAN_ID).await?;
    anyhow::ensure!(
        records.len() == 11,
        "expected exactly 11 disclosure_record rows for {OLD_POLITICIAN_ID} (8+3), found {}",
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
        &[
            "id",
            "canonical_name",
            "wikidata_qid",
            "external_identifier",
            "details",
        ],
        vec![vec![
            politician.id.clone(),
            politician.canonical_name.clone(),
            opt(politician.wikidata_qid.as_ref()),
            opt(politician.external_identifier.as_ref()),
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

/// Writes the pre-change snapshot: one CSV per table, scoped exactly like
/// the JULIO CESAR fix's own step 1 — a real, inspectable artifact on disk
/// before any write happens.
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

/// Finds the one moving filing + its (possibly several) `disclosure_record`s
/// inside the already-loaded blast radius — unlike JULIO CESAR's fix, this
/// case's moving filing carries 3 records, not 1.
fn find_moving(radius: &BlastRadius) -> anyhow::Result<(&FilingRow, Vec<&DisclosureRecordRow>)> {
    let filing = radius
        .filings
        .iter()
        .find(|f| f.id == MOVING_FILING_ID)
        .with_context(|| {
            format!("filing {MOVING_FILING_ID} not found among {OLD_POLITICIAN_ID}'s filings")
        })?;
    anyhow::ensure!(
        filing.regime_id == br::seed::REGIME_ID,
        "moving filing {MOVING_FILING_ID} has regime_id {}, expected the Câmara regime {}",
        filing.regime_id,
        br::seed::REGIME_ID
    );
    let moving_records: Vec<&DisclosureRecordRow> = radius
        .records
        .iter()
        .filter(|r| r.filing_id == MOVING_FILING_ID)
        .collect();
    anyhow::ensure!(
        moving_records.len() == 3,
        "expected exactly 3 disclosure_record rows for the moving filing {MOVING_FILING_ID}, \
         found {}",
        moving_records.len()
    );
    Ok((filing, moving_records))
}

// ---------------------------------------------------------------------------
// Steps 2/3: re-derive from Bronze, self-check, then corrected fingerprints.
// ---------------------------------------------------------------------------

/// Re-derives the moving filing's candidates from Bronze (via sha256, NOT
/// `raw_document.storage_uri` — this row's `storage_uri` still points at the
/// old, since-deleted `%TEMP%\govfolio-backfill-real-br-17764\...` path per
/// the INCIDENT RESOLVED journal entry; the real bytes live under
/// `target/bronze-backfill-real-br` post-recovery), then asserts each
/// re-derived fingerprint (under the OLD `politician_id`) matches the LIVE
/// stored value exactly for EVERY moving record — a mismatch on any one is a
/// hard halt.
async fn rederive_and_check(
    pool: &PgPool,
    filing: &FilingRow,
    records: &[&DisclosureRecordRow],
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
        candidates.len() == records.len(),
        "expected {} re-derived GoldCandidate(s) for {}, got {}",
        records.len(),
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
        .context("recomputing fingerprints under the OLD politician_id")?;
    anyhow::ensure!(
        rederived.len() == records.len(),
        "expected {} recomputed fingerprint(s), got {}",
        records.len(),
        rederived.len()
    );
    let mut stored: Vec<&str> = records.iter().map(|r| r.fingerprint.as_str()).collect();
    stored.sort_unstable();
    let mut got: Vec<&str> = rederived.iter().map(String::as_str).collect();
    got.sort_unstable();
    anyhow::ensure!(
        got == stored,
        "Step 2 HALT: re-derived fingerprints {got:?} do not match the currently stored \
         disclosure_record fingerprints {stored:?} for {} — the re-derivation is not proven \
         exact; nothing downstream may be trusted.",
        filing.id
    );
    println!("Step 2 OK: re-derived fingerprints match the stored fingerprints (set-equal).");
    Ok((candidates, old_baseline))
}

/// The corrected fingerprints under a freshly minted `politician_id`
/// (generated at RUN TIME, never hardcoded), keyed by the OLD fingerprint so
/// each moving record can be matched to its own new value.
fn compute_new_fingerprints(
    candidates: &[GoldCandidate],
    old_baseline: FilingBaseline,
) -> anyhow::Result<(String, String, Vec<String>)> {
    let new_politician_id = govfolio_core::ids::PoliticianId::generate().to_string();
    let new_mandate_id = ulid::Ulid::new().to_string();
    let new_baseline = FilingBaseline {
        politician_id: new_politician_id.clone(),
        ..old_baseline
    };
    let new_fps = candidate_fingerprints("br", &new_baseline, candidates)
        .context("computing the corrected fingerprints under the new politician_id")?;
    anyhow::ensure!(
        new_fps.len() == candidates.len(),
        "expected {} corrected fingerprint(s), got {}",
        candidates.len(),
        new_fps.len()
    );
    println!(
        "Step 3: new politician_id {new_politician_id} (mandate {new_mandate_id}) -> {} \
         corrected fingerprint(s)",
        new_fps.len()
    );
    Ok((new_politician_id, new_mandate_id, new_fps))
}

// ---------------------------------------------------------------------------
// Steps 4-6: the one write transaction (--execute only).
// ---------------------------------------------------------------------------

/// Mints the new politician/alias/mandate rows (byte-identical copies of the
/// old politician's), repoints the moving filing, then repoints EVERY
/// moving `disclosure_record` (3 here, vs. JULIO CESAR's 1) by matching each
/// old fingerprint to its corrected replacement.
///
/// Idempotency: `check_scope` (called before this function) already proved
/// the sweep matched exactly the expected collision and `filing.
/// politician_id` was still `OLD_POLITICIAN_ID` — a second invocation never
/// reaches here (short-circuits at Step 0's `AlreadyApplied`). The `WHERE`
/// guards below are defense-in-depth: `rows_affected() != expected` past
/// this point means a concurrent writer raced this transaction.
#[allow(clippy::too_many_arguments)]
async fn apply_fix(
    pool: &PgPool,
    radius: &BlastRadius,
    filing: &FilingRow,
    records: &[&DisclosureRecordRow],
    new_politician_id: &str,
    new_mandate_id: &str,
    old_to_new_fingerprint: &[(String, String)],
) -> anyhow::Result<()> {
    let politician = &radius.politician;
    let alias = &radius.aliases[0];
    let mandate = &radius.mandates[0];

    let mut tx = pool.begin().await.context("opening the fix transaction")?;

    sqlx::query(
        "insert into politician (id, canonical_name, wikidata_qid, external_identifier, details) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(new_politician_id)
    .bind(&politician.canonical_name)
    .bind(&politician.wikidata_qid)
    .bind(CPF_MOVES)
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

    for record in records {
        let new_fingerprint = old_to_new_fingerprint
            .iter()
            .find(|(old, _)| old == &record.fingerprint)
            .map(|(_, new)| new.as_str())
            .with_context(|| {
                format!(
                    "no corrected fingerprint computed for record {} (old fingerprint {})",
                    record.id, record.fingerprint
                )
            })?;
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
        .with_context(|| format!("Step 6: repointing disclosure_record {}", record.id))?;
        anyhow::ensure!(
            record_result.rows_affected() == 1,
            "Step 6: expected to repoint exactly 1 disclosure_record row ({}), affected {} — a \
             concurrent write raced this transaction since Step 0; aborting rather than \
             proceeding blind",
            record.id,
            record_result.rows_affected()
        );
    }

    tx.commit()
        .await
        .context("committing the fix transaction")?;
    println!(
        "Steps 4-6 committed: politician {new_politician_id} minted; filing {} and {} \
         disclosure_record(s) repointed.",
        filing.id,
        records.len()
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
        "== fix-br-carlos-alberto-souza-sp ({}) ==",
        if execute { "EXECUTE" } else { "DRY-RUN" }
    );
    println!("Step 0: re-running the CPF-collision sweep (read-only)...");
    let sweep = run_sweep(&pool).await?;
    print_sweep(&sweep);

    if let ScopeOutcome::AlreadyApplied { moved_to } = check_scope(&pool, &sweep).await? {
        println!(
            "Step 0: already applied — filing {MOVING_FILING_ID} is already repointed to \
             {moved_to}. No-op (idempotency)."
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
        "fix-br-carlos-alberto-souza-sp-snapshot-{}",
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    write_snapshot(&snapshot_dir, &radius)?;
    println!(
        "Step 1: pre-change snapshot written to {}",
        snapshot_dir.display()
    );

    let (filing, records) = find_moving(&radius)?;
    let (candidates, old_baseline) = rederive_and_check(&pool, filing, &records).await?;
    let (new_politician_id, new_mandate_id, new_fingerprints) =
        compute_new_fingerprints(&candidates, old_baseline)?;
    let old_to_new_fingerprint: Vec<(String, String)> = records
        .iter()
        .map(|r| r.fingerprint.clone())
        .zip(new_fingerprints)
        .collect();

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
        for (old_fp, new_fp) in &old_to_new_fingerprint {
            println!(
                "  UPDATE disclosure_record (fingerprint {old_fp}) -> politician_id {new_politician_id}, fingerprint {new_fp}"
            );
        }
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
        &records,
        &new_politician_id,
        &new_mandate_id,
        &old_to_new_fingerprint,
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

    fn row(politician_id: &str, cpfs: &[&str]) -> SweepRow {
        SweepRow {
            politician_id: politician_id.to_owned(),
            canonical_name: "CARLOS ALBERTO DE SOUZA".to_owned(),
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
        assert_eq!(classify_sweep(&[]), ScopeCheck::NoRowForOldPolitician);
    }

    /// A DIFFERENT, unrelated collision existing elsewhere must not block
    /// this fix — only THIS politician's own row matters.
    #[test]
    fn an_unrelated_collision_elsewhere_does_not_block() {
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

    /// No row at all for `OLD_POLITICIAN_ID` — ambiguous on its own
    /// (`check_scope`'s live filing lookup disambiguates already-applied
    /// from something-else-changed; `classify_sweep` alone cannot).
    #[test]
    fn no_row_for_old_politician_is_not_a_direct_halt() {
        let rows = [row("01SOMEOTHERPOLITICIAN00000", &[CPF_STAYS, CPF_MOVES])];
        assert_eq!(classify_sweep(&rows), ScopeCheck::NoRowForOldPolitician);
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
        let rows = [row(OLD_POLITICIAN_ID, &[CPF_STAYS])];
        assert!(matches!(
            classify_sweep(&rows),
            ScopeCheck::PremiseChanged(_)
        ));
    }
}
