//! Atomic jurisdiction lease (goal 097) — implements the claim path that
//! `docs/runbooks/parallel-factory.md` pre-check 1 required before running
//! N>1 loop workers: claiming a jurisdiction is a SINGLE statement; a
//! SELECT-then-UPDATE races and two lanes grab the same source.
//!
//! `FOR UPDATE SKIP LOCKED` in the claim subquery is load-bearing: without
//! it, a concurrent claim blocks on the winner's row lock and — under READ
//! COMMITTED re-check semantics — re-evaluates only the outer `j.id = pick.id`
//! predicate after the winner commits, silently overwriting the winner's
//! lease. With it, a locked row is passed over entirely.
//!
//! Lease semantics (source-exploration.md conventions):
//! - one jurisdiction lease per lane; `claim_next` prefers resuming the
//!   lane's own in-flight row (renewing `claimed_at` as the heartbeat)
//!   before taking anything new, so a lane can never accumulate two leases
//!   through the claim path;
//! - a lease older than 24h is stale and free for any lane to reclaim;
//! - `advance` moves `coverage_phase` at an intermediate phase boundary and
//!   KEEPS the lease; `release` clears it (optionally advancing to a final
//!   phase or blocking with a reason).

use anyhow::{Context as _, bail};
use chrono::{DateTime, Utc};

/// Phases a holder may advance/release a row to. `stub` is the seed state
/// (never a target) and `blocked` only enters through [`Disposition::Block`],
/// which also records `blocked_reason`.
const ADVANCE_PHASES: &[&str] = &["scouted", "surveyed", "sampled", "specced", "built", "live"];

/// A successfully claimed registry row.
#[derive(Debug, sqlx::FromRow)]
pub struct Lease {
    pub id: String,
    pub coverage_phase: String,
    pub epoch: Option<i16>,
    pub priority_score: Option<f32>,
}

/// One live lease, as reported by [`status`].
#[derive(Debug, sqlx::FromRow)]
pub struct LeaseStatus {
    pub id: String,
    pub coverage_phase: String,
    pub claimed_by: String,
    pub claimed_at: DateTime<Utc>,
}

/// How a lease ends.
#[derive(Debug)]
pub enum Disposition {
    /// Clear the lease, leave `coverage_phase` untouched.
    Keep,
    /// Clear the lease and advance to a final phase (validated against the
    /// phase contract).
    Advance(String),
    /// Clear the lease, set `coverage_phase = 'blocked'` + the reason.
    Block(String),
}

fn validate_phase(phase: &str) -> anyhow::Result<()> {
    if !ADVANCE_PHASES.contains(&phase) {
        bail!(
            "phase {phase:?} is outside the advance contract {ADVANCE_PHASES:?} \
             (blocked goes through release --block <reason>)"
        );
    }
    Ok(())
}

/// Atomically claim the best claimable jurisdiction in `epoch` for `me`:
/// own in-flight lease first (resume + heartbeat renew), then unclaimed or
/// stale (>24h) rows by `priority_score`. Returns `None` when nothing in the
/// epoch is claimable — the caller stops, fail closed.
///
/// # Errors
/// Propagates the Postgres error when the claim statement fails.
pub async fn claim_next<'e, E>(exec: E, me: &str, epoch: i16) -> anyhow::Result<Option<Lease>>
where
    E: sqlx::PgExecutor<'e>,
{
    // `IS NOT DISTINCT FROM` (not `=`) in the ORDER BY: `claimed_by = $1` is
    // NULL for unclaimed rows, and DESC sorts NULLs first — which would rank
    // every unclaimed row above the lane's own lease and hand it a second one.
    sqlx::query_as(
        "update jurisdiction j
         set claimed_by = $1, claimed_at = now()
         from (
           select id from jurisdiction
           where epoch = $2
             and coverage_phase not in ('live', 'blocked')
             and (claimed_by is null
                  or claimed_by = $1
                  or claimed_at < now() - interval '24 hours')
           order by (claimed_by is not distinct from $1) desc,
                    priority_score desc nulls last, id
           limit 1
           for update skip locked
         ) pick
         where j.id = pick.id
         returning j.id, j.coverage_phase, j.epoch, j.priority_score",
    )
    .bind(me)
    .bind(epoch)
    .fetch_optional(exec)
    .await
    .context("claiming next jurisdiction lease")
}

/// Atomically claim one specific jurisdiction. Same holdability guard as
/// [`claim_next`] (free, own, or stale) but no phase/epoch filter — targeted
/// claims are an operator affordance, not the factory path.
///
/// # Errors
/// Propagates the Postgres error when the claim statement fails.
pub async fn claim_id<'e, E>(exec: E, me: &str, id: &str) -> anyhow::Result<Option<Lease>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "update jurisdiction j
         set claimed_by = $1, claimed_at = now()
         from (
           select id from jurisdiction
           where id = $2
             and (claimed_by is null
                  or claimed_by = $1
                  or claimed_at < now() - interval '24 hours')
           for update skip locked
         ) pick
         where j.id = pick.id
         returning j.id, j.coverage_phase, j.epoch, j.priority_score",
    )
    .bind(me)
    .bind(id)
    .fetch_optional(exec)
    .await
    .context("claiming jurisdiction lease by id")
}

/// Advance `coverage_phase` at an intermediate phase boundary while KEEPING
/// the lease (renews the heartbeat). Returns `false` when `me` does not hold
/// the lease — never advances a row that isn't yours.
///
/// # Errors
/// `to` outside the phase contract (fail closed, before the DB CHECK would
/// reject it), or a Postgres error from the update.
pub async fn advance<'e, E>(exec: E, me: &str, id: &str, to: &str) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    validate_phase(to)?;
    let row: Option<(String,)> = sqlx::query_as(
        "update jurisdiction
         set coverage_phase = $3, claimed_at = now()
         where id = $2 and claimed_by = $1
         returning id",
    )
    .bind(me)
    .bind(id)
    .bind(to)
    .fetch_optional(exec)
    .await
    .context("advancing leased jurisdiction phase")?;
    Ok(row.is_some())
}

/// Clear the lease per the disposition. Returns `false` when `me` does not
/// hold it (can't release a lease that isn't yours).
///
/// # Errors
/// A [`Disposition::Advance`] phase outside the contract, or a Postgres
/// error from the update.
pub async fn release<'e, E>(
    exec: E,
    me: &str,
    id: &str,
    disposition: Disposition,
) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let query = match &disposition {
        Disposition::Keep => sqlx::query_as(
            "update jurisdiction
             set claimed_by = null, claimed_at = null
             where id = $2 and claimed_by = $1
             returning id",
        )
        .bind(me)
        .bind(id),
        Disposition::Advance(phase) => {
            validate_phase(phase)?;
            sqlx::query_as(
                "update jurisdiction
                 set claimed_by = null, claimed_at = null, coverage_phase = $3
                 where id = $2 and claimed_by = $1
                 returning id",
            )
            .bind(me)
            .bind(id)
            .bind(phase.clone())
        }
        Disposition::Block(reason) => sqlx::query_as(
            "update jurisdiction
             set claimed_by = null, claimed_at = null,
                 coverage_phase = 'blocked', blocked_reason = $3
             where id = $2 and claimed_by = $1
             returning id",
        )
        .bind(me)
        .bind(id)
        .bind(reason.clone()),
    };
    let row: Option<(String,)> = query
        .fetch_optional(exec)
        .await
        .context("releasing jurisdiction lease")?;
    Ok(row.is_some())
}

/// Every live lease — the shared "who's doing what" board
/// (parallel-factory.md legibility discipline).
///
/// # Errors
/// Propagates the Postgres error when the select fails.
pub async fn status<'e, E>(exec: E) -> anyhow::Result<Vec<LeaseStatus>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "select id, coverage_phase, claimed_by, claimed_at
         from jurisdiction
         where claimed_by is not null
         order by claimed_at",
    )
    .fetch_all(exec)
    .await
    .context("listing live jurisdiction leases")
}
