//! Send passes: pending deliveries → channel senders, with retries into the
//! DLQ. Instant rows go one message per record; digest rows accumulate and
//! ship as ONE summary per (rule, channel) — design §6.3.

use anyhow::Context as _;
use sqlx::PgPool;

use govfolio_core::alerts::AlertChannel;

use crate::alerts::matcher::EVENT_KIND;
use crate::alerts::payload::{
    AlertPayload, DigestEntry, DigestPayload, RecordSummary, digest_email, instant_email,
    load_summary, provenance_url,
};
use crate::alerts::retry::send_with_retry;
use crate::alerts::webhook::{WebhookRequest, signature_hex};
use crate::alerts::{DispatchConfig, Senders};

/// What a send/digest pass did.
#[derive(Debug, Default, Clone, Copy)]
pub struct SendStats {
    /// Delivery rows settled as `sent`.
    pub sent: u64,
    /// Delivery rows settled as `dead` (the DLQ).
    pub dead: u64,
}

/// One pending delivery with its rule channels and event payload.
#[derive(Debug, sqlx::FromRow)]
struct PendingRow {
    id: String,
    alert_rule_id: String,
    outbox_event_id: String,
    channel: String,
    attempts: i32,
    channels: serde_json::Value,
    payload: serde_json::Value,
}

/// Shared claim query (`$1` = status, `$2` = batch), oldest first.
const CLAIM_SQL: &str = "select d.id, d.alert_rule_id, d.outbox_event_id, d.channel, \
            d.attempts, r.channels, e.payload \
     from delivery d \
     join alert_rule r on r.id = d.alert_rule_id \
     join outbox_event e on e.id = d.outbox_event_id \
     where d.status = $1 \
     order by d.id \
     limit $2";

/// Sends every `pending` delivery: one message per record, full retry budget
/// per pass; exhaustion or a terminal error settles the row `dead`.
///
/// # Errors
/// Database failure or corrupt event payloads — content-level channel
/// problems (e.g. a channel removed from its rule) dead-letter the row
/// instead of failing the pass.
pub async fn send_pass(
    pool: &PgPool,
    config: &DispatchConfig,
    senders: &Senders<'_>,
) -> anyhow::Result<SendStats> {
    let rows: Vec<PendingRow> = sqlx::query_as(CLAIM_SQL)
        .bind("pending")
        .bind(config.batch)
        .fetch_all(pool)
        .await
        .context("claiming pending deliveries")?;
    let mut stats = SendStats::default();
    for row in rows {
        let summary = summary_for(pool, &row).await?;
        let channel = match resolve_channel(&row.channels, &row.channel) {
            Ok(channel) => channel,
            Err(error) => {
                settle(pool, &[row.id], "dead", 0, Some(&format!("{error:#}"))).await?;
                stats.dead += 1;
                continue;
            }
        };
        let provenance = provenance_url(&config.public_base_url, &summary.filing_id);
        let budget = remaining_budget(config, row.attempts);
        if budget == 0 {
            settle(pool, &[row.id], "dead", 0, Some("retry budget exhausted")).await?;
            stats.dead += 1;
            continue;
        }
        let outcome = match &channel {
            AlertChannel::Email { to } => {
                let message = instant_email(to, &summary, &provenance);
                send_with_retry(budget, config.backoff_base, || senders.email.send(&message)).await
            }
            AlertChannel::Webhook { url, secret } => {
                let body = serde_json::to_string(&AlertPayload {
                    kind: EVENT_KIND,
                    alert_rule_id: &row.alert_rule_id,
                    outbox_event_id: &row.outbox_event_id,
                    record: &summary,
                    provenance_url: provenance,
                })
                .context("serializing the alert payload")?;
                let request = WebhookRequest {
                    signature: signature_hex(secret, &body),
                    url: url.clone(),
                    body,
                };
                send_with_retry(budget, config.backoff_base, || {
                    senders.webhook.post(&request)
                })
                .await
            }
        };
        match outcome {
            Ok(made) => {
                settle(pool, &[row.id], "sent", made, None).await?;
                stats.sent += 1;
            }
            Err((made, error)) => {
                settle(pool, &[row.id], "dead", made, Some(&format!("{error:#}"))).await?;
                stats.dead += 1;
            }
        }
    }
    Ok(stats)
}

/// Ships every `pending_digest` delivery: grouped per (rule, channel), ONE
/// summary message per group; the whole group settles together.
///
/// # Errors
/// Same contract as [`send_pass`].
pub async fn digest_pass(
    pool: &PgPool,
    config: &DispatchConfig,
    senders: &Senders<'_>,
) -> anyhow::Result<SendStats> {
    let rows: Vec<PendingRow> = sqlx::query_as(CLAIM_SQL)
        .bind("pending_digest")
        .bind(config.batch)
        .fetch_all(pool)
        .await
        .context("claiming digest deliveries")?;
    let mut groups: std::collections::BTreeMap<(String, String), Vec<PendingRow>> =
        std::collections::BTreeMap::new();
    for row in rows {
        groups
            .entry((row.alert_rule_id.clone(), row.channel.clone()))
            .or_default()
            .push(row);
    }

    let mut stats = SendStats::default();
    for ((rule_id, channel_kind), group) in groups {
        let ids: Vec<String> = group.iter().map(|row| row.id.clone()).collect();
        let count = ids.len() as u64;
        let mut entries: Vec<(String, RecordSummary, String)> = Vec::with_capacity(group.len());
        for row in &group {
            let summary = summary_for(pool, row).await?;
            let provenance = provenance_url(&config.public_base_url, &summary.filing_id);
            entries.push((row.outbox_event_id.clone(), summary, provenance));
        }
        let channel = match resolve_channel(&group[0].channels, &channel_kind) {
            Ok(channel) => channel,
            Err(error) => {
                settle(pool, &ids, "dead", 0, Some(&format!("{error:#}"))).await?;
                stats.dead += count;
                continue;
            }
        };
        let prior = group.iter().map(|row| row.attempts).max().unwrap_or(0);
        let budget = remaining_budget(config, prior);
        if budget == 0 {
            settle(pool, &ids, "dead", 0, Some("retry budget exhausted")).await?;
            stats.dead += count;
            continue;
        }
        let outcome = match &channel {
            AlertChannel::Email { to } => {
                let items: Vec<(RecordSummary, String)> = entries
                    .iter()
                    .map(|(_, summary, provenance)| (summary.clone(), provenance.clone()))
                    .collect();
                let message = digest_email(to, &items);
                send_with_retry(budget, config.backoff_base, || senders.email.send(&message)).await
            }
            AlertChannel::Webhook { url, secret } => {
                let body = serde_json::to_string(&DigestPayload {
                    kind: "digest",
                    alert_rule_id: &rule_id,
                    records: entries
                        .iter()
                        .map(|(event_id, summary, provenance)| DigestEntry {
                            outbox_event_id: event_id,
                            record: summary,
                            provenance_url: provenance.clone(),
                        })
                        .collect(),
                })
                .context("serializing the digest payload")?;
                let request = WebhookRequest {
                    signature: signature_hex(secret, &body),
                    url: url.clone(),
                    body,
                };
                send_with_retry(budget, config.backoff_base, || {
                    senders.webhook.post(&request)
                })
                .await
            }
        };
        match outcome {
            Ok(made) => {
                settle(pool, &ids, "sent", made, None).await?;
                stats.sent += count;
            }
            Err((made, error)) => {
                settle(pool, &ids, "dead", made, Some(&format!("{error:#}"))).await?;
                stats.dead += count;
            }
        }
    }
    Ok(stats)
}

/// Loads the record summary behind a delivery's outbox event.
async fn summary_for(pool: &PgPool, row: &PendingRow) -> anyhow::Result<RecordSummary> {
    let record_id = row
        .payload
        .get("record_id")
        .and_then(serde_json::Value::as_str)
        .with_context(|| {
            format!(
                "outbox_event {} payload lacks record_id",
                row.outbox_event_id
            )
        })?;
    load_summary(pool, record_id).await
}

/// Finds the delivery's channel config on its rule; a removed channel is a
/// content-level failure (dead letter), not a pass failure.
fn resolve_channel(channels: &serde_json::Value, kind: &str) -> anyhow::Result<AlertChannel> {
    let channels: Vec<AlertChannel> =
        serde_json::from_value(channels.clone()).context("rule channels outside the contract")?;
    channels
        .into_iter()
        .find(|channel| channel.kind() == kind)
        .ok_or_else(|| anyhow::anyhow!("rule no longer has a {kind} channel"))
}

/// Attempts still allowed for a row that already burned `prior`.
fn remaining_budget(config: &DispatchConfig, prior: i32) -> u32 {
    config
        .max_attempts
        .saturating_sub(u32::try_from(prior).unwrap_or(0))
}

/// Settles a set of delivery rows in one statement.
async fn settle(
    pool: &PgPool,
    ids: &[String],
    status: &str,
    attempts_made: u32,
    last_error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "update delivery \
         set status = $2, attempts = attempts + $3, last_error = $4, updated_at = now() \
         where id = any($1)",
    )
    .bind(ids)
    .bind(status)
    .bind(i32::try_from(attempts_made).unwrap_or(i32::MAX))
    .bind(last_error)
    .execute(pool)
    .await
    .with_context(|| format!("settling {} delivery row(s) as {status}", ids.len()))?;
    Ok(())
}
