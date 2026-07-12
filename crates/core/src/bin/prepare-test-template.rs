//! Prime `template1` with the full migrated schema so every `#[sqlx::test]` DB —
//! which sqlx creates via `CREATE DATABASE` cloning `template1` (sqlx-postgres
//! `testing`: it issues `create database "…"` with no TEMPLATE clause) — is born
//! already migrated. That turns each test's `govfolio_core::db::migrate()` call
//! into a cheap no-op and replaces the ~1300 per-test migration replays across the
//! `--ignored` suite with this single one.
//!
//! Safe-degrading: if this is NOT run, `#[sqlx::test]` DBs are empty and each test
//! migrates normally (correct, just slow) — exactly the pre-existing behavior. It
//! only ever touches `template1`; existing databases (the durable local dataset in
//! `govfolio`) are untouched, since `CREATE DATABASE` is the only consumer of a
//! template.
//!
//! Run once before the DB suite:
//!   cargo run -p core --bin prepare-test-template
//!   cargo test --workspace -- --ignored
#![allow(clippy::unwrap_used, clippy::expect_used)]

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection as _, PgConnection};

/// The same advisory-lock constant sqlx's own test harness takes before it runs
/// `CREATE DATABASE` (sqlx-postgres `testing`: `pg_advisory_xact_lock(…)`, the 8
/// ascii bytes `sqlxtest`). Holding it while `template1` is rebuilt guarantees no
/// concurrent test clones a half-built template.
const SQLX_TEST_LOCK: i64 = 8_318_549_251_334_697_844;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set (the test Postgres server)"))?;

    // Maintenance connection (the DATABASE_URL database). A single PgConnection so
    // the advisory lock is bound to one session and released deterministically.
    let mut maint = PgConnection::connect(&url).await?;
    sqlx::query("select pg_advisory_lock($1)")
        .bind(SQLX_TEST_LOCK)
        .execute(&mut maint)
        .await?;

    // Rebuild template1's public schema from the real migrator on a short-lived
    // one-connection pool. `drop schema … cascade` clears any prior priming so a
    // changed migration set can never leave a stale template behind.
    let t1_opts: PgConnectOptions = url.parse::<PgConnectOptions>()?.database("template1");
    let t1 = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(t1_opts)
        .await?;
    sqlx::raw_sql("drop schema if exists public cascade; create schema public;")
        .execute(&t1)
        .await?;
    govfolio_core::db::migrate(&t1).await?;
    t1.close().await; // disconnect template1 BEFORE releasing the lock (returns ())

    sqlx::query("select pg_advisory_unlock($1)")
        .bind(SQLX_TEST_LOCK)
        .execute(&mut maint)
        .await?;
    maint.close().await?;

    eprintln!(
        "template1 primed: schema migrated — #[sqlx::test] DBs now clone a ready schema (db::migrate becomes a no-op)"
    );
    Ok(())
}
