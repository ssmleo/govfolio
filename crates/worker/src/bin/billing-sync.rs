//! billing-sync (goal 050): rolls unbilled `usage_event` rows into
//! `usage_report` rows and sends them to Stripe as meter events —
//! exactly-once via report-ULID identifiers (see `worker::billing`).
//!
//! Usage: `cargo run -p worker --bin billing-sync` — one pass, then exit
//! (cadence belongs to the scheduler, Cloud Scheduler later).
//!
//! Env: `DATABASE_URL` (required); `STRIPE_SECRET_KEY` (required — without
//! it the bin refuses to run: fail closed, no pretend mode outside tests).

use anyhow::Context as _;

use worker::billing::billing_sync_pass;
use worker::stripe::HttpStripeClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    let Some(stripe) = HttpStripeClient::from_env()? else {
        anyhow::bail!(
            "STRIPE_SECRET_KEY is not set — refusing to run (fail closed; \
             usage stays unbilled in the ledger until billing is configured)"
        );
    };
    let stats = billing_sync_pass(&pool, &stripe).await?;
    println!(
        "billing-sync: {} report(s) created ({} event(s)), {} report(s) accepted by Stripe",
        stats.reports_created, stats.events_billed, stats.reports_sent
    );
    Ok(())
}
