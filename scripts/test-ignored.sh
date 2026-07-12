#!/usr/bin/env sh
# Fast DB-gated test run. Two levers over `cargo test --workspace -- --ignored`:
#   1. Prime template1 so every #[sqlx::test] DB is born already migrated —
#      each test's govfolio_core::db::migrate() becomes a no-op instead of
#      replaying 14 migrations (crates/core/src/bin/prepare-test-template.rs).
#   2. Run via cargo-nextest, which schedules all test binaries in one global
#      parallel pool (~8x `cargo test`'s binary-by-binary run on this DB suite).
#
# Requires cargo-nextest (https://get.nexte.st) and postgres on DATABASE_URL.
# Extra args are forwarded to nextest, e.g. `scripts/test-ignored.sh -p worker`.
set -eu
: "${DATABASE_URL:=postgres://postgres:postgres@localhost:5433/govfolio}"
export DATABASE_URL
cargo run -p core --bin prepare-test-template
cargo nextest run --workspace --run-ignored ignored-only "$@"
