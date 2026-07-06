//! The publish stage (design §5.2): inserts the filing (dedup by
//! `(regime_id, external_id)`), the Gold rows as `unverified`, one
//! `outbox_event` per inserted record, and the regime-specific review tasks —
//! ALL IN ONE TRANSACTION. Every write is `ON CONFLICT DO NOTHING`
//! (invariant 4); the `details` contract is validated at promotion
//! (invariant 5); fingerprints are computed HERE via `core::fingerprint`
//! over the id-bound candidate (T8c ships candidates with `fingerprint:
//! None` by contract).

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};

use govfolio_core::domain::fingerprint::fingerprint;
use govfolio_core::domain::gold::GoldCandidate;

use crate::conformance::check_details;
use crate::run::FilingIdentity;

/// Everything the publish stage needs to bind one filing's candidates to real
/// Postgres identities (the runner resolved them; design §5.4).
#[derive(Debug)]
pub struct FilingSpec<'a> {
    /// Seeded `disclosure_regime.id`.
    pub regime_id: &'a str,
    /// Adapter code, e.g. `us_house` — selects the details-schema registry arm.
    pub regime_code: &'a str,
    /// Roster-resolved `politician.id` of the filer.
    pub politician_id: &'a str,
    /// `raw_document.id` of the Bronze source.
    pub raw_document_id: &'a str,
    /// Filing identity extracted from the document's own silver rows.
    pub identity: &'a FilingIdentity,
    /// When WE found the filing (`filing.discovered_at`, design §4.2).
    pub discovered_at: DateTime<Utc>,
    /// Historical backfill run (goal 081 Task 2): Gold rows and `review_task`
    /// rows are unaffected, but each `outbox_event` is written already dispatched
    /// (`dispatched_at = now()`) so the matcher never fires real subscriber
    /// alerts for filings that are years old. The row still exists for audit.
    pub backfill: bool,
}

/// Audit stats of one publish; doubles as the `pipeline_run.stats` payload.
#[derive(Debug, Clone, Serialize)]
pub struct PublishStats {
    /// The filing row the records hang off (existing or newly inserted).
    pub filing_id: String,
    /// Candidates offered by normalize.
    pub candidates: u64,
    /// Gold rows actually inserted (0 on a replay — invariant 4).
    pub gold_inserted: u64,
    /// Outbox events written (always equals `gold_inserted` — same txn).
    pub outbox_written: u64,
    /// Review tasks opened for newly inserted records.
    pub review_tasks: u64,
    /// Candidates dropped by the redaction pass as un-republishable (design
    /// §7.5, e.g. FR patrimony) — never reached Gold.
    pub suppressed: u64,
}

/// Publishes one filing's Gold candidates atomically. `review_reasons` is the
/// regime-specific §7 rule hook (e.g. `ptr_amendment_unlinked`); reasons apply
/// only to records actually inserted, so replays never duplicate tasks.
///
/// # Errors
/// Database failure, a candidate failing domain validation, or a `details`
/// contract violation (invariant 5) — any error rolls back the WHOLE filing:
/// no partial Gold, no orphan outbox events.
pub async fn publish_filing(
    pool: &PgPool,
    spec: &FilingSpec<'_>,
    candidates: &[GoldCandidate],
    review_reasons: &(dyn Fn(&GoldCandidate) -> Vec<String> + Sync),
) -> anyhow::Result<PublishStats> {
    // Fail-closed drift gate (design §5.6, goal 070): when the sentinel has
    // frozen this regime (a layout shift, count-to-zero, or vanished markers —
    // migration 0008), garbage never reaches Gold. Open/refresh a review_task
    // and write NOTHING. The publish stage fails (retryable): once the source
    // recovers and the sentinel unfreezes it, the retry publishes normally.
    if is_regime_frozen(pool, spec.regime_code).await? {
        crate::stages::roster::open_review_task_once(
            pool,
            "regime",
            spec.regime_code,
            "publish_blocked_frozen",
        )
        .await?;
        anyhow::bail!(
            "regime {} publication is frozen by sentinel drift (design §5.6) — \
             refusing to publish Gold; review_task opened, nothing written",
            spec.regime_code
        );
    }

    let mut tx = pool.begin().await.context("opening publish txn")?;
    let filing_id = ensure_filing(&mut tx, spec).await?;
    let mut stats = PublishStats {
        filing_id: filing_id.clone(),
        candidates: u64::try_from(candidates.len()).context("candidate count overflow")?,
        gold_inserted: 0,
        outbox_written: 0,
        review_tasks: 0,
        suppressed: 0,
    };
    for (index, candidate) in candidates.iter().enumerate() {
        let ordinal = u32::try_from(index).context("ordinal overflow")?;
        let mut bound = bind_identity(candidate, &filing_id, spec)?;
        // Pre-publication redaction (design §7.5): strip out-of-scope personal
        // data / drop un-republishable records BEFORE the contract check and
        // the Gold insert. Bronze + the staged Silver row keep the raw
        // (invariant 2 — `bound` is a clone, the source candidate is untouched).
        match crate::redaction::redact(spec.regime_code, &mut bound) {
            crate::redaction::Redaction::Suppress { reason } => {
                // Un-republishable (e.g. FR patrimony): no Gold row. Surface
                // the belt-and-suspenders catch for audit, then skip it.
                insert_filing_review_task(&mut tx, &filing_id, &reason).await?;
                stats.suppressed += 1;
                continue;
            }
            crate::redaction::Redaction::Publish { .. } => {}
        }
        bound
            .validate()
            .map_err(|e| anyhow::anyhow!("gold[{ordinal}] fails domain validation: {e}"))?;
        let contract_failures = check_details(spec.regime_code, &bound)?;
        anyhow::ensure!(
            contract_failures.is_empty(),
            "gold[{ordinal}] fails the details contract at promotion (invariant 5): {}",
            contract_failures.join("; ")
        );
        // Deterministic fingerprint over (filing_id, ordinal, canonical
        // content) — plan Task 6 — computed at this stage, after binding.
        // `fingerprint_content` is the per-regime content-selector hook (same
        // `regime_code` dispatch idiom as `redact`/`check_details` above): the
        // default arm is the unchanged bare serialization; `br` excludes its
        // two backend-re-timestamped raw fields from the hash only (the
        // stored `details` itself is never touched — see
        // `crate::fingerprint_content` and `docs/regimes/br/AUTHORITY.md`).
        let content = crate::fingerprint_content::fingerprint_content(spec.regime_code, &bound)
            .context("computing fingerprint content")?;
        let fp = fingerprint(&filing_id, ordinal, &content);
        let Some(record_id) = insert_record(&mut tx, &bound, &fp).await? else {
            continue; // fingerprint seen before: replay inserts nothing
        };
        stats.gold_inserted += 1;
        insert_outbox(&mut tx, &record_id, &fp, &bound, spec.backfill).await?;
        stats.outbox_written += 1;
        for reason in review_reasons(&bound) {
            insert_review_task(&mut tx, &record_id, &reason).await?;
            stats.review_tasks += 1;
        }
    }
    tx.commit().await.context("committing publish txn")?;
    Ok(stats)
}

/// Whether the regime's publication is frozen by the sentinel (design §5.6;
/// `sentinel_watch.frozen`, kept in sync with the open freezing `drift_report`
/// by `worker::sentinel`). A regime never watched (no row) is not frozen.
///
/// # Errors
/// Database failure.
async fn is_regime_frozen(pool: &PgPool, regime_code: &str) -> anyhow::Result<bool> {
    let frozen: Option<bool> =
        sqlx::query_scalar("select frozen from sentinel_watch where regime_code = $1")
            .bind(regime_code)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("checking freeze state for {regime_code}"))?;
    Ok(frozen.unwrap_or(false))
}

/// Opens a filing-scoped `review_task` (used when a record is suppressed by the
/// redaction pass — the drop must be visible, not silent).
async fn insert_filing_review_task(
    tx: &mut Transaction<'_, Postgres>,
    filing_id: &str,
    reason: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, 'filing', $2, $3)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(filing_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .with_context(|| format!("opening filing review_task {reason} for {filing_id}"))?;
    Ok(())
}

/// Inserts the filing row (dedup by `(regime_id, external_id)`, design §5.2)
/// and returns the surviving row's id — stable across replays, which keeps
/// fingerprints deterministic.
async fn ensure_filing(
    tx: &mut Transaction<'_, Postgres>,
    spec: &FilingSpec<'_>,
) -> anyhow::Result<String> {
    sqlx::query(
        "insert into filing \
           (id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
            filed_date, discovered_at) \
         values ($1, $2, $3, $4, $5, $6, $7, $8) \
         on conflict (regime_id, external_id) do nothing",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(spec.regime_id)
    .bind(spec.politician_id)
    .bind(spec.raw_document_id)
    .bind(&spec.identity.external_id)
    .bind(&spec.identity.filing_type)
    .bind(spec.identity.filed_date)
    .bind(spec.discovered_at)
    .execute(&mut **tx)
    .await
    .with_context(|| format!("inserting filing {}", spec.identity.external_id))?;
    sqlx::query_scalar("select id from filing where regime_id = $1 and external_id = $2")
        .bind(spec.regime_id)
        .bind(&spec.identity.external_id)
        .fetch_one(&mut **tx)
        .await
        .with_context(|| format!("resolving filing id for {}", spec.identity.external_id))
}

/// Rebinds the candidate's identity triple to the resolved Postgres ids.
/// Adapters running pool-backed emit UNBOUND (nil-ULID) identities — this is
/// the closing half of that seam; FK constraints reject any placeholder that
/// could ever leak past it.
fn bind_identity(
    candidate: &GoldCandidate,
    filing_id: &str,
    spec: &FilingSpec<'_>,
) -> anyhow::Result<GoldCandidate> {
    let mut bound = candidate.clone();
    bound.filing_id = filing_id
        .parse()
        .map_err(|e| anyhow::anyhow!("filing id {filing_id:?}: {e}"))?;
    bound.politician_id = spec
        .politician_id
        .parse()
        .map_err(|e| anyhow::anyhow!("politician id {:?}: {e}", spec.politician_id))?;
    bound.regime_id = spec
        .regime_id
        .parse()
        .map_err(|e| anyhow::anyhow!("regime id {:?}: {e}", spec.regime_id))?;
    Ok(bound)
}

/// Serializes a closed-vocabulary value to its wire token (the SQL CHECK
/// literal — one rule, two enforcers). Shared with `promote` (the superseding
/// insert speaks the same vocabulary).
pub(crate) fn wire<T: Serialize>(value: &T) -> anyhow::Result<String> {
    match serde_json::to_value(value).context("serializing wire token")? {
        serde_json::Value::String(s) => Ok(s),
        other => anyhow::bail!("expected a string wire token, got {other}"),
    }
}

/// Inserts one Gold row as `unverified`; `None` when the fingerprint already
/// exists (idempotent replay, invariant 4).
async fn insert_record(
    tx: &mut Transaction<'_, Postgres>,
    bound: &GoldCandidate,
    fp: &str,
) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, instrument_id, asset_description_raw, \
            record_type, asset_class, side, transaction_date, as_of_date, notified_date, \
            value_low, value_high, currency, owner, verification_state, \
            extraction_confidence, extracted_by, fingerprint, details) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                 'unverified', $17, $18, $19, $20) \
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
    .bind(fp)
    .bind(&bound.details)
    .fetch_optional(&mut **tx)
    .await
    .with_context(|| format!("inserting disclosure_record {fp}"))
}

/// Writes the record's outbox event — same transaction as the Gold insert
/// (design §5.2), so alerts can never observe a record that was rolled back.
/// `backfill` (goal 081 Task 2) binds `dispatched_at = now()` in this same
/// INSERT instead of leaving it NULL, so the matcher's `dispatched_at is
/// null` poll never picks up historical events — no real subscriber alert
/// ever fires for a backfilled filing, while the event row still exists for
/// audit.
async fn insert_outbox(
    tx: &mut Transaction<'_, Postgres>,
    record_id: &str,
    fp: &str,
    bound: &GoldCandidate,
    backfill: bool,
) -> anyhow::Result<()> {
    let payload = json!({
        "record_id": record_id,
        "fingerprint": fp,
        "filing_id": bound.filing_id.to_string(),
        "politician_id": bound.politician_id.to_string(),
        "regime_id": bound.regime_id.to_string(),
        "record_type": wire(&bound.record_type)?,
    });
    sqlx::query(
        "insert into outbox_event (id, kind, payload, dispatched_at) \
         values ($1, $2, $3, case when $4 then now() else null end)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind("disclosure_record.published")
    .bind(payload)
    .bind(backfill)
    .execute(&mut **tx)
    .await
    .with_context(|| format!("writing outbox_event for record {record_id}"))?;
    Ok(())
}

/// Opens a review task against a newly inserted record (§7 rules; only new
/// records reach here, so replays cannot duplicate tasks).
async fn insert_review_task(
    tx: &mut Transaction<'_, Postgres>,
    record_id: &str,
    reason: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into review_task (id, target_kind, target_id, reason) values ($1, $2, $3, $4)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind("disclosure_record")
    .bind(record_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .with_context(|| format!("opening review_task {reason} for record {record_id}"))?;
    Ok(())
}
