//! Politician roster seeding + high-precision resolution (design §5.4:
//! "filings name their filer; rosters seeded from official member lists").
//!
//! Resolution is an EXACT match of the as-filed name (`politician_alias`)
//! joined to a mandate on `(body, district)`. Anything other than exactly one
//! hit — unknown filer, ambiguous roster — resolves to `None` and the caller
//! fails closed: review task, no Gold row (invariant 3, never guess).
//!
//! `(alias, district, body)` alone cannot tell two different real people
//! apart when they share all three (see
//! `docs/decisions/politician-identity-resolution-design.md`, built from two
//! confirmed live `br` collisions). [`resolve_hits`] additionally
//! disambiguates using an optional per-regime `external_identifier` (e.g.
//! `br`'s CPF) when the caller has one, falling back to a year-window
//! tenure-plausibility check when neither side has one. Both are `None`/
//! skipped for every regime that has no such signal (`us_house`/`us_senate`
//! today), so behavior there is unchanged.

use anyhow::Context as _;
use chrono::Datelike as _;
use govfolio_core::ids::PoliticianId;
use sqlx::PgPool;

use crate::run::RegimeBinding;

/// Generous upper bound on a single real person's political career span, in
/// years — used only when neither side of a hit has an `external_identifier`
/// to compare (design §3.4). Justified against the longest documented real
/// tenures in the bodies this project resolves against today: John Dingell
/// (US House, 59 years) and Strom Thurmond (US Senate, a comparably long
/// career) are the extreme real-world ceiling; Brazilian federal deputies'
/// documented careers are shorter. 65 years gives headroom above every real
/// case found without being so loose it stops meaning anything. Deliberately
/// NOT tight enough to catch every real collision (CARLOS ALBERTO DE SOUZA's
/// actual gap was 8 years, ordinary for a genuine re-candidacy) — this is
/// defense-in-depth for regimes with no better signal, not a general fix; see
/// the design doc's §3.4 for the honestly-acknowledged limitation.
const MAX_PLAUSIBLE_TENURE_YEARS: i32 = 65;

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
    /// Durable per-filer national/official id, when this regime's raw
    /// source data has one (design §3.2) — e.g. `br`'s CPF (falling back to
    /// the voter-registration number when CPF is masked). `None` for every
    /// regime without one (`us_house`/`us_senate` today): zero behavior
    /// change, `resolve_hits` falls back to the year-window check alone.
    pub external_identifier: Option<String>,
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
        let hits = resolve_hits(
            &mut *tx,
            regime,
            &member.filed_alias,
            &member.district,
            member.external_identifier.as_deref(),
            Some(member.active_year),
        )
        .await?;
        match hits.len() {
            1 => continue, // already seeded (same person, confirmed or plausible)
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
        sqlx::query(
            "insert into politician (id, canonical_name, external_identifier) values ($1, $2, $3)",
        )
        .bind(&politician_id)
        .bind(&member.canonical_name)
        .bind(&member.external_identifier)
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
/// `(body, district)`, disambiguated by `external_identifier`/year-window
/// when more than one real person could otherwise share that key (design
/// §3.3/§3.4). `None` unless EXACTLY one politician matches.
///
/// `external_identifier`/`as_of_year` are `None` for any regime without a
/// durable id or a filed date to compare — behavior for those calls is
/// exactly the pre-fix exact-match lookup.
///
/// # Errors
/// Database failure.
pub async fn resolve_politician(
    pool: &PgPool,
    regime: &RegimeBinding,
    filer_name: &str,
    district: &str,
    external_identifier: Option<&str>,
    as_of_year: Option<i32>,
) -> anyhow::Result<Option<String>> {
    let hits = resolve_hits(
        pool,
        regime,
        filer_name,
        district,
        external_identifier,
        as_of_year,
    )
    .await?;
    match hits.as_slice() {
        [one] => Ok(Some(one.clone())),
        _ => Ok(None), // zero or ambiguous: never guess (invariant 3)
    }
}

/// One raw `(alias, district, body)` match before disambiguation.
struct RawHit {
    politician_id: String,
    external_identifier: Option<String>,
    /// Earliest attested year across this politician's matching mandates —
    /// `mandate.start_date` is only ever seeded as an index-attested "active
    /// since at least" bound (never a real term-end), so this is the best
    /// available anchor for the year-window plausibility check.
    earliest_start_year: i32,
}

/// Resolves `(alias, district, body)` hits, then excludes any hit that is
/// provably a DIFFERENT real person from the incoming record (design §3.3):
/// a stored `external_identifier` that disagrees with the incoming one, or —
/// when neither side has one to compare — a gap between the hit's earliest
/// attested year and `as_of_year` wider than any real career plausibly spans
/// (§3.4). A hit with no stored id and no comparable year is kept
/// permissively (preserves resolution for every politician seeded before
/// this mechanism existed).
async fn resolve_hits<'e, E>(
    executor: E,
    regime: &RegimeBinding,
    filer_name: &str,
    district: &str,
    external_identifier: Option<&str>,
    as_of_year: Option<i32>,
) -> anyhow::Result<Vec<String>>
where
    E: sqlx::PgExecutor<'e>,
{
    let rows: Vec<(String, Option<String>, chrono::NaiveDate)> = sqlx::query_as(
        "select p.id, p.external_identifier, min(m.start_date) as earliest_start \
         from politician p \
         join politician_alias a on a.politician_id = p.id \
         join mandate m on m.politician_id = p.id \
         where a.alias = $1 and m.district = $2 and m.body = $3 \
         group by p.id, p.external_identifier",
    )
    .bind(filer_name)
    .bind(district)
    .bind(&regime.body)
    .fetch_all(executor)
    .await
    .with_context(|| format!("resolving politician {filer_name:?} ({district})"))?;

    let hits: Vec<RawHit> = rows
        .into_iter()
        .map(
            |(politician_id, external_identifier, earliest_start)| RawHit {
                politician_id,
                external_identifier,
                earliest_start_year: earliest_start.year(),
            },
        )
        .collect();

    Ok(hits
        .into_iter()
        .filter(|hit| is_plausible_match(hit, external_identifier, as_of_year))
        .map(|hit| hit.politician_id)
        .collect())
}

/// The per-hit keep/exclude decision (design §3.3/§3.4) — pure, unit-tested
/// directly (see this module's tests).
fn is_plausible_match(
    hit: &RawHit,
    incoming_external_identifier: Option<&str>,
    as_of_year: Option<i32>,
) -> bool {
    if let (Some(incoming), Some(stored)) = (
        incoming_external_identifier,
        hit.external_identifier.as_deref(),
    ) {
        // Both sides have an id: it is the ONLY signal that matters — a
        // match confirms the same person (skip the year-window check
        // entirely, stronger evidence); a mismatch proves a different one.
        return incoming == stored;
    }
    // At least one side has no id (a regime without one, or a politician
    // seeded before this mechanism existed) — fall back to plausibility.
    let Some(as_of_year) = as_of_year else {
        return true; // no date to compare either: permissive (unchanged pre-fix behavior)
    };
    (as_of_year - hit.earliest_start_year).abs() <= MAX_PLAUSIBLE_TENURE_YEARS
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn hit(external_identifier: Option<&str>, earliest_start_year: i32) -> RawHit {
        RawHit {
            politician_id: "01TESTPOLITICIAN00000000000".to_owned(),
            external_identifier: external_identifier.map(str::to_owned),
            earliest_start_year,
        }
    }

    /// JULIO CESAR / CARLOS ALBERTO shape: both sides have an id and they
    /// disagree — a different real person, must exclude regardless of how
    /// close the years are (design §3.3 rule 1).
    #[test]
    fn differing_external_identifiers_exclude_even_with_a_tiny_year_gap() {
        assert!(!is_plausible_match(
            &hit(Some("80673872653"), 2018),
            Some("67701124500"),
            Some(2018)
        ));
    }

    /// Matching ids confirm the same person even across an implausible year
    /// gap — an id match is stronger evidence than the year-window fallback
    /// (design §3.3 rule 2).
    #[test]
    fn matching_external_identifiers_include_even_with_a_huge_year_gap() {
        assert!(is_plausible_match(
            &hit(Some("29168317972"), 1960),
            Some("29168317972"),
            Some(2022)
        ));
    }

    /// Legacy row (seeded before this mechanism existed, `external_identifier
    /// = NULL`) plus an incoming id that can't confirm or deny — falls back
    /// to the year-window check, and a normal re-candidacy gap stays
    /// plausible (backward compatibility: nothing already resolved
    /// regresses).
    #[test]
    fn legacy_row_with_no_stored_id_falls_back_to_year_window() {
        assert!(is_plausible_match(
            &hit(None, 2014),
            Some("09867774809"),
            Some(2022)
        ));
    }

    /// CARLOS ALBERTO DE SOUZA's actual real-world gap (8 years, no id on
    /// either side) — the year-window fallback's honestly-documented
    /// limitation: it cannot catch this, only the id mechanism can (design
    /// §3.4).
    #[test]
    fn a_plausible_real_world_gap_with_no_id_on_either_side_still_matches() {
        assert!(is_plausible_match(&hit(None, 2014), None, Some(2022)));
    }

    /// An implausible gap with no id on either side fails closed rather than
    /// silently merging (design §3.4).
    #[test]
    fn an_implausible_year_gap_with_no_id_on_either_side_excludes() {
        assert!(!is_plausible_match(&hit(None, 1950), None, Some(2022)));
    }

    /// No date to compare on either side: permissive, matches the unchanged
    /// pre-fix behavior for a regime/call site that can't supply one.
    #[test]
    fn no_as_of_year_is_permissive() {
        assert!(is_plausible_match(&hit(None, 1950), None, None));
    }

    /// Exactly at the boundary is still plausible; one year past it is not —
    /// proves the threshold is inclusive and not fenced off by one, not just
    /// "some big number works".
    #[test]
    fn threshold_boundary_is_inclusive() {
        assert!(is_plausible_match(
            &hit(None, 2000),
            None,
            Some(2000 + MAX_PLAUSIBLE_TENURE_YEARS)
        ));
        assert!(!is_plausible_match(
            &hit(None, 2000),
            None,
            Some(2000 + MAX_PLAUSIBLE_TENURE_YEARS + 1)
        ));
    }
}
