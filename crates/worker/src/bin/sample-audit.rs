//! Monthly sampling-audit job (design §7.4, goal 070): draws a deterministic,
//! stratified-per-regime sample of published Gold records, queues them into
//! `sample_audit`, and prints the precision report. Cloud Scheduler cadence
//! (monthly); this bin does one batch.
//!
//! Usage:
//!   `cargo run -p worker --bin sample-audit [-- <YYYY-MM> <per_regime> <seed>]`
//!
//! Env: `DATABASE_URL` (required). Args default to the current UTC month, 30
//! records/regime, and a month-derived seed (so a given month is reproducible).

use anyhow::Context as _;

use worker::sampler::run_sampling_audit;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let mut args = std::env::args().skip(1);
    let month = args
        .next()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
    let per_regime: usize = args
        .next()
        .map_or(Ok(30), |s| s.parse())
        .context("per_regime must be a non-negative integer")?;
    let seed: i64 = match args.next() {
        Some(s) => s.parse().context("seed must be an integer")?,
        None => month_seed(&month),
    };

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    let report = run_sampling_audit(&pool, &month, per_regime, seed).await?;

    println!(
        "sampling audit {} (per_regime={per_regime}, seed={seed}): {} regime(s)",
        report.sample_month,
        report.regimes.len()
    );
    for r in &report.regimes {
        let precision = r
            .precision_estimate
            .map_or_else(|| "n/a".to_owned(), |p| format!("{p:.4}"));
        println!(
            "  {:<28} sampled={:<4} audited={:<4} discrepancies={:<3} precision={precision}",
            r.body, r.sampled, r.audited, r.discrepancies
        );
    }
    Ok(())
}

/// A stable seed derived from the batch month, so re-running the same month
/// draws the same sample without an explicit seed argument.
fn month_seed(month: &str) -> i64 {
    // FNV-1a 64-bit over the month string, truncated into i64 range.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in month.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    #[allow(clippy::cast_possible_wrap)] // opaque draw key; wrap is fine
    let seed = hash as i64;
    seed
}
