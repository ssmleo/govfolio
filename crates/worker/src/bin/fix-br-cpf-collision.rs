//! General-purpose (but still narrowly scoped and safety-gated) fix for the
//! `br` politician-identity CPF-collision defect class — the same one
//! `fix-br-julio-cesar-santos-ba-2018.rs` and
//! `fix-br-carlos-alberto-souza-sp.rs` each fixed one hardcoded instance of.
//! Per those bins' own "do not generalize speculatively" instruction: this
//! generalization is now warranted, not speculative — goal 093 Phase 2's
//! 2010 real write surfaced 3 MORE instances in one pass (`MARCOS ROBERTO
//! DOS SANTOS`, `FRANCISCO DE ASSIS NUNES`, `JOSÉ CARLOS DOS SANTOS`),
//! crossing the "second case surfaces" threshold those bins' own doc
//! comments named as the point to factor shared logic out.
//!
//! Takes a `politician_id` naming the exact politician
//! `check-br-identity-collisions` (or `backfill-br-external-identifiers`'s
//! own "left untouched — ambiguous" report) flagged. Requires EXACTLY two
//! distinct resolved identifiers (CPF, masked-sentinel-aware, falling back
//! to voter-title) across that politician's filings — more than two is a
//! hard halt (needs case-by-case review, out of scope here, same
//! conservatism as the two hardcoded predecessor bins). The identifier
//! group with FEWER total `disclosure_record` rows (ties broken by fewer
//! filings, then the lexicographically larger identifier — deterministic,
//! not "which is more legitimate") moves to a freshly minted politician;
//! the larger group keeps the existing `politician_id` untouched. Handles
//! N filings per group (not just one), unlike the two predecessor bins,
//! which each only ever needed to move exactly one.
//!
//! Dry-run by default; `--execute` gates the one real transaction. Reviewed
//! before its first `--execute` run against the shared dev DB, same
//! elevated gate the two predecessor bins used.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin fix-br-cpf-collision -- --politician-id 01... [--execute]
//! ```
//!
//! Env: `DATABASE_URL` (required).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context as _;
use chrono::{NaiveDate, Utc};
use sqlx::PgPool;

use br::BrAdapter;
use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RawDocRef, RunCtx};
use pipeline::conformance::workspace_root;
use worker::backfill::{FilingBaseline, candidate_fingerprints};

struct Args {
    politician_id: String,
    execute: bool,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut politician_id = None;
    let mut execute = false;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--execute" => execute = true,
            "--politician-id" => {
                politician_id = Some(iter.next().context("--politician-id needs a value")?);
            }
            other => {
                anyhow::bail!("unknown argument {other:?} (expected --politician-id/--execute)")
            }
        }
    }
    Ok(Args {
        politician_id: politician_id.context("--politician-id is required")?,
        execute,
    })
}

// ---------------------------------------------------------------------------
// Step 0: scope this politician's filings by resolved identifier.
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow, Clone)]
struct FilingIdentifierRow {
    filing_id: String,
    raw_document_id: String,
    identifier: String,
}

const SELECT_IDENTIFIERS: &str = "\
    select f.id as filing_id, f.raw_document_id, \
           coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) as identifier \
    from filing f \
    join raw_document rd on rd.id = f.raw_document_id \
    join stg_br s on s.raw_document_id = rd.id \
    where f.politician_id = $1 \
      and coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) is not null \
    group by f.id, f.raw_document_id, s.nr_cpf_candidato, s.nr_titulo_eleitoral_candidato";

async fn fetch_identifiers(
    pool: &PgPool,
    politician_id: &str,
) -> anyhow::Result<Vec<FilingIdentifierRow>> {
    sqlx::query_as(SELECT_IDENTIFIERS)
        .bind(politician_id)
        .fetch_all(pool)
        .await
        .context("fetching this politician's filing identifiers")
}

#[derive(sqlx::FromRow)]
struct RecordCountRow {
    filing_id: String,
    n: i64,
}

async fn fetch_record_counts_by_filing(
    pool: &PgPool,
    politician_id: &str,
) -> anyhow::Result<HashMap<String, i64>> {
    let rows: Vec<RecordCountRow> = sqlx::query_as(
        "select filing_id, count(*) as n from disclosure_record where politician_id = $1 \
         group by filing_id",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .context("counting disclosure_records per filing")?;
    Ok(rows.into_iter().map(|r| (r.filing_id, r.n)).collect())
}

/// Groups filings by resolved identifier and decides which group moves.
/// Hard errors (halt, never guess) unless EXACTLY two distinct identifiers
/// are present.
fn plan_split(
    identifiers: &[FilingIdentifierRow],
    record_counts: &HashMap<String, i64>,
) -> anyhow::Result<(
    String,
    Vec<FilingIdentifierRow>,
    String,
    Vec<FilingIdentifierRow>,
)> {
    let mut groups: HashMap<String, Vec<FilingIdentifierRow>> = HashMap::new();
    for row in identifiers {
        groups
            .entry(row.identifier.clone())
            .or_default()
            .push(row.clone());
    }
    anyhow::ensure!(
        groups.len() == 2,
        "expected exactly 2 distinct identifiers for this politician, found {} ({:?}) — this \
         bin only handles the two-way split case; halt, do not guess",
        groups.len(),
        groups.keys().collect::<Vec<_>>()
    );
    let mut entries: Vec<(String, Vec<FilingIdentifierRow>)> = groups.into_iter().collect();
    let score = |filings: &[FilingIdentifierRow]| -> (i64, usize) {
        let records: i64 = filings
            .iter()
            .map(|f| record_counts.get(&f.filing_id).copied().unwrap_or(0))
            .sum();
        (records, filings.len())
    };
    entries.sort_by(|(id_a, filings_a), (id_b, filings_b)| {
        score(filings_b)
            .cmp(&score(filings_a))
            .then_with(|| id_b.cmp(id_a)) // deterministic tie-break, not "more legitimate"
    });
    let (stays_id, stays_filings) = entries.remove(0);
    let (moves_id, moves_filings) = entries.remove(0);
    Ok((stays_id, stays_filings, moves_id, moves_filings))
}

// ---------------------------------------------------------------------------
// Step 1: blast-radius rows + pre-change snapshot.
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

#[derive(sqlx::FromRow, Clone)]
struct DisclosureRecordRow {
    id: String,
    fingerprint: String,
}

async fn fetch_politician(pool: &PgPool, id: &str) -> anyhow::Result<PoliticianRow> {
    sqlx::query_as("select id, canonical_name, wikidata_qid, details from politician where id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up politician {id}"))?
        .with_context(|| format!("politician {id} not found"))
}

async fn fetch_alias(pool: &PgPool, politician_id: &str) -> anyhow::Result<AliasRow> {
    let rows: Vec<AliasRow> = sqlx::query_as(
        "select politician_id, alias, lang from politician_alias where politician_id = $1",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up politician_alias rows for {politician_id}"))?;
    anyhow::ensure!(
        rows.len() == 1,
        "expected exactly 1 politician_alias row for {politician_id}, found {} — halt",
        rows.len()
    );
    rows.into_iter().next().context("unreachable")
}

async fn fetch_mandate(pool: &PgPool, politician_id: &str) -> anyhow::Result<MandateRow> {
    let rows: Vec<MandateRow> = sqlx::query_as(
        "select id, politician_id, jurisdiction_id, body, role, party, district, start_date, \
         end_date from mandate where politician_id = $1",
    )
    .bind(politician_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("looking up mandate rows for {politician_id}"))?;
    anyhow::ensure!(
        rows.len() == 1,
        "expected exactly 1 mandate row for {politician_id}, found {} — halt",
        rows.len()
    );
    rows.into_iter().next().context("unreachable")
}

async fn fetch_records_for_filing(
    pool: &PgPool,
    filing_id: &str,
) -> anyhow::Result<Vec<DisclosureRecordRow>> {
    sqlx::query_as("select id, fingerprint from disclosure_record where filing_id = $1")
        .bind(filing_id)
        .fetch_all(pool)
        .await
        .with_context(|| format!("looking up disclosure_record rows for filing {filing_id}"))
}

async fn fetch_raw_document_sha256(pool: &PgPool, raw_document_id: &str) -> anyhow::Result<String> {
    sqlx::query_scalar("select sha256 from raw_document where id = $1")
        .bind(raw_document_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up raw_document {raw_document_id}"))?
        .with_context(|| format!("raw_document {raw_document_id} not found"))
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

fn write_snapshot(
    dir: &Path,
    politician: &PoliticianRow,
    alias: &AliasRow,
    mandate: &MandateRow,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    write_csv(
        &dir.join("snapshot_politician.csv"),
        &["id", "canonical_name", "wikidata_qid", "details"],
        vec![vec![
            politician.id.clone(),
            politician.canonical_name.clone(),
            opt(politician.wikidata_qid.as_ref()),
            politician.details.to_string(),
        ]],
    )?;
    write_csv(
        &dir.join("snapshot_alias.csv"),
        &["politician_id", "alias", "lang"],
        vec![vec![
            alias.politician_id.clone(),
            alias.alias.clone(),
            opt(alias.lang.as_ref()),
        ]],
    )?;
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
        vec![vec![
            mandate.id.clone(),
            mandate.politician_id.clone(),
            mandate.jurisdiction_id.clone(),
            mandate.body.clone(),
            mandate.role.clone(),
            opt(mandate.party.as_ref()),
            opt(mandate.district.as_ref()),
            mandate.start_date.to_string(),
            opt(mandate.end_date.as_ref()),
        ]],
    )?;
    println!("  snapshot -> {}", dir.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Step 2/3: re-derive from Bronze per moving filing, self-check, corrected
// fingerprints.
// ---------------------------------------------------------------------------

struct MovingFiling {
    filing_id: String,
    records: Vec<DisclosureRecordRow>,
    old_to_new_fingerprint: Vec<(String, String)>,
}

async fn rederive_one_filing(
    pool: &PgPool,
    old_politician_id: &str,
    new_politician_id: &str,
    mandate_body: &str,
    filing_row: &FilingIdentifierRow,
) -> anyhow::Result<MovingFiling> {
    let records = fetch_records_for_filing(pool, &filing_row.filing_id).await?;
    let raw_document_sha256 = fetch_raw_document_sha256(pool, &filing_row.raw_document_id).await?;

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
        .with_context(|| format!("re-parsing filing {}", filing_row.filing_id))?;
    let candidates: Vec<GoldCandidate> = adapter
        .normalize(&staging_rows, &ctx)
        .await
        .with_context(|| format!("re-normalizing filing {}", filing_row.filing_id))?;
    anyhow::ensure!(
        candidates.len() == records.len(),
        "filing {}: expected {} re-derived candidate(s), got {}",
        filing_row.filing_id,
        records.len(),
        candidates.len()
    );

    let regime_id = if mandate_body == br::seed::BODY_SENADO {
        br::seed::REGIME_ID_SENADO.to_owned()
    } else {
        br::seed::REGIME_ID.to_owned()
    };
    let old_baseline = FilingBaseline {
        filing_id: filing_row.filing_id.clone(),
        politician_id: old_politician_id.to_owned(),
        regime_id: regime_id.clone(),
        fingerprints: HashSet::new(),
    };
    let rederived = candidate_fingerprints("br", &old_baseline, &candidates)
        .with_context(|| format!("recomputing OLD fingerprints for {}", filing_row.filing_id))?;
    let mut stored: Vec<&str> = records.iter().map(|r| r.fingerprint.as_str()).collect();
    stored.sort_unstable();
    let mut got: Vec<&str> = rederived.iter().map(String::as_str).collect();
    got.sort_unstable();
    anyhow::ensure!(
        got == stored,
        "filing {}: re-derived fingerprints {got:?} do not match stored {stored:?} — halt, the \
         re-derivation is not proven exact",
        filing_row.filing_id
    );

    let new_baseline = FilingBaseline {
        politician_id: new_politician_id.to_owned(),
        ..old_baseline
    };
    let new_fps = candidate_fingerprints("br", &new_baseline, &candidates)
        .with_context(|| format!("computing NEW fingerprints for {}", filing_row.filing_id))?;
    let old_to_new_fingerprint: Vec<(String, String)> = records
        .iter()
        .map(|r| r.fingerprint.clone())
        .zip(new_fps)
        .collect();

    Ok(MovingFiling {
        filing_id: filing_row.filing_id.clone(),
        records,
        old_to_new_fingerprint,
    })
}

async fn rederive_all_moving(
    pool: &PgPool,
    old_politician_id: &str,
    new_politician_id: &str,
    mandate_body: &str,
    moves_filings: &[FilingIdentifierRow],
) -> anyhow::Result<Vec<MovingFiling>> {
    println!("Step 2/3: re-deriving + checking each moving filing from Bronze...");
    let mut moving = Vec::new();
    for filing_row in moves_filings {
        let m = rederive_one_filing(
            pool,
            old_politician_id,
            new_politician_id,
            mandate_body,
            filing_row,
        )
        .await?;
        println!(
            "  filing {} OK: {} record(s) re-derived and matched",
            m.filing_id,
            m.records.len()
        );
        moving.push(m);
    }
    Ok(moving)
}

fn print_dry_run_plan(
    new_politician_id: &str,
    new_mandate_id: &str,
    alias: &str,
    moving: &[MovingFiling],
) {
    println!("--- DRY-RUN: nothing written. Would mint politician {new_politician_id} ---");
    println!("  (mandate {new_mandate_id}, alias {alias:?})");
    for m in moving {
        println!(
            "  UPDATE filing {} -> politician_id {new_politician_id}",
            m.filing_id
        );
        for record in &m.records {
            println!("    UPDATE disclosure_record {}", record.id);
        }
    }
}

// ---------------------------------------------------------------------------
// Steps 4-6: the one write transaction (--execute only).
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn apply_fix(
    pool: &PgPool,
    old_politician_id: &str,
    politician: &PoliticianRow,
    alias: &AliasRow,
    mandate: &MandateRow,
    new_politician_id: &str,
    new_mandate_id: &str,
    stays_identifier: &str,
    moves_identifier: &str,
    moving: &[MovingFiling],
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await.context("opening the fix transaction")?;

    // The old politician's own id was left NULL by construction (it was
    // ambiguous — >1 distinct identifier — until this split resolves it);
    // set it now so a future year's real write against this SAME politician
    // gets the strong id-match path immediately, not another fallback-only
    // round (goal 093 Phase 2 finding: this is exactly the gap that let
    // these collisions happen in the first place).
    sqlx::query("update politician set external_identifier = $1 where id = $2 and external_identifier is null")
        .bind(stays_identifier)
        .bind(old_politician_id)
        .execute(&mut *tx)
        .await
        .context("Step 4: setting the staying politician's external_identifier")?;

    sqlx::query(
        "insert into politician (id, canonical_name, wikidata_qid, external_identifier, details) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(new_politician_id)
    .bind(&politician.canonical_name)
    .bind(&politician.wikidata_qid)
    .bind(moves_identifier)
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

    for filing in moving {
        let filing_result = sqlx::query(
            "update filing set politician_id = $1 where id = $2 and politician_id = $3",
        )
        .bind(new_politician_id)
        .bind(&filing.filing_id)
        .bind(old_politician_id)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Step 5: repointing filing {}", filing.filing_id))?;
        anyhow::ensure!(
            filing_result.rows_affected() == 1,
            "Step 5: expected to repoint exactly 1 filing row ({}), affected {} — a concurrent \
             write raced this transaction; aborting rather than proceeding blind",
            filing.filing_id,
            filing_result.rows_affected()
        );

        for record in &filing.records {
            let new_fingerprint = filing
                .old_to_new_fingerprint
                .iter()
                .find(|(old, _)| old == &record.fingerprint)
                .map(|(_, new)| new.as_str())
                .with_context(|| format!("no corrected fingerprint for record {}", record.id))?;
            let record_result = sqlx::query(
                "update disclosure_record set politician_id = $1, fingerprint = $2 \
                 where id = $3 and politician_id = $4 and fingerprint = $5",
            )
            .bind(new_politician_id)
            .bind(new_fingerprint)
            .bind(&record.id)
            .bind(old_politician_id)
            .bind(&record.fingerprint)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Step 6: repointing disclosure_record {}", record.id))?;
            anyhow::ensure!(
                record_result.rows_affected() == 1,
                "Step 6: expected to repoint exactly 1 disclosure_record row ({}), affected {} \
                 — aborting rather than proceeding blind",
                record.id,
                record_result.rows_affected()
            );
        }
    }

    tx.commit()
        .await
        .context("committing the fix transaction")?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool)
        .await
        .context("applying migrations")?;

    println!(
        "== fix-br-cpf-collision politician_id={} ({}) ==",
        args.politician_id,
        if args.execute { "EXECUTE" } else { "DRY-RUN" }
    );

    println!("Step 0: scoping this politician's filings by resolved identifier...");
    let identifiers = fetch_identifiers(&pool, &args.politician_id).await?;
    anyhow::ensure!(
        !identifiers.is_empty(),
        "no CPF/voter-title data found for politician {} — nothing to split",
        args.politician_id
    );
    let record_counts = fetch_record_counts_by_filing(&pool, &args.politician_id).await?;
    let (stays_id, stays_filings, moves_id, moves_filings) =
        plan_split(&identifiers, &record_counts)?;
    println!(
        "  identifier {stays_id:?} stays ({} filing(s)); identifier {moves_id:?} moves ({} \
         filing(s))",
        stays_filings.len(),
        moves_filings.len()
    );

    let politician = fetch_politician(&pool, &args.politician_id).await?;
    let alias = fetch_alias(&pool, &args.politician_id).await?;
    let mandate = fetch_mandate(&pool, &args.politician_id).await?;
    let snapshot_dir = workspace_root().join("target").join(format!(
        "fix-br-cpf-collision-{}-snapshot-{}",
        args.politician_id,
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    write_snapshot(&snapshot_dir, &politician, &alias, &mandate)?;

    let new_politician_id = govfolio_core::ids::PoliticianId::generate().to_string();
    let new_mandate_id = ulid::Ulid::new().to_string();
    let moving = rederive_all_moving(
        &pool,
        &args.politician_id,
        &new_politician_id,
        &mandate.body,
        &moves_filings,
    )
    .await?;

    if !args.execute {
        print_dry_run_plan(&new_politician_id, &new_mandate_id, &alias.alias, &moving);
        return Ok(());
    }

    apply_fix(
        &pool,
        &args.politician_id,
        &politician,
        &alias,
        &mandate,
        &new_politician_id,
        &new_mandate_id,
        &stays_id,
        &moves_id,
        &moving,
    )
    .await?;
    println!(
        "Committed: politician {new_politician_id} minted; {} filing(s) repointed.",
        moving.len()
    );

    let remaining = fetch_identifiers(&pool, &args.politician_id).await?;
    let remaining_distinct: HashSet<&str> =
        remaining.iter().map(|r| r.identifier.as_str()).collect();
    anyhow::ensure!(
        remaining_distinct.len() <= 1,
        "Step 7: {} still shows {} distinct identifiers after the fix — investigate before \
         trusting this run",
        args.politician_id,
        remaining_distinct.len()
    );
    println!(
        "Step 7 OK: {} now shows <=1 distinct identifier.",
        args.politician_id
    );

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn row(filing_id: &str, raw_document_id: &str, identifier: &str) -> FilingIdentifierRow {
        FilingIdentifierRow {
            filing_id: filing_id.to_owned(),
            raw_document_id: raw_document_id.to_owned(),
            identifier: identifier.to_owned(),
        }
    }

    #[test]
    fn plan_split_moves_the_group_with_fewer_records() {
        let identifiers = vec![row("f1", "d1", "CPF_A"), row("f2", "d2", "CPF_B")];
        let mut counts = HashMap::new();
        counts.insert("f1".to_owned(), 8);
        counts.insert("f2".to_owned(), 3);
        let (stays_id, stays, moves_id, moves) = plan_split(&identifiers, &counts).unwrap();
        assert_eq!(stays_id, "CPF_A");
        assert_eq!(stays.len(), 1);
        assert_eq!(moves_id, "CPF_B");
        assert_eq!(moves.len(), 1);
    }

    #[test]
    fn plan_split_handles_multiple_filings_per_group() {
        let identifiers = vec![
            row("f1", "d1", "CPF_A"),
            row("f2", "d2", "CPF_B"),
            row("f3", "d3", "CPF_B"),
        ];
        let mut counts = HashMap::new();
        counts.insert("f1".to_owned(), 1);
        counts.insert("f2".to_owned(), 1);
        counts.insert("f3".to_owned(), 1);
        let (stays_id, stays, moves_id, moves) = plan_split(&identifiers, &counts).unwrap();
        assert_eq!(
            stays_id, "CPF_B",
            "more filings wins the tie-break on record count"
        );
        assert_eq!(stays.len(), 2);
        assert_eq!(moves_id, "CPF_A");
        assert_eq!(moves.len(), 1);
    }

    #[test]
    fn plan_split_halts_on_more_than_two_distinct_identifiers() {
        let identifiers = vec![
            row("f1", "d1", "CPF_A"),
            row("f2", "d2", "CPF_B"),
            row("f3", "d3", "CPF_C"),
        ];
        let counts = HashMap::new();
        assert!(plan_split(&identifiers, &counts).is_err());
    }

    #[test]
    fn plan_split_halts_on_only_one_distinct_identifier() {
        let identifiers = vec![row("f1", "d1", "CPF_A"), row("f2", "d2", "CPF_A")];
        let counts = HashMap::new();
        assert!(plan_split(&identifiers, &counts).is_err());
    }
}
