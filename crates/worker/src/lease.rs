//! Generation-fenced jurisdiction leases for factory producers.
//!
//! Claim selection and ownership transfer remain one SQL statement with
//! `FOR UPDATE SKIP LOCKED`. New or stale ownership increments the durable
//! generation; resuming the same lane preserves it. Once an immutable
//! integration receipt is pending, every producer claim/renew/abandon path
//! excludes the row. Receipt apply is the sole `coverage_phase` authority.

use anyhow::{Context as _, bail};
use chrono::{DateTime, Utc};

/// The claimability definition shared by [`claim_next`] and
/// [`claimable_count`]. `$1` is an optional lane identity and `$2` is epoch.
/// A pending receipt remains a live lane holding, so the `not exists` leg
/// deliberately still prevents that lane from acquiring a second row.
const CLAIMABLE_PREDICATE: &str = "\
pending_integration_id is null
and coverage_phase not in ('live', 'blocked')
and (
  ($1::text is not null and claimed_by = $1)
  or (
    epoch = $2
    and (claimed_by is null
         or claimed_at < now() - interval '24 hours')
    and not exists (
      select 1 from jurisdiction held
      where held.claimed_by = $1
        and held.coverage_phase not in ('live', 'blocked')
    )
  )
)";

fn claimable_sql(head: &str, tail: &str) -> String {
    // Both call sites supply compile-time SQL fragments. Runtime values remain
    // bind parameters before the locally constructed statement is asserted.
    format!("{head}\nwhere {CLAIMABLE_PREDICATE}\n{tail}")
}

/// One successfully claimed registry row.
#[derive(Debug, sqlx::FromRow)]
pub struct Lease {
    pub id: String,
    pub coverage_phase: String,
    pub epoch: Option<i16>,
    pub priority_score: Option<f32>,
    pub generation: i64,
}

/// One currently held lease for monitoring.
#[derive(Debug, sqlx::FromRow)]
pub struct LeaseStatus {
    pub id: String,
    pub coverage_phase: String,
    pub claimed_by: String,
    pub claimed_at: DateTime<Utc>,
    pub generation: i64,
    pub pending_integration_id: Option<String>,
}

/// Retained only so legacy direct-release callers fail at runtime instead of
/// silently mutating phase. New producer code uses generation-CAS [`abandon`].
#[derive(Debug)]
pub enum Disposition {
    Keep,
    Advance(String),
    Block(String),
}

/// Claim the best row atomically, preferring the lane's own unfinished row.
/// An own resume renews the heartbeat without changing generation; a new or
/// stale ownership transfer increments generation in the same statement.
///
/// # Errors
///
/// Propagates connection acquisition or Postgres claim failures.
pub async fn claim_next<'a, A>(acq: A, me: &str, epoch: i16) -> anyhow::Result<Option<Lease>>
where
    A: sqlx::Acquire<'a, Database = sqlx::Postgres>,
{
    let mut conn = acq.acquire().await.context("acquiring connection")?;
    let pick = claimable_sql(
        "select id from jurisdiction",
        "order by case when claimed_by = $1 then 0 else 1 end,
                  priority_score desc nulls last, id
         limit 1
         for update skip locked",
    );
    let statement = format!(
        "update jurisdiction j
         set claimed_by = $1,
             claimed_at = now(),
             lease_generation = case
               when j.claimed_by = $1 then j.lease_generation
               else j.lease_generation + 1
             end
         from ({pick}) pick
         where j.id = pick.id
         returning j.id, j.coverage_phase, j.epoch, j.priority_score,
                   j.lease_generation as generation"
    );
    sqlx::query_as(sqlx::AssertSqlSafe(statement))
        .bind(me)
        .bind(epoch)
        .fetch_optional(&mut *conn)
        .await
        .context("claiming next jurisdiction lease")
}

/// Count rows visible to the exact [`claim_next`] predicate without locking or
/// mutating them. Suppressed/pending rows therefore yield a zero-spend stop.
///
/// # Errors
///
/// Propagates the Postgres read failure.
pub async fn claimable_count<'e, E>(exec: E, me: Option<&str>, epoch: i16) -> anyhow::Result<i64>
where
    E: sqlx::PgExecutor<'e>,
{
    let statement = claimable_sql("select count(*) from jurisdiction", "");
    sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
        .bind(me)
        .bind(epoch)
        .fetch_one(exec)
        .await
        .context("counting claimable jurisdictions")
}

/// Claim or resume one specific jurisdiction atomically. Pending integration
/// excludes unclaimed, own, and stale targeted paths alike.
///
/// # Errors
///
/// Propagates the Postgres claim failure.
pub async fn claim_id<'e, E>(exec: E, me: &str, id: &str) -> anyhow::Result<Option<Lease>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "update jurisdiction j
         set claimed_by = $1,
             claimed_at = now(),
             lease_generation = case
               when j.claimed_by = $1 then j.lease_generation
               else j.lease_generation + 1
             end
         from (
           select id from jurisdiction
           where id = $2
             and pending_integration_id is null
             and (claimed_by is null
                  or claimed_by = $1
                  or claimed_at < now() - interval '24 hours')
           for update skip locked
         ) pick
         where j.id = pick.id
         returning j.id, j.coverage_phase, j.epoch, j.priority_score,
                   j.lease_generation as generation",
    )
    .bind(me)
    .bind(id)
    .fetch_optional(exec)
    .await
    .context("claiming jurisdiction lease by id")
}

/// Renew only the exact lane/generation pair while no receipt is pending.
/// A stale or foreign credential returns `false` without changing the row.
///
/// # Errors
///
/// Propagates the Postgres compare-and-swap failure.
pub async fn renew<'e, E>(exec: E, me: &str, id: &str, generation: i64) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let row: Option<(String,)> = sqlx::query_as(
        "update jurisdiction
         set claimed_at = now()
         where id = $2
           and claimed_by = $1
           and lease_generation = $3
           and pending_integration_id is null
         returning id",
    )
    .bind(me)
    .bind(id)
    .bind(generation)
    .fetch_optional(exec)
    .await
    .context("renewing jurisdiction lease generation")?;
    Ok(row.is_some())
}

/// Abandon without changing phase, only for the exact lane/generation pair
/// and only before receipt submission. Receipt apply owns pending leases.
///
/// # Errors
///
/// Propagates the Postgres compare-and-swap failure.
pub async fn abandon<'e, E>(exec: E, me: &str, id: &str, generation: i64) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let row: Option<(String,)> = sqlx::query_as(
        "update jurisdiction
         set claimed_by = null, claimed_at = null
         where id = $2
           and claimed_by = $1
           and lease_generation = $3
           and pending_integration_id is null
         returning id",
    )
    .bind(me)
    .bind(id)
    .bind(generation)
    .fetch_optional(exec)
    .await
    .context("abandoning jurisdiction lease generation")?;
    Ok(row.is_some())
}

/// Retired direct phase mutation. Receipt apply is the sole phase authority.
///
/// # Errors
///
/// Always returns an error so old callers cannot move registry state ahead of
/// the exact green commit on `origin/main`.
#[expect(clippy::unused_async, reason = "retired async API must fail closed")]
pub async fn advance<'e, E>(_exec: E, _me: &str, _id: &str, _to: &str) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    bail!("direct phase advance is retired; submit an immutable integration receipt")
}

/// Retired direct release/live/block mutation. Generation-CAS [`abandon`] is
/// the only pre-receipt producer release path.
///
/// # Errors
///
/// Always returns an error so old callers cannot mutate phase or release a
/// pending integration lease.
#[expect(clippy::unused_async, reason = "retired async API must fail closed")]
pub async fn release<'e, E>(
    _exec: E,
    _me: &str,
    _id: &str,
    _disposition: Disposition,
) -> anyhow::Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    bail!("direct phase release is retired; submit an immutable integration receipt")
}

/// Return every held lease, including pending integration, for monitoring.
///
/// # Errors
///
/// Propagates the Postgres read failure.
pub async fn status<'e, E>(exec: E) -> anyhow::Result<Vec<LeaseStatus>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "select id, coverage_phase, claimed_by, claimed_at,
                lease_generation as generation, pending_integration_id
         from jurisdiction
         where claimed_by is not null
         order by claimed_at",
    )
    .fetch_all(exec)
    .await
    .context("listing live jurisdiction leases")
}
