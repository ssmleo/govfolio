//! Verification promotion + supersession (plan Task 11, design §7.2):
//! resolving a `review_task` against its target `disclosure_record`, in one
//! transaction, under the supersede-never-update invariant (invariant 1).
//!
//! The three reviewer verdicts and exactly what each may write:
//!
//! - **Confirm** — the ONE sanctioned `UPDATE` of a Gold row, and it touches
//!   the single status column: `verification_state` `unverified → 'verified'`.
//!   Fact columns are never part of the statement.
//! - **Edit** — ZERO `UPDATE`s of `disclosure_record`. The corrected facts
//!   become a new row (`INSERT`) with `verification_state = 'corrected'` and
//!   `supersedes_record_id` pointing at the original (design §7.2: "edits
//!   create superseding `corrected` records"), plus one
//!   `disclosure_record.corrected` outbox event in the SAME transaction. The
//!   original row — every column of it — stays exactly as published.
//! - **Reject** — the reviewer examined the record and could NOT confirm it,
//!   without supplying a correction. State routing per the DDL vocabulary
//!   (`unverified|verified|corrected|disputed`): `'disputed'`. Leaving it
//!   `'unverified'` would claim nobody adjudicated it (and keep it in the
//!   `dr_review` backlog index); `'disputed'` says precisely "accuracy
//!   contested, no correction available", honestly labeled for consumers.
//!
//! Confirm/reject transition FROM `'unverified'` only; any other current
//! state means a conflicting adjudication and fails closed (whole txn rolls
//! back, the task stays open). Edit supersedes regardless of the original's
//! state — §5.6: reprocessing/corrections supersede any history, they never
//! mutate it.
//!
//! Idempotency: the task row is locked and checked `status = 'open'` first;
//! a second resolution attempt of the same task returns
//! [`ResolveOutcome::AlreadyResolved`] and writes nothing. The superseding
//! insert itself is `ON CONFLICT (fingerprint) DO NOTHING` (invariant 4) with
//! a deterministic correction-namespaced fingerprint.

use anyhow::Context as _;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};

use govfolio_core::domain::fingerprint::fingerprint;
use govfolio_core::domain::gold::GoldCandidate;

use crate::conformance::check_details;
use crate::stages::publish::wire;

/// A reviewer's verdict on one `review_task` targeting a `disclosure_record`
/// (design §7.2: approve / edit / reject).
#[derive(Debug)]
pub enum Verdict {
    /// The extraction is accurate as published: `unverified → 'verified'`.
    Confirm,
    /// The extraction is wrong and here are the corrected facts: INSERT a
    /// superseding `'corrected'` record. Identity (filing/politician/regime)
    /// is pinned from the original row — reviewer-supplied identity fields
    /// are ignored, corrections cannot rebind a record.
    Edit {
        /// Adapter code selecting the `details` registry arm (invariant 5),
        /// e.g. `us_house`.
        regime_code: String,
        /// The corrected facts, validated exactly like a publish candidate.
        corrected: Box<GoldCandidate>,
    },
    /// The extraction could not be confirmed and no correction is available:
    /// `unverified → 'disputed'`.
    Reject,
}

/// What resolving a task did.
#[derive(Debug, PartialEq, Eq)]
pub enum ResolveOutcome {
    /// The verdict was applied and the task is now `resolved`.
    Applied {
        /// The target `disclosure_record.id` the verdict adjudicated.
        record_id: String,
        /// The superseding row inserted by an edit (`None` for confirm/reject).
        superseding_record_id: Option<String>,
    },
    /// The task was not `open` — a previous resolution already adjudicated
    /// it. Nothing was written (idempotent no-op).
    AlreadyResolved,
}

/// The original row's identity, read once and pinned onto any correction.
struct OriginalRecord {
    id: String,
    filing_id: String,
    politician_id: String,
    regime_id: String,
}

/// Resolves one `review_task` with the given verdict — task update, record
/// state transition or superseding insert, and outbox event all in ONE
/// transaction.
///
/// # Errors
/// A task id that does not exist, a target kind this resolver does not handle
/// (only `disclosure_record`), a missing target record, a conflicting
/// adjudication (confirm/reject on a record no longer `'unverified'`), a
/// correction failing domain validation or the `details` contract
/// (invariant 5), or database failure. Any error rolls back the WHOLE
/// resolution: the task stays open, no partial writes survive.
pub async fn resolve_review_task(
    pool: &PgPool,
    task_id: &str,
    verdict: Verdict,
) -> anyhow::Result<ResolveOutcome> {
    let mut tx = pool.begin().await.context("opening resolve txn")?;

    // Lock the task row: concurrent resolvers serialize here, and the status
    // check below makes the second one a no-op.
    let task: Option<(String, String, String)> = sqlx::query_as(
        "select status, target_kind, target_id from review_task where id = $1 for update",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await
    .with_context(|| format!("loading review_task {task_id}"))?;
    let Some((status, target_kind, target_id)) = task else {
        anyhow::bail!("review_task {task_id} does not exist");
    };
    if status != "open" {
        return Ok(ResolveOutcome::AlreadyResolved);
    }
    anyhow::ensure!(
        target_kind == "disclosure_record",
        "review_task {task_id} targets {target_kind:?} — this resolver only \
         adjudicates disclosure_record tasks, fail closed"
    );

    let original: Option<(String, String, String)> = sqlx::query_as(
        "select filing_id, politician_id, regime_id from disclosure_record where id = $1",
    )
    .bind(&target_id)
    .fetch_optional(&mut *tx)
    .await
    .with_context(|| format!("loading target record {target_id}"))?;
    let Some((filing_id, politician_id, regime_id)) = original else {
        anyhow::bail!("review_task {task_id} targets missing record {target_id} — fail closed");
    };
    let original = OriginalRecord {
        id: target_id,
        filing_id,
        politician_id,
        regime_id,
    };

    let (superseding_record_id, resolution) = match verdict {
        Verdict::Confirm => {
            transition(&mut tx, &original.id, "verified").await?;
            (None, json!({ "verdict": "confirm" }))
        }
        Verdict::Reject => {
            transition(&mut tx, &original.id, "disputed").await?;
            (None, json!({ "verdict": "reject" }))
        }
        Verdict::Edit {
            regime_code,
            corrected,
        } => {
            let superseding = supersede(&mut tx, &original, &regime_code, &corrected).await?;
            let resolution = json!({
                "verdict": "edit",
                "superseding_record_id": superseding.record_id,
                "fingerprint": superseding.fingerprint,
            });
            (Some(superseding), resolution)
        }
    };

    if let Some(superseding) = &superseding_record_id
        && superseding.inserted
    {
        insert_corrected_outbox(&mut tx, &original, superseding, task_id).await?;
    }

    let resolved = sqlx::query(
        "update review_task \
         set status = 'resolved', resolution = $2, resolved_at = now() \
         where id = $1 and status = 'open'",
    )
    .bind(task_id)
    .bind(&resolution)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("resolving review_task {task_id}"))?
    .rows_affected();
    anyhow::ensure!(
        resolved == 1,
        "review_task {task_id} vanished under our row lock — refusing to commit"
    );

    tx.commit().await.context("committing resolve txn")?;
    Ok(ResolveOutcome::Applied {
        record_id: original.id,
        superseding_record_id: superseding_record_id.map(|s| s.record_id),
    })
}

/// The sanctioned state transition: `verification_state` and NOTHING else,
/// guarded on the current state being `'unverified'`. Zero rows affected
/// means a conflicting adjudication (or a state this transition is not
/// defined from) — fail closed, roll back, leave the task open.
async fn transition(
    tx: &mut Transaction<'_, Postgres>,
    record_id: &str,
    to: &str,
) -> anyhow::Result<()> {
    let affected = sqlx::query(
        "update disclosure_record set verification_state = $2 \
         where id = $1 and verification_state = 'unverified'",
    )
    .bind(record_id)
    .bind(to)
    .execute(&mut **tx)
    .await
    .with_context(|| format!("transitioning record {record_id} to {to}"))?
    .rows_affected();
    anyhow::ensure!(
        affected == 1,
        "record {record_id} is no longer 'unverified' — conflicting adjudication, fail closed"
    );
    Ok(())
}

/// The superseding row an edit produced.
struct Superseding {
    record_id: String,
    fingerprint: String,
    record_type: String,
    /// False when an identical correction already existed (fingerprint
    /// conflict) — no new row, so no new outbox event.
    inserted: bool,
}

/// INSERTs the corrected facts as a superseding `'corrected'` record. The
/// original row is read, never written: this function contains no `UPDATE`.
async fn supersede(
    tx: &mut Transaction<'_, Postgres>,
    original: &OriginalRecord,
    regime_code: &str,
    corrected: &GoldCandidate,
) -> anyhow::Result<Superseding> {
    // Pin identity from the original row (the closing half of the same seam
    // publish's bind_identity closes — corrections cannot rebind a record).
    let mut bound = corrected.clone();
    bound.filing_id = original
        .filing_id
        .parse()
        .map_err(|e| anyhow::anyhow!("filing id {:?}: {e}", original.filing_id))?;
    bound.politician_id = original
        .politician_id
        .parse()
        .map_err(|e| anyhow::anyhow!("politician id {:?}: {e}", original.politician_id))?;
    bound.regime_id = original
        .regime_id
        .parse()
        .map_err(|e| anyhow::anyhow!("regime id {:?}: {e}", original.regime_id))?;
    bound.fingerprint = None; // computed below, like publish

    // A correction clears the same bar as a publish candidate: domain
    // validation (the SQL CHECKs' mirror) + the details contract (invariant 5).
    bound
        .validate()
        .map_err(|e| anyhow::anyhow!("corrected record fails domain validation: {e}"))?;
    let contract_failures = check_details(regime_code, &bound)?;
    anyhow::ensure!(
        contract_failures.is_empty(),
        "corrected record fails the details contract at promotion (invariant 5): {}",
        contract_failures.join("; ")
    );

    // Deterministic fingerprint, namespaced by the superseded record so it
    // can never collide with publish fingerprints (which hash filing ids) and
    // so replaying the same correction is idempotent (invariant 4).
    let content = serde_json::to_value(&bound).context("serializing corrected record")?;
    let fp = fingerprint(&format!("correction:{}", original.id), 0, &content);

    let inserted: Option<String> = sqlx::query_scalar(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, instrument_id, asset_description_raw, \
            record_type, asset_class, side, transaction_date, as_of_date, notified_date, \
            value_low, value_high, currency, owner, verification_state, \
            extraction_confidence, extracted_by, fingerprint, supersedes_record_id, details) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                 'corrected', $17, $18, $19, $20, $21) \
         on conflict (fingerprint) do nothing \
         returning id",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(bound.filing_id.to_string())
    .bind(bound.politician_id.to_string())
    .bind(bound.regime_id.to_string())
    .bind(bound.instrument_id.map(|id| id.to_string()))
    .bind(&bound.asset_description_raw)
    .bind(wire(&bound.record_type)?)
    .bind(wire(&bound.asset_class)?)
    .bind(bound.side.as_ref().map(wire).transpose()?)
    .bind(bound.transaction_date)
    .bind(bound.as_of_date)
    .bind(bound.notified_date)
    .bind(bound.value.map(|v| v.low()))
    .bind(bound.value.and_then(|v| v.high()))
    .bind(bound.value.map(|v| wire(&v.currency())).transpose()?)
    .bind(bound.owner.as_ref().map(wire).transpose()?)
    .bind(bound.extraction_confidence)
    .bind(&bound.extracted_by)
    .bind(&fp)
    .bind(&original.id)
    .bind(&bound.details)
    .fetch_optional(&mut **tx)
    .await
    .with_context(|| format!("inserting superseding record for {}", original.id))?;

    let (record_id, was_inserted) = if let Some(id) = inserted {
        (id, true)
    } else {
        // An identical correction already exists (another task on the same
        // record resolved with the same facts): reuse it, write nothing new.
        let id = sqlx::query_scalar("select id from disclosure_record where fingerprint = $1")
            .bind(&fp)
            .fetch_one(&mut **tx)
            .await
            .with_context(|| format!("resolving existing correction {fp}"))?;
        (id, false)
    };
    Ok(Superseding {
        record_id,
        fingerprint: fp,
        record_type: wire(&bound.record_type)?,
        inserted: was_inserted,
    })
}

/// Writes the supersession's outbox event — same transaction as the
/// superseding insert (design §5.2 discipline), so alerts can never observe
/// a correction that was rolled back.
async fn insert_corrected_outbox(
    tx: &mut Transaction<'_, Postgres>,
    original: &OriginalRecord,
    superseding: &Superseding,
    task_id: &str,
) -> anyhow::Result<()> {
    let payload = json!({
        "record_id": superseding.record_id,
        "superseded_record_id": original.id,
        "fingerprint": superseding.fingerprint,
        "filing_id": original.filing_id,
        "politician_id": original.politician_id,
        "regime_id": original.regime_id,
        "record_type": superseding.record_type,
        "review_task_id": task_id,
    });
    sqlx::query("insert into outbox_event (id, kind, payload) values ($1, $2, $3)")
        .bind(ulid::Ulid::new().to_string())
        .bind("disclosure_record.corrected")
        .bind(payload)
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!(
                "writing corrected outbox_event for record {}",
                superseding.record_id
            )
        })?;
    Ok(())
}
