//! Politician roster seeding + high-precision resolution (design §5.4:
//! "filings name their filer; rosters seeded from official member lists").
//!
//! Resolution is an EXACT match of the as-filed name (`politician_alias`)
//! joined to a mandate on `(body, district)`. Anything other than exactly one
//! hit — unknown filer, ambiguous roster — resolves to `None` and the caller
//! fails closed: review task, no Gold row (invariant 3, never guess).

use anyhow::Context as _;
use govfolio_core::ids::PoliticianId;
use sqlx::PgPool;

use crate::run::RegimeBinding;

/// One roster entry derived from an official member list (e.g. the Clerk's
/// filing-index `Member` data for `us_house`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterMember {
    /// Canonical person name (no honorific), e.g. `Nicholas Begich III`.
    pub canonical_name: String,
    /// Name exactly as filings print it, e.g. `Hon. Nicholas Begich III` —
    /// the `politician_alias` row resolution matches on.
    pub filed_alias: String,
    /// District code as filed, e.g. `AK00`.
    pub district: String,
    /// Mandate role, e.g. `Representative`.
    pub role: String,
    /// Year the member list attests activity for. Stored as the mandate's
    /// `start_date` (Jan 1) — an index-attested "active since at least" bound,
    /// NOT real tenure start; tenure refinement (Wikidata) is a later goal.
    pub active_year: i32,
}

/// Seeds politicians + mandates for members not already resolvable. Idempotent
/// by lookup (the same exact-match query as resolution), so replays insert
/// nothing. Returns how many members were newly inserted.
///
/// # Errors
/// Database failure, an ambiguous roster (two politicians already matching one
/// member — seed data corruption, fail closed), or an invalid `active_year`.
pub async fn seed_roster(
    pool: &PgPool,
    regime: &RegimeBinding,
    members: &[RosterMember],
) -> anyhow::Result<u32> {
    let mut inserted = 0u32;
    let mut tx = pool.begin().await.context("opening roster seed txn")?;
    for member in members {
        let hits = resolve_hits(&mut *tx, regime, &member.filed_alias, &member.district).await?;
        match hits.len() {
            1 => continue, // already seeded
            0 => {}
            n => anyhow::bail!(
                "roster is ambiguous for {:?} ({}): {n} politicians match — fail closed",
                member.filed_alias,
                member.district
            ),
        }
        let start_date = chrono::NaiveDate::from_ymd_opt(member.active_year, 1, 1)
            .with_context(|| format!("invalid active_year {}", member.active_year))?;
        let politician_id = PoliticianId::generate().to_string();
        sqlx::query("insert into politician (id, canonical_name) values ($1, $2)")
            .bind(&politician_id)
            .bind(&member.canonical_name)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("seeding politician {:?}", member.canonical_name))?;
        sqlx::query("insert into politician_alias (politician_id, alias) values ($1, $2)")
            .bind(&politician_id)
            .bind(&member.filed_alias)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("seeding alias {:?}", member.filed_alias))?;
        // Paper filings print the name WITHOUT the honorific (us_house quirks
        // log 2026-07-05): the member list attests both forms, so the
        // prefix-less canonical name is a legitimate as-filed alias too.
        if member.canonical_name != member.filed_alias {
            sqlx::query(
                "insert into politician_alias (politician_id, alias) values ($1, $2) \
                 on conflict do nothing",
            )
            .bind(&politician_id)
            .bind(&member.canonical_name)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("seeding canonical alias {:?}", member.canonical_name))?;
        }
        sqlx::query(
            "insert into mandate \
               (id, politician_id, jurisdiction_id, body, role, district, start_date) \
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(&politician_id)
        .bind(&regime.jurisdiction_id)
        .bind(&regime.body)
        .bind(&member.role)
        .bind(&member.district)
        .bind(start_date)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("seeding mandate for {:?}", member.canonical_name))?;
        inserted += 1;
    }
    tx.commit().await.context("committing roster seed txn")?;
    Ok(inserted)
}

/// High-precision politician resolution: exact as-filed alias + mandate on
/// `(body, district)`. `None` unless EXACTLY one politician matches.
///
/// # Errors
/// Database failure.
pub async fn resolve_politician(
    pool: &PgPool,
    regime: &RegimeBinding,
    filer_name: &str,
    district: &str,
) -> anyhow::Result<Option<String>> {
    let hits = resolve_hits(pool, regime, filer_name, district).await?;
    match hits.as_slice() {
        [one] => Ok(Some(one.clone())),
        _ => Ok(None), // zero or ambiguous: never guess (invariant 3)
    }
}

async fn resolve_hits<'e, E>(
    executor: E,
    regime: &RegimeBinding,
    filer_name: &str,
    district: &str,
) -> anyhow::Result<Vec<String>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_scalar(
        "select distinct p.id \
         from politician p \
         join politician_alias a on a.politician_id = p.id \
         join mandate m on m.politician_id = p.id \
         where a.alias = $1 and m.district = $2 and m.body = $3",
    )
    .bind(filer_name)
    .bind(district)
    .bind(&regime.body)
    .fetch_all(executor)
    .await
    .with_context(|| format!("resolving politician {filer_name:?} ({district})"))
}

/// Opens a `review_task` unless the same open task already exists — retries of
/// a fail-closed filing must not multiply tasks. Returns whether a task was
/// inserted.
///
/// # Errors
/// Database failure.
pub async fn open_review_task_once(
    pool: &PgPool,
    target_kind: &str,
    target_id: &str,
    reason: &str,
) -> anyhow::Result<bool> {
    let existing: Option<String> = sqlx::query_scalar(
        "select id from review_task \
         where target_kind = $1 and target_id = $2 and reason = $3 and status = 'open' \
         limit 1",
    )
    .bind(target_kind)
    .bind(target_id)
    .bind(reason)
    .fetch_optional(pool)
    .await
    .context("checking for an existing open review_task")?;
    if existing.is_some() {
        return Ok(false);
    }
    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, $2, $3, $4)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(target_kind)
    .bind(target_id)
    .bind(reason)
    .execute(pool)
    .await
    .with_context(|| format!("opening review_task {reason} for {target_kind}/{target_id}"))?;
    Ok(true)
}
