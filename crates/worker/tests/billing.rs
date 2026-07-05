//! billing-sync (goal 050): `usage_event` -> `usage_report` -> Stripe meter
//! events, exactly-once. Mock transport — no Stripe credentials anywhere.
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use sqlx::PgPool;

use worker::billing::billing_sync_pass;
use worker::stripe::StripeClient;

/// Records every call; optionally refuses (transport failure injection).
#[derive(Default)]
struct MockStripe {
    usage_calls: Mutex<Vec<(String, u64, String)>>,
    fail: AtomicBool,
}

impl MockStripe {
    fn calls(&self) -> Vec<(String, u64, String)> {
        self.usage_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl StripeClient for MockStripe {
    async fn create_customer(&self, _email: &str) -> anyhow::Result<String> {
        Ok("cus_mock".to_owned())
    }

    async fn checkout_link(
        &self,
        _customer_id: &str,
        _price_id: &str,
        _tier: &str,
    ) -> anyhow::Result<String> {
        Ok("https://checkout.stripe.test/mock".to_owned())
    }

    async fn report_usage(
        &self,
        customer_id: &str,
        quantity: u64,
        idempotency_key: &str,
    ) -> anyhow::Result<()> {
        if self.fail.load(Ordering::SeqCst) {
            anyhow::bail!("injected transport failure");
        }
        self.usage_calls.lock().unwrap().push((
            customer_id.to_owned(),
            quantity,
            idempotency_key.to_owned(),
        ));
        Ok(())
    }
}

async fn seed_user(pool: &PgPool, email: &str, customer: Option<&str>, subscribed: bool) -> String {
    let user_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "insert into user_account (id, email, tier, stripe_customer_id) \
         values ($1, $2, 'data', $3)",
    )
    .bind(&user_id)
    .bind(email)
    .bind(customer)
    .execute(pool)
    .await
    .unwrap();
    if subscribed {
        sqlx::query(
            "insert into subscription (id, user_id, stripe_subscription_id, status) \
             values ($1, $2, $3, 'active')",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(&user_id)
        .bind(format!("sub_{user_id}"))
        .execute(pool)
        .await
        .unwrap();
    }
    user_id
}

async fn add_events(pool: &PgPool, user_id: &str, n: usize) {
    for _ in 0..n {
        sqlx::query(
            "insert into usage_event (id, user_id, endpoint) values ($1, $2, '/v1/records')",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn unbilled(pool: &PgPool, user_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from usage_event where user_id = $1 and report_id is null")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn billing_rolls_unbilled_usage_into_idempotent_reports(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let billable = seed_user(&pool, "data@test", Some("cus_data"), true).await;
    let free_rider = seed_user(&pool, "nosub@test", Some("cus_nosub"), false).await;
    add_events(&pool, &billable, 3).await;
    add_events(&pool, &free_rider, 5).await;

    // Pass 1: one report, quantity 3, sent under the report id.
    let stripe = MockStripe::default();
    let stats = billing_sync_pass(&pool, &stripe).await.unwrap();
    assert_eq!(stats.reports_created, 1);
    assert_eq!(stats.events_billed, 3);
    assert_eq!(stats.reports_sent, 1);
    let calls = stripe.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "cus_data");
    assert_eq!(calls[0].1, 3, "aggregated quantity");
    let report_id: String = sqlx::query_scalar(
        "select id from usage_report where user_id = $1 and reported_at is not null",
    )
    .bind(&billable)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(calls[0].2, report_id, "identifier = report ULID");
    assert_eq!(unbilled(&pool, &billable).await, 0, "events stamped");

    // No subscription = never billed (events stay in the ledger).
    assert_eq!(unbilled(&pool, &free_rider).await, 5);

    // Pass 2: nothing new — no reports, no calls (idempotent).
    let stats = billing_sync_pass(&pool, &stripe).await.unwrap();
    assert_eq!(stats, worker::billing::BillingStats::default());
    assert_eq!(stripe.calls().len(), 1);

    // New usage rolls into a NEW report next pass.
    add_events(&pool, &billable, 2).await;
    let stats = billing_sync_pass(&pool, &stripe).await.unwrap();
    assert_eq!((stats.reports_created, stats.events_billed), (1, 2));
    assert_eq!(stripe.calls().len(), 2);
    assert_eq!(stripe.calls()[1].1, 2);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn billing_send_failure_retries_same_identifier_never_double_bills(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let billable = seed_user(&pool, "data@test", Some("cus_data"), true).await;
    add_events(&pool, &billable, 4).await;

    // Send fails: the report exists (events stamped) but stays unreported.
    let stripe = MockStripe::default();
    stripe.fail.store(true, Ordering::SeqCst);
    let stats = billing_sync_pass(&pool, &stripe).await.unwrap();
    assert_eq!((stats.reports_created, stats.reports_sent), (1, 0));
    assert_eq!(
        unbilled(&pool, &billable).await,
        0,
        "stamping is atomic with the report"
    );
    let report_id: String = sqlx::query_scalar(
        "select id from usage_report where user_id = $1 and reported_at is null",
    )
    .bind(&billable)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Recovery pass: the SAME report id goes out (Stripe-side dedup key),
    // quantity unchanged — crash-between-send-and-stamp can only resend,
    // never re-aggregate.
    stripe.fail.store(false, Ordering::SeqCst);
    let stats = billing_sync_pass(&pool, &stripe).await.unwrap();
    assert_eq!((stats.reports_created, stats.reports_sent), (0, 1));
    let calls = stripe.calls();
    assert_eq!(calls, vec![("cus_data".to_owned(), 4, report_id)]);
}
