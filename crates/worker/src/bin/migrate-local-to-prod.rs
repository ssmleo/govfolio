//! `migrate-local-to-prod` (per founder-directed policy, 2026-07-09 session
//! direction: historical backfills must run only against local dev
//! Postgres; prod receives data via migration of the already-collected
//! local dataset, not by re-running the backfill pipeline against prod —
//! pending write-back into a future root `CLAUDE.md` invariant): copies one
//! regime's already-collected LOCAL Gold dataset into PROD, idempotently.
//! Thin CLI wrapper — all logic lives in
//! [`worker::migrate_local_to_prod`] (kept there, not here, so
//! `crates/worker/tests/migrate_local_to_prod.rs` can exercise it against
//! two local ephemeral Postgres databases without shelling out to this
//! binary, and inject a fake [`worker::migrate_local_to_prod::BronzeUploader`]
//! instead of touching real GCS/gcloud).
//!
//! Never runs discovery/fetch/parse/normalize against PROD — this bin ONLY
//! copies rows + Bronze bytes a LOCAL `backfill-real`/`backfill-real-br` run
//! already produced. Safe to re-run: a second invocation transfers only
//! whatever's new since the last run (every write is `ON CONFLICT (id) DO
//! NOTHING`, invariant 4).
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin migrate-local-to-prod -- --regime <regime_id>
//! ```
//! `<regime_id>` is an existing LOCAL `disclosure_regime.id` (e.g.
//! `us_house::seed::REGIME_ID`) — NOT an adapter code like `us_house`.
//!
//! Env: `LOCAL_DATABASE_URL` (default
//! `postgres://postgres:postgres@localhost:5433/govfolio`), `PROD_DATABASE_URL`
//! (required — via the Cloud SQL Auth Proxy; the caller is responsible for
//! having the proxy running, this bin only connects to whatever it points
//! at), `LOCAL_BRONZE_ROOT` (default `<durable_bronze_parent()>/bronze-backfill-real`
//! — i.e. `target/` or `GOVFOLIO_BRONZE_ROOT`, matching `bin/backfill-real.rs`'s
//! own default), `PROD_BRONZE_BUCKET` (default `govfolio-bronze`).
//!
//! Exit code: 0 even when individual rows failed to migrate (per-row
//! isolation is the design, not an error — see the printed report's `failed`
//! counts) — nonzero only on genuine connection/setup failure (bad
//! `--regime`, unreachable LOCAL/PROD).

use std::path::PathBuf;

use anyhow::Context as _;

use worker::migrate_local_to_prod::{GcloudUploader, migrate_regime};

fn parse_regime_arg() -> anyhow::Result<String> {
    let mut regime_id = None;
    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        match flag.as_str() {
            "--regime" => {
                regime_id = Some(
                    cli.next()
                        .context("--regime requires a value (a LOCAL disclosure_regime.id)")?,
                );
            }
            other => anyhow::bail!("unknown argument {other:?} (expected --regime <regime_id>)"),
        }
    }
    regime_id.context("--regime is required, e.g. --regime 0HSEREG0000000000000000001")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let regime_id = parse_regime_arg()?;

    let local_url = std::env::var("LOCAL_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/govfolio".to_owned());
    let prod_url = std::env::var("PROD_DATABASE_URL").context(
        "PROD_DATABASE_URL must point at PROD Postgres (via the Cloud SQL Auth Proxy — start \
         it first; this bin only connects, it does not manage the proxy)",
    )?;
    let local = sqlx::PgPool::connect(&local_url)
        .await
        .context("connecting to LOCAL Postgres")?;
    let prod = sqlx::PgPool::connect(&prod_url)
        .await
        .context("connecting to PROD Postgres")?;

    let bronze_root = std::env::var("LOCAL_BRONZE_ROOT").map_or_else(
        |_| pipeline::conformance::durable_bronze_parent().join("bronze-backfill-real"),
        PathBuf::from,
    );
    let bucket =
        std::env::var("PROD_BRONZE_BUCKET").unwrap_or_else(|_| "govfolio-bronze".to_owned());
    let uploader = GcloudUploader { bucket };

    println!("migrating regime {regime_id}: LOCAL -> PROD...");
    let report = migrate_regime(&local, &prod, &regime_id, &bronze_root, &uploader).await?;
    report.print();
    Ok(())
}
