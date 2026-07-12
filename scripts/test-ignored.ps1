# Fast DB-gated test run (PowerShell mirror of scripts/test-ignored.sh).
#   1. Prime template1 so every #[sqlx::test] DB is born migrated (db::migrate → no-op).
#   2. Run the #[ignore] suites via cargo-nextest (global parallel scheduling, ~8x
#      cargo test on this DB suite).
# Requires cargo-nextest (https://get.nexte.st) and postgres on DATABASE_URL.
# Extra args are forwarded to nextest, e.g. `./scripts/test-ignored.ps1 -p worker`.
if (-not $env:DATABASE_URL) { $env:DATABASE_URL = 'postgres://postgres:postgres@localhost:5433/govfolio' }
cargo run -p core --bin prepare-test-template
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo nextest run --workspace --run-ignored ignored-only @args
