//! The matcher pass: outbox → deliveries, exactly once.
//!
//! One transaction per pass: the batch of undispatched events is locked
//! (`for update skip locked`), each event's record is matched against every
//! active rule through the shared grammar (an indexed point query per pair —
//! microseconds at design volumes, §6.3), matching (rule, channel) pairs get
//! delivery rows with deterministic dedup keys (`ON CONFLICT DO NOTHING`,
//! invariant 4), and `dispatched_at` is stamped IN THE SAME TXN — so a crash
//! anywhere rolls back to a cleanly redeliverable state, and redelivery
//! inserts nothing new.

use anyhow::Context as _;
use const_format::concatcp;
use sqlx::PgPool;

use govfolio_core::query::RecordFilter;

use crate::alerts::{DispatchConfig, load_active_rules};

/// The outbox kind the dispatcher understands. Other kinds stay undispatched
/// (visible backlog) until code that understands them ships.
pub const EVENT_KIND: &str = "disclosure_record.published";

/// One record-vs-filter evaluation: the grammar owns `$1..=$10`, the record
/// id binds at `$11`. This is the SAME `SQL_WHERE` `/v1/records` runs — one
/// grammar, one evaluator (design §6.3).
const MATCH_SQL: &str = concatcp!(
    "select exists(select 1 from disclosure_record where id = $11 and ",
    RecordFilter::SQL_WHERE,
    ")"
);

/// What a matcher pass did.
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchStats {
    /// Outbox events consumed (dispatched).
    pub events: u64,
    /// Delivery rows actually inserted (0 for replayed events).
    pub deliveries: u64,
}

/// Runs one matcher pass. Safe to run concurrently (row locks + dedup keys)
/// and to re-run after any crash.
///
/// # Errors
/// Database failure or a rule outside the contracts — the whole pass rolls
/// back (fail closed; no partially dispatched events).
pub async fn match_pass(pool: &PgPool, config: &DispatchConfig) -> anyhow::Result<MatchStats> {
    let rules = load_active_rules(pool).await?;
    let mut tx = pool.begin().await.context("opening matcher txn")?;
    let events: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "select id, payload from outbox_event \
         where dispatched_at is null and kind = $1 \
         order by id limit $2 \
         for update skip locked",
    )
    .bind(EVENT_KIND)
    .bind(config.batch)
    .fetch_all(&mut *tx)
    .await
    .context("locking undispatched outbox events")?;

    let mut stats = MatchStats::default();
    for (event_id, event_payload) in &events {
        let record_id = event_payload
            .get("record_id")
            .and_then(serde_json::Value::as_str)
            .with_context(|| format!("outbox_event {event_id} payload lacks record_id"))?;
        for rule in &rules {
            let matched: bool = rule
                .filter
                .bind_query_scalar(sqlx::query_scalar(MATCH_SQL))
                .context("binding the filter grammar")?
                .bind(record_id)
                .fetch_one(&mut *tx)
                .await
                .with_context(|| format!("matching record {record_id} vs rule {}", rule.id))?;
            if !matched {
                continue;
            }
            let delivery_status = if rule.digest {
                "pending_digest"
            } else {
                "pending"
            };
            for channel in &rule.channels {
                let kind = channel.kind();
                let inserted = sqlx::query(
                    "insert into delivery \
                       (id, alert_rule_id, outbox_event_id, channel, dedup_key, status) \
                     values ($1, $2, $3, $4, $5, $6) \
                     on conflict (dedup_key) do nothing",
                )
                .bind(ulid::Ulid::new().to_string())
                .bind(&rule.id)
                .bind(event_id)
                .bind(kind)
                // Deterministic (rule, event, channel) — the exactly-once key.
                .bind(format!("{}:{event_id}:{kind}", rule.id))
                .bind(delivery_status)
                .execute(&mut *tx)
                .await
                .with_context(|| format!("inserting delivery for rule {}", rule.id))?;
                stats.deliveries += inserted.rows_affected();
            }
        }
        sqlx::query("update outbox_event set dispatched_at = now() where id = $1")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("stamping dispatched_at on {event_id}"))?;
        stats.events += 1;
    }
    tx.commit().await.context("committing matcher txn")?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_match_sql_composes_the_shared_grammar() {
        // The evaluator is core's fragment verbatim, with the record pinned
        // at the next free slot — no second grammar implementation exists.
        assert!(MATCH_SQL.contains(RecordFilter::SQL_WHERE));
        assert!(MATCH_SQL.contains("$11"));
    }
}
