//! sentinel WATCH (goal 017, design §5.6/§5.8): one drift-defense pass over the
//! live sources, or a self-paced loop. The WEEKLY cadence is Cloud Scheduler
//! (infra/scheduler.tf, goal 020) — this bin does the work of one pass.
//!
//! Usage:
//!   cargo run -p worker --bin sentinel [-- --once]   (default: one pass, exit)
//!   cargo run -p worker --bin sentinel -- --loop      (repeat every interval)
//!
//! Env: `DATABASE_URL` (required); `SENTINEL_CONTACT` (UA contact, optional);
//! `SENTINEL_INTERVAL_SECS` (loop spacing, default 604800 = weekly).

use std::time::Duration;

use anyhow::Context as _;

use worker::sentinel::{HttpProbe, PgWatchStore, WatchSummary, live_targets, watch_pass};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let looping = std::env::args().any(|a| a == "--loop");
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let contact =
        std::env::var("SENTINEL_CONTACT").unwrap_or_else(|_| "ops@govfolio.io".to_owned());

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    let probe = HttpProbe::new(contact)?;
    let store = PgWatchStore::new(pool);
    let targets = live_targets();

    if !looping {
        report(&watch_pass(&probe, &store, &targets).await?);
        return Ok(());
    }

    // Default cadence is weekly, matching Cloud Scheduler (infra/scheduler.tf);
    // the loop mode exists mainly for local/standalone runs.
    let weekly_secs: u64 = 7 * 24 * 60 * 60;
    let interval = std::env::var("SENTINEL_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or_else(|| Duration::from_secs(weekly_secs), Duration::from_secs);
    loop {
        match watch_pass(&probe, &store, &targets).await {
            Ok(summary) => report(&summary),
            // A pass failure is loud but non-fatal: the loop keeps watching.
            Err(error) => eprintln!("sentinel: pass failed: {error:#}"),
        }
        tokio::time::sleep(interval).await;
    }
}

fn report(summary: &WatchSummary) {
    println!(
        "sentinel: checked {} source(s), {} drift filed, {} re-detected",
        summary.checked, summary.filed, summary.redetected
    );
    for drift in &summary.reports {
        println!(
            "  [{:>5.0}] {:<13} {} ({})",
            drift.priority_score,
            drift.kind.as_str(),
            drift.regime_code,
            if drift.freeze { "FROZEN" } else { "filed" }
        );
    }
}
