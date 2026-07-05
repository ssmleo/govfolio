//! Transactional-outbox alert dispatcher (design §6.3, goal 030).
//!
//! Flow: [`matcher::match_pass`] consumes undispatched `outbox_event` rows,
//! matches each event's record against every active `alert_rule` through the
//! ONE shared filter grammar (`core::query::RecordFilter` — the same
//! evaluator behind `/v1/records`), and writes `delivery` rows with
//! deterministic dedup keys (exactly-once under at-least-once redelivery).
//! [`sender::send_pass`] / [`sender::digest_pass`] then push pending rows
//! through the channel senders (traits — offline-testable with mocks) with
//! retries and backoff; exhausted rows land in the DLQ
//! (`delivery.status = 'dead'`).
//!
//! Between send and settle the process can crash, so channel delivery itself
//! is at-least-once; payloads carry `outbox_event_id` + `alert_rule_id` so
//! receivers can dedup.

pub mod email;
pub mod matcher;
pub mod payload;
pub mod retry;
pub mod sender;
pub mod webhook;

use anyhow::Context as _;
use serde::Deserialize;
use sqlx::PgPool;

use govfolio_core::alerts::AlertChannel;
use govfolio_core::query::RecordFilter;

use self::email::EmailSender;
use self::webhook::WebhookTransport;

/// Dispatcher knobs (one instance per pass; the `dispatch` bin builds it
/// from flags + env).
#[derive(Debug, Clone)]
pub struct DispatchConfig {
    /// Total send attempts per delivery before it goes `dead`.
    pub max_attempts: u32,
    /// First backoff delay; doubles per retry.
    pub backoff_base: std::time::Duration,
    /// Max outbox events / delivery rows consumed per pass.
    pub batch: i64,
    /// Base URL for provenance links in alert payloads.
    pub public_base_url: String,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            backoff_base: std::time::Duration::from_millis(500),
            batch: 100,
            public_base_url: "https://govfolio.io".to_owned(),
        }
    }
}

/// The channel senders a pass fans out through — trait objects so tests
/// inject mock transports.
pub struct Senders<'a> {
    /// Email channel.
    pub email: &'a dyn EmailSender,
    /// Webhook channel.
    pub webhook: &'a dyn WebhookTransport,
}

/// A send failure with retry classification (mirrors the pipeline's
/// transport-error shape; owned here so mocks can construct it).
#[derive(Debug)]
pub struct SendError {
    /// True for connection-level failures and 408/429/5xx responses.
    pub retryable: bool,
    /// Human-readable cause; recorded as `delivery.last_error`.
    pub message: String,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SendError {}

/// One active alert rule, re-typed through the core contracts.
#[derive(Debug, Deserialize)]
pub(crate) struct ActiveRule {
    pub id: String,
    pub filter: RecordFilter,
    pub channels: Vec<AlertChannel>,
    pub digest: bool,
}

/// Loads every active rule, failing closed on jsonb outside the contracts
/// (rule config only enters through the validated API door, so a corrupt row
/// is a real integrity problem — never silently skipped).
pub(crate) async fn load_active_rules(pool: &PgPool) -> anyhow::Result<Vec<ActiveRule>> {
    let rows: Vec<(String, serde_json::Value, serde_json::Value, bool)> =
        sqlx::query_as("select id, filter, channels, digest from alert_rule where active")
            .fetch_all(pool)
            .await
            .context("loading active alert rules")?;
    rows.into_iter()
        .map(|(id, filter, channels, digest)| {
            Ok(ActiveRule {
                filter: serde_json::from_value(filter)
                    .with_context(|| format!("alert_rule {id}: filter outside the grammar"))?,
                channels: serde_json::from_value(channels)
                    .with_context(|| format!("alert_rule {id}: channels outside the contract"))?,
                id,
                digest,
            })
        })
        .collect()
}
