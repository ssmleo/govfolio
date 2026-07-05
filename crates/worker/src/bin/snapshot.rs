//! snapshot (goal 050): full Gold export as gzipped CSV + JSONL with a
//! sha256 manifest and CC BY note (see `worker::snapshot`). Monthly cadence
//! belongs to the scheduler (Cloud Scheduler later); GCS upload is deploy
//! plumbing, later — output lands in a dated local directory.
//!
//! Usage: `cargo run -p worker --bin snapshot -- [--out DIR]`
//! (default `./snapshots`; the export lands in `DIR/govfolio-YYYY-MM/`).
//!
//! Env: `DATABASE_URL` (required).

use anyhow::Context as _;

use worker::snapshot::run_snapshot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut out = std::path::PathBuf::from("snapshots");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => out = args.next().context("--out requires a directory")?.into(),
            other => anyhow::bail!("unknown argument {other:?} (expected --out DIR)"),
        }
    }
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    let dir = out.join(format!("govfolio-{}", chrono::Utc::now().format("%Y-%m")));
    let outcome = run_snapshot(&pool, &dir).await?;
    println!(
        "snapshot: {} record(s) -> {} (manifest sha256s inside MANIFEST.json)",
        outcome.manifest.record_count,
        outcome.dir.display()
    );
    Ok(())
}
