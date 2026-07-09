//! One-off, additive-only backfill: populates `politician.external_identifier`
//! for every `br` politician seeded BEFORE goal 093's identity-resolution fix
//! (`docs/decisions/politician-identity-resolution-design.md`) landed — every
//! one of them has `external_identifier = NULL` by construction, since the
//! column and its seed-time population logic didn't exist yet when they were
//! written.
//!
//! Why this is needed (goal 093 Phase 2 finding, not a hypothetical): the
//! design's own §3.3 rule 3 is deliberately permissive when a hit's stored
//! id is `NULL` — it falls back to the year-window check so nothing already
//! resolved regresses. But that same permissiveness means seeding a NEW
//! year's real data against the EXISTING (pre-fix, `NULL`-id) roster gets
//! the WEAK fallback every time, not the strong id-match exclusion — exactly
//! the scenario Phase 2 backfills into. The 2010 real write surfaced 3 new
//! collisions this way (`MARCOS ROBERTO DOS SANTOS`, `FRANCISCO DE ASSIS
//! NUNES`, `JOSÉ CARLOS DOS SANTOS`) that the id mechanism would have caught
//! had the pre-existing politician already carried its own CPF.
//!
//! This bin closes that gap for every politician it safely CAN (exactly one
//! distinct CPF/voter-title across all their filings — the overwhelmingly
//! common, non-colliding case) and deliberately leaves ambiguous ones alone
//! (more than one distinct identifier — an EXISTING, already-merged
//! collision that `check-br-identity-collisions` already reports and that
//! needs a targeted one-off split, not a bulk id write).
//!
//! Pure additive `UPDATE ... WHERE external_identifier IS NULL`: never
//! touches an already-set id, never merges/splits/deletes anything. Safe to
//! run repeatedly (idempotent) and safe to run before or after any future
//! real-write year.
//!
//! Dry-run by default; `--execute` writes.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin backfill-br-external-identifiers [-- --execute]
//! ```
//!
//! Env: `DATABASE_URL` (required).

use anyhow::Context as _;
use sqlx::PgPool;

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

/// One politician safely backfillable: exactly one distinct resolved
/// identifier (CPF, masked-sentinel-aware, falling back to voter-title —
/// the same selection rule as `br::binding::external_identifier`) across
/// every filing this politician has, and no id stored yet.
#[derive(sqlx::FromRow)]
struct Candidate {
    politician_id: String,
    identifier: String,
}

/// Politicians with MORE than one distinct resolved identifier — an
/// existing, already-merged collision (same class `check-br-identity-
/// collisions` reports); deliberately left untouched by this bin.
#[derive(sqlx::FromRow)]
struct Ambiguous {
    politician_id: String,
    canonical_name: String,
    distinct_identifiers: i64,
}

const SELECT_CANDIDATES: &str = "\
    with resolved as ( \
        select f.politician_id, \
               coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) as identifier \
        from filing f \
        join raw_document rd on rd.id = f.raw_document_id \
        join stg_br s on s.raw_document_id = rd.id \
        where coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) is not null \
    ), \
    per_politician as ( \
        select politician_id, count(distinct identifier) as n, min(identifier) as only_identifier \
        from resolved \
        group by politician_id \
    ) \
    select pp.politician_id, pp.only_identifier as identifier \
    from per_politician pp \
    join politician p on p.id = pp.politician_id \
    where pp.n = 1 and p.external_identifier is null";

const SELECT_AMBIGUOUS: &str = "\
    with resolved as ( \
        select f.politician_id, \
               coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) as identifier \
        from filing f \
        join raw_document rd on rd.id = f.raw_document_id \
        join stg_br s on s.raw_document_id = rd.id \
        where coalesce(nullif(s.nr_cpf_candidato, '-4'), s.nr_titulo_eleitoral_candidato) is not null \
    ) \
    select r.politician_id, p.canonical_name, count(distinct r.identifier) as distinct_identifiers \
    from resolved r \
    join politician p on p.id = r.politician_id \
    where p.external_identifier is null \
    group by r.politician_id, p.canonical_name \
    having count(distinct r.identifier) > 1";

async fn fetch_candidates(pool: &PgPool) -> anyhow::Result<Vec<Candidate>> {
    sqlx::query_as(SELECT_CANDIDATES)
        .fetch_all(pool)
        .await
        .context("selecting safely-backfillable politicians")
}

async fn fetch_ambiguous(pool: &PgPool) -> anyhow::Result<Vec<Ambiguous>> {
    sqlx::query_as(SELECT_AMBIGUOUS)
        .fetch_all(pool)
        .await
        .context("selecting ambiguous (already-collided) politicians")
}

async fn apply(pool: &PgPool, candidates: &[Candidate]) -> anyhow::Result<u64> {
    let mut updated = 0u64;
    let mut tx = pool.begin().await.context("opening backfill transaction")?;
    for candidate in candidates {
        let result = sqlx::query(
            "update politician set external_identifier = $1 \
             where id = $2 and external_identifier is null",
        )
        .bind(&candidate.identifier)
        .bind(&candidate.politician_id)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("updating politician {}", candidate.politician_id))?;
        updated += result.rows_affected();
    }
    tx.commit()
        .await
        .context("committing backfill transaction")?;
    Ok(updated)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let execute = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    println!(
        "== backfill-br-external-identifiers ({}) ==",
        if execute { "EXECUTE" } else { "DRY-RUN" }
    );

    let candidates = fetch_candidates(&pool).await?;
    let ambiguous = fetch_ambiguous(&pool).await?;

    println!(
        "{} politician(s) safely backfillable (exactly 1 distinct identifier, currently NULL)",
        candidates.len()
    );
    println!(
        "{} politician(s) left untouched (already ambiguous — >1 distinct identifier; a \
         pre-existing collision needing a targeted fix, same class check-br-identity-collisions \
         reports):",
        ambiguous.len()
    );
    for row in &ambiguous {
        println!(
            "  politician_id={} canonical_name={:?} distinct_identifiers={}",
            row.politician_id, row.canonical_name, row.distinct_identifiers
        );
    }

    if !execute {
        println!("--- DRY-RUN: nothing written. ---");
        return Ok(());
    }

    let updated = apply(&pool, &candidates).await?;
    println!("Committed: {updated} politician(s) updated.");
    let expected = u64::try_from(candidates.len()).context("candidate count overflow")?;
    anyhow::ensure!(
        updated == expected,
        "expected to update {expected} rows, actually updated {updated} — investigate before \
         trusting this run"
    );

    Ok(())
}
