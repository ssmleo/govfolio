//! `pipeline_run` bookkeeping (design §5.2): every stage unit is claimed under
//! a deterministic idempotency key. A key seen before with `status =
//! 'succeeded'` is a replay — the stage is skipped; anything else re-executes
//! (crash-safe, at-least-once). Row-level `ON CONFLICT DO NOTHING` in the
//! stages themselves is the actual write guarantee (invariant 4) — this table
//! is the audit trail and the short-circuit.
//!
//! `pipeline_run` rows are operational bookkeeping, not Gold facts: updating
//! their `status`/`stats` does not touch invariant 1 (supersede-never-update
//! governs `disclosure_record`).

use anyhow::Context as _;
use serde_json::Value;
use sqlx::PgPool;

/// Outcome of claiming one stage unit by idempotency key.
#[derive(Debug)]
pub enum Claim {
    /// First time this unit is seen: execute, then [`finish_ok`]/[`finish_failed`].
    New {
        /// The freshly inserted `pipeline_run.id`.
        run_id: String,
    },
    /// Seen before but not `succeeded` (crash or failure): re-execute against
    /// the same audit row.
    Retry {
        /// The existing `pipeline_run.id`.
        run_id: String,
    },
    /// Already `succeeded`: skip execution; `stats` carries the prior audit.
    Replay {
        /// The `stats` jsonb recorded by the successful run.
        stats: Value,
    },
}

/// Claims a stage unit. Inserts a `running` row under `idempotency_key`
/// (`ON CONFLICT DO NOTHING`); when the key already exists, the prior status
/// decides between [`Claim::Replay`] and [`Claim::Retry`].
///
/// # Errors
/// Database failure.
pub async fn claim(
    pool: &PgPool,
    adapter: &str,
    stage: &str,
    idempotency_key: &str,
) -> anyhow::Result<Claim> {
    let minted = ulid::Ulid::new().to_string();
    let claimed: Option<String> = sqlx::query_scalar(
        "insert into pipeline_run (id, adapter, stage, idempotency_key, status) \
         values ($1, $2, $3, $4, 'running') \
         on conflict (idempotency_key) do nothing \
         returning id",
    )
    .bind(&minted)
    .bind(adapter)
    .bind(stage)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("claiming pipeline_run {idempotency_key}"))?;
    if let Some(run_id) = claimed {
        return Ok(Claim::New { run_id });
    }
    let (run_id, prior_status, audit): (String, String, Value) =
        sqlx::query_as("select id, status, stats from pipeline_run where idempotency_key = $1")
            .bind(idempotency_key)
            .fetch_one(pool)
            .await
            .with_context(|| format!("reading claimed pipeline_run {idempotency_key}"))?;
    if prior_status == "succeeded" {
        return Ok(Claim::Replay { stats: audit });
    }
    sqlx::query("update pipeline_run set status = 'running', error = null where id = $1")
        .bind(&run_id)
        .execute(pool)
        .await
        .with_context(|| format!("reopening pipeline_run {run_id}"))?;
    Ok(Claim::Retry { run_id })
}

/// Marks a claimed run `succeeded` with its audit stats.
///
/// # Errors
/// Database failure.
pub async fn finish_ok(pool: &PgPool, run_id: &str, stats: Value) -> anyhow::Result<()> {
    sqlx::query(
        "update pipeline_run \
         set status = 'succeeded', stats = $2, finished_at = now(), error = null \
         where id = $1",
    )
    .bind(run_id)
    .bind(stats)
    .execute(pool)
    .await
    .with_context(|| format!("finishing pipeline_run {run_id}"))?;
    Ok(())
}

/// Marks a claimed run `failed`, keeping the error for the audit trail.
///
/// # Errors
/// Database failure.
pub async fn finish_failed(pool: &PgPool, run_id: &str, error: &str) -> anyhow::Result<()> {
    sqlx::query(
        "update pipeline_run set status = 'failed', finished_at = now(), error = $2 where id = $1",
    )
    .bind(run_id)
    .bind(error)
    .execute(pool)
    .await
    .with_context(|| format!("failing pipeline_run {run_id}"))?;
    Ok(())
}
