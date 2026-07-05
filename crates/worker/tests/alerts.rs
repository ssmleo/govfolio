//! Goal 030 acceptance: `cargo test -p worker alerts`.
//!
//! Transactional-outbox alert dispatcher (design §6.3): matcher (ONE shared
//! filter grammar — `core::query::RecordFilter`), exactly-once fan-out under
//! redelivery (dedup keys + ON CONFLICT), HMAC-signed webhooks, retry/backoff
//! into the DLQ (`delivery.status = 'dead'`), digest grouping.
//!
//! DB-gated like every sqlx suite: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::NaiveDate;
use hmac::Mac as _;
use serde_json::{Value, json};
use sqlx::PgPool;

use worker::alerts::email::{EmailMessage, EmailSender};
use worker::alerts::matcher::match_pass;
use worker::alerts::sender::{digest_pass, send_pass};
use worker::alerts::webhook::{WebhookRequest, WebhookTransport, signature_hex};
use worker::alerts::{DispatchConfig, SendError, Senders};

// ---------------------------------------------------------------- fixtures --

/// Minimal FK graph under two jurisdictions: us (regime + politician) and gb.
async fn seed_graph(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::query(
        "insert into jurisdiction (id, name, iso_code, level) values
           ('us', 'United States', 'US', 'national'),
           ('gb', 'United Kingdom', 'GB', 'national')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_regime
           (id, jurisdiction_id, body, regime_type, value_precision, effective_from)
         values ('0RUSHSE00000000000000000US', 'us', 'US House', 'transaction_report',
                 'banded', '2012-07-03')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into politician (id, canonical_name) values
           ('0HSEMBR0000000000000000001', 'Hon. Ada Lovelace'),
           ('0HSEMBR0000000000000000002', 'Hon. Charles Babbage')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at)
         values ('0RAWDOC0000000000000000001', 'file:///dev/null', repeat('a', 64),
                 'application/pdf', now())",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into filing (id, regime_id, politician_id, raw_document_id, external_id,
                             filing_type, discovered_at)
         values ('0FILING0000000000000000001', '0RUSHSE00000000000000000US',
                 '0HSEMBR0000000000000000001', '0RAWDOC0000000000000000001',
                 '20026969', 'ptr', now())",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// One Gold row + its outbox event, mirroring the publish stage's writes.
/// Returns (`record_id`, `outbox_event_id`).
#[allow(clippy::too_many_arguments)]
async fn seed_record(
    pool: &PgPool,
    n: u32,
    politician_id: &str,
    record_type: &str,
    asset_class: &str,
    transaction_date: Option<NaiveDate>,
    as_of_date: Option<NaiveDate>,
    value: Option<(&str, &str)>,
) -> (String, String) {
    let record_id = format!("0RECORD00000000000000000{n:02}");
    let event_id = format!("0OUTBOX00000000000000000{n:02}");
    let side = (record_type == "transaction").then_some("buy");
    sqlx::query(
        "insert into disclosure_record
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type,
            asset_class, side, transaction_date, as_of_date, value_low, value_high,
            currency, extraction_confidence, extracted_by, fingerprint)
         values ($1, '0FILING0000000000000000001', $2, '0RUSHSE00000000000000000US',
                 $3, $4, $5, $6, $7, $8, cast($9 as numeric), cast($10 as numeric),
                 case when $9 is null then null else 'USD' end, 0.97, 'test-seed', $1)",
    )
    .bind(&record_id)
    .bind(politician_id)
    .bind(format!("Seed asset {n}"))
    .bind(record_type)
    .bind(asset_class)
    .bind(side)
    .bind(transaction_date)
    .bind(as_of_date)
    .bind(value.map(|(low, _)| low))
    .bind(value.map(|(_, high)| high))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("insert into outbox_event (id, kind, payload) values ($1, $2, $3)")
        .bind(&event_id)
        .bind("disclosure_record.published")
        .bind(json!({ "record_id": record_id, "filing_id": "0FILING0000000000000000001" }))
        .execute(pool)
        .await
        .unwrap();
    (record_id, event_id)
}

async fn seed_rule(pool: &PgPool, id: &str, filter: Value, channels: Value, digest: bool) {
    sqlx::query(
        "insert into alert_rule (id, user_id, filter, channels, digest)
         values ($1, 'user-030', $2, $3, $4)",
    )
    .bind(id)
    .bind(filter)
    .bind(channels)
    .bind(digest)
    .execute(pool)
    .await
    .unwrap();
}

fn both_channels() -> Value {
    json!([
        { "type": "email", "to": "alerts@example.org" },
        { "type": "webhook", "url": "https://example.org/hook", "secret": "s3cret-030" },
    ])
}

const D: fn(i32, u32, u32) -> NaiveDate = |y, m, d| match NaiveDate::from_ymd_opt(y, m, d) {
    Some(date) => date,
    None => panic!("bad test date"),
};

// ------------------------------------------------------------------- mocks --

#[derive(Default)]
struct MockEmail {
    sent: Mutex<Vec<EmailMessage>>,
    fail: bool,
}

#[async_trait]
impl EmailSender for MockEmail {
    async fn send(&self, message: &EmailMessage) -> anyhow::Result<()> {
        if self.fail {
            return Err(SendError {
                retryable: true,
                message: "smtp down (mock)".to_owned(),
            }
            .into());
        }
        self.sent.lock().unwrap().push(message.clone());
        Ok(())
    }
}

#[derive(Default)]
struct MockWebhook {
    sent: Mutex<Vec<WebhookRequest>>,
    fail: bool,
}

#[async_trait]
impl WebhookTransport for MockWebhook {
    async fn post(&self, request: &WebhookRequest) -> anyhow::Result<()> {
        if self.fail {
            return Err(SendError {
                retryable: true,
                message: "endpoint 503 (mock)".to_owned(),
            }
            .into());
        }
        self.sent.lock().unwrap().push(request.clone());
        Ok(())
    }
}

fn fast_config() -> DispatchConfig {
    DispatchConfig {
        max_attempts: 3,
        backoff_base: std::time::Duration::from_millis(1),
        batch: 100,
        public_base_url: "https://govfolio.io".to_owned(),
    }
}

// ------------------------------------------------------------------- tests --

/// Exactly-once under redelivery: the SAME outbox event processed twice (the
/// queue is at-least-once — dedup is ours, invariant 4) yields ONE delivery
/// row per (rule, channel); `dispatched_at` is stamped in the matcher's txn.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alerts_exactly_once_delivery_under_redelivery(pool: PgPool) {
    seed_graph(&pool).await;
    let (_, event_id) = seed_record(
        &pool,
        1,
        "0HSEMBR0000000000000000001",
        "transaction",
        "equity",
        Some(D(2026, 6, 1)),
        None,
        Some(("1001.00", "15000.00")),
    )
    .await;
    seed_rule(
        &pool,
        "0RULE000000000000000000001",
        json!({}),
        both_channels(),
        false,
    )
    .await;

    let stats = match_pass(&pool, &fast_config()).await.unwrap();
    assert_eq!(stats.events, 1);
    assert_eq!(stats.deliveries, 2, "one delivery per channel");
    let dispatched: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select dispatched_at from outbox_event where id = $1")
            .bind(&event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(dispatched.is_some(), "dispatched_at set in the same txn");

    // Second pass: nothing undispatched — a no-op.
    let stats = match_pass(&pool, &fast_config()).await.unwrap();
    assert_eq!((stats.events, stats.deliveries), (0, 0));

    // Redelivery: the event comes back (at-least-once). Dedup keys + ON
    // CONFLICT DO NOTHING keep the ledger at exactly one row per channel.
    sqlx::query("update outbox_event set dispatched_at = null where id = $1")
        .bind(&event_id)
        .execute(&pool)
        .await
        .unwrap();
    let stats = match_pass(&pool, &fast_config()).await.unwrap();
    assert_eq!(stats.events, 1);
    assert_eq!(stats.deliveries, 0, "redelivery inserts no new rows");
    let per_channel: Vec<(String, i64)> =
        sqlx::query_as("select channel, count(*) from delivery group by channel order by channel")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        per_channel,
        vec![("email".to_owned(), 1), ("webhook".to_owned(), 1)],
        "exactly one delivery row per (rule, channel)"
    );
}

/// Matcher correctness against the ONE shared grammar: matching and
/// non-matching rules across `record_type`, `asset_class`, politician,
/// jurisdiction, date range and value bounds.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alerts_matcher_applies_the_shared_grammar(pool: PgPool) {
    seed_graph(&pool).await;
    // A $1,001–$15,000 equity buy by politician ...01 on 2026-06-01 (us).
    seed_record(
        &pool,
        1,
        "0HSEMBR0000000000000000001",
        "transaction",
        "equity",
        Some(D(2026, 6, 1)),
        None,
        Some(("1001.00", "15000.00")),
    )
    .await;
    let email = json!([{ "type": "email", "to": "a@example.org" }]);
    // Matching rules.
    seed_rule(
        &pool,
        "0RULEMATCH0000000000000001",
        json!({ "record_type": "transaction", "asset_class": "equity" }),
        email.clone(),
        false,
    )
    .await;
    seed_rule(
        &pool,
        "0RULEMATCH0000000000000002",
        json!({
            "jurisdiction_id": "us",
            "politician_id": "0HSEMBR0000000000000000001",
            "event_date_from": "2026-06-01",
            "event_date_to": "2026-06-30",
            "value_min": "5000.00",
        }),
        email.clone(),
        false,
    )
    .await;
    // Non-matching rules, one per grammar axis.
    for (n, filter) in [
        json!({ "record_type": "holding" }),
        json!({ "asset_class": "crypto" }),
        json!({ "politician_id": "0HSEMBR0000000000000000002" }),
        json!({ "jurisdiction_id": "gb" }),
        json!({ "event_date_to": "2026-05-31" }),
        json!({ "value_min": "20000.00" }),
        json!({ "value_max": "1000.00" }),
        json!({ "verification_state": "verified" }),
        json!({ "instrument_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV" }),
    ]
    .into_iter()
    .enumerate()
    {
        seed_rule(
            &pool,
            &format!("0RULEMISS000000000000000{n:02}"),
            filter,
            email.clone(),
            false,
        )
        .await;
    }

    let stats = match_pass(&pool, &fast_config()).await.unwrap();
    assert_eq!(stats.deliveries, 2, "only the two matching rules fan out");
    let matched: Vec<String> =
        sqlx::query_scalar("select alert_rule_id from delivery order by alert_rule_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        matched,
        vec![
            "0RULEMATCH0000000000000001".to_owned(),
            "0RULEMATCH0000000000000002".to_owned(),
        ]
    );
}

/// Webhook bodies are HMAC-SHA256 signed with the rule's per-channel secret,
/// and the payload carries the §6.3 honesty fields.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alerts_webhook_is_hmac_signed_and_honest(pool: PgPool) {
    seed_graph(&pool).await;
    seed_record(
        &pool,
        1,
        "0HSEMBR0000000000000000001",
        "transaction",
        "equity",
        Some(D(2026, 6, 1)),
        None,
        Some(("1001.00", "15000.00")),
    )
    .await;
    seed_rule(
        &pool,
        "0RULE000000000000000000001",
        json!({}),
        both_channels(),
        false,
    )
    .await;
    match_pass(&pool, &fast_config()).await.unwrap();

    let email = MockEmail::default();
    let webhook = MockWebhook::default();
    let senders = Senders {
        email: &email,
        webhook: &webhook,
    };
    let stats = send_pass(&pool, &fast_config(), &senders).await.unwrap();
    assert_eq!(stats.sent, 2);

    // The signature is verifiable with the shared secret over the exact body.
    let requests = webhook.sent.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.url, "https://example.org/hook");
    assert_eq!(
        request.signature,
        signature_hex("s3cret-030", &request.body)
    );
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"s3cret-030").unwrap();
    mac.update(request.body.as_bytes());
    assert_eq!(
        request.signature,
        format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
        "independent HMAC recomputation matches"
    );

    // Honesty travels with the fast path (design §6.3).
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["record"]["verification_state"], json!("unverified"));
    assert!(body["record"]["extraction_confidence"].is_number());
    assert_eq!(
        body["provenance_url"],
        json!("https://govfolio.io/filings/0FILING0000000000000000001")
    );
    assert!(
        body["record"]["value"]["low"].is_string(),
        "money stays decimal strings on the wire (invariant 7)"
    );

    // Email side: same honesty fields in the text body.
    let emails = email.sent.lock().unwrap().clone();
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].to, "alerts@example.org");
    for needle in ["unverified", "0.97", "/filings/0FILING0000000000000000001"] {
        assert!(
            emails[0].text.contains(needle),
            "email body must carry {needle:?}:\n{}",
            emails[0].text
        );
    }

    // Ledger: both rows sent, attempts recorded.
    let states: Vec<(String, i32)> =
        sqlx::query_as("select status, attempts from delivery order by channel")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(states, vec![("sent".to_owned(), 1), ("sent".to_owned(), 1)]);
}

/// Retries with backoff, then the DLQ: after `max_attempts` failures the row
/// goes `dead` with the attempts counter and last error recorded.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alerts_retry_backoff_then_dead(pool: PgPool) {
    seed_graph(&pool).await;
    seed_record(
        &pool,
        1,
        "0HSEMBR0000000000000000001",
        "transaction",
        "equity",
        Some(D(2026, 6, 1)),
        None,
        None,
    )
    .await;
    seed_rule(
        &pool,
        "0RULE000000000000000000001",
        json!({}),
        json!([{ "type": "webhook", "url": "https://example.org/hook", "secret": "x" }]),
        false,
    )
    .await;
    match_pass(&pool, &fast_config()).await.unwrap();

    let email = MockEmail::default();
    let webhook = MockWebhook {
        fail: true,
        ..MockWebhook::default()
    };
    let senders = Senders {
        email: &email,
        webhook: &webhook,
    };
    let stats = send_pass(&pool, &fast_config(), &senders).await.unwrap();
    assert_eq!((stats.sent, stats.dead), (0, 1));

    let (status, attempts, last_error): (String, i32, Option<String>) =
        sqlx::query_as("select status, attempts, last_error from delivery")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "dead", "DLQ = delivery rows with status dead");
    assert_eq!(attempts, 3, "the full retry budget was spent");
    assert!(last_error.unwrap().contains("endpoint 503"));

    // Dead rows are terminal: another pass never resends them.
    let stats = send_pass(&pool, &fast_config(), &senders).await.unwrap();
    assert_eq!((stats.sent, stats.dead), (0, 0));
}

/// Digest mode: deliveries accumulate as `pending_digest`; the digest pass
/// sends ONE summary per (rule, channel) covering all pending records.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn alerts_digest_groups_per_rule(pool: PgPool) {
    seed_graph(&pool).await;
    seed_record(
        &pool,
        1,
        "0HSEMBR0000000000000000001",
        "transaction",
        "equity",
        Some(D(2026, 6, 1)),
        None,
        Some(("1001.00", "15000.00")),
    )
    .await;
    seed_record(
        &pool,
        2,
        "0HSEMBR0000000000000000001",
        "holding",
        "fund",
        None,
        Some(D(2026, 6, 2)),
        None,
    )
    .await;
    seed_rule(
        &pool,
        "0RULE000000000000000000001",
        json!({}),
        both_channels(),
        true,
    )
    .await;

    match_pass(&pool, &fast_config()).await.unwrap();
    let pending: i64 =
        sqlx::query_scalar("select count(*) from delivery where status = 'pending_digest'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        pending, 4,
        "2 events x 2 channels accumulate for the digest"
    );

    // The instant pass must NOT touch digest rows.
    let email = MockEmail::default();
    let webhook = MockWebhook::default();
    let senders = Senders {
        email: &email,
        webhook: &webhook,
    };
    let stats = send_pass(&pool, &fast_config(), &senders).await.unwrap();
    assert_eq!(stats.sent, 0);

    let stats = digest_pass(&pool, &fast_config(), &senders).await.unwrap();
    assert_eq!(stats.sent, 4, "all four rows settle");

    let emails = email.sent.lock().unwrap().clone();
    assert_eq!(emails.len(), 1, "ONE digest email for two records");
    for needle in ["Seed asset 1", "Seed asset 2", "unverified"] {
        assert!(emails[0].text.contains(needle), "digest lists {needle:?}");
    }
    let requests = webhook.sent.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "ONE digest webhook for two records");
    let body: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(body["records"].as_array().unwrap().len(), 2);
    assert_eq!(
        requests[0].signature,
        signature_hex("s3cret-030", &requests[0].body)
    );

    let settled: i64 = sqlx::query_scalar("select count(*) from delivery where status = 'sent'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settled, 4);
}
