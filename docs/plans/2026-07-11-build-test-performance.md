# Rust build and database-test performance

- **Date:** 2026-07-11
- **Status:** Approved by founder for implementation outside the loop
- **Branch:** `codex/build-test-performance`, based on fresh `main`

## Invariants

- Preserve every required Rust, database, contract, web, authority, and guardrail check.
- Preserve SQLx database-per-test isolation, protected-main enforcement, and fail-closed behavior.
- Never accept persisted or stale green evidence; the final release verdict is bound to the exact commit.
- Parallel agents keep private Cargo targets. Share registry data and compiler-cache entries, never one mutable target.
- Bronze/raw data remains immutable and must be separated from disposable compiler artifacts before cleanup.

## Role-gate semantics

Refactor the E1 role scorer into a pure deterministic evaluation of frozen reference artifacts and recorded calibration evidence. Remove nested Cargo subprocesses, the recursion sentinel, and the nested target directory. The epoch gate must still block on lock drift, missing artifacts, `NOT_APPLICABLE`, or a score below 1.00. Current-code fmt, Clippy, workspace tests, conformance, ignored SQLx tests, and contract verification remain explicit release checks.

Owned files:

- `crates/pipeline/src/evals/mod.rs`
- `crates/pipeline/src/evals/roles.rs`
- `crates/pipeline/tests/role_evals.rs`
- `crates/pipeline/src/bin/epoch_gate.rs`
- `docs/decisions/role-eval-thresholds.md`

Acceptance: role-eval and epoch-gate tests pass; source contains no child-Cargo execution or `GOVFOLIO_ROLE_EVALS_INNER`; fail-closed negative cases remain covered.

## CI metrics, cache, and OpenAPI

Instrument `.github/workflows/ci.yml` with stable named gates, elapsed-time summaries, cache-hit status, commit/toolchain metadata, and a non-compiling aggregate `release-gate` that fails unless all required jobs succeed. Give Rust and DB jobs a shared dependency-cache namespace without concurrent saves. Retain pnpm caching and all existing checks. Add a byte-for-byte OpenAPI snapshot integration test while keeping the existing regeneration drift command during the parity phase.

Owned files:

- `.github/workflows/ci.yml`
- `crates/api/tests/openapi_snapshot.rs`

Acceptance: workflow syntax is valid; every existing gate remains; aggregate status is fail-closed; OpenAPI snapshot passes and detects byte drift.

## Local Windows cache and Bronze safety

Add an optional PowerShell Cargo wrapper that uses `sccache` only when installed, stores its cache below `%LOCALAPPDATA%\govfolio\sccache`, disables incremental compilation for cache eligibility, and leaves `CARGO_TARGET_DIR` private to the worktree. Change the loop's default shared Bronze root from `target/` to a dedicated sibling directory, document hash-and-count migration, and never delete or move existing raw data as part of this change.

Owned files:

- `scripts/dev/cargo-agent.ps1`
- `docs/runbooks/dev-host-windows.md`
- `docs/runbooks/parallel-factory.md`
- `agents/run-loop.sh`

Acceptance: wrapper fails clearly without `sccache`, forwards Cargo exit codes, and never changes target sharing; shell syntax passes; docs explicitly prohibit cleaning old Bronze until verified migration.

## Integration, governance, and shadow pilots

After the three independent workstreams land, align builder/orchestrator/auditor language so narrow checks run during iteration and the complete block runs once on the exact integrated tree. Add shadow-only nextest configuration/workflow with no retries, separate doctests, and database concurrency capped at four; it must not become required in this change. Record measured findings in the append-only journal. Conditional test-profile changes and prepared database templates remain deferred until their measurement thresholds are met.

Integrator-owned files:

- `CLAUDE.md`
- `agents/workflows/orchestration.md`
- `agents/workflows/factory-lane.md`
- `agents/roles/rust-builder.md`
- `agents/roles/auditor.md`
- `.config/nextest.toml`
- `.github/workflows/perf-shadow.yml`
- `agents/JOURNAL.md`

Final acceptance: fmt, Clippy, workspace tests, `us_house` conformance, full ignored SQLx suite against local PostgreSQL, OpenAPI no-diff, authority/skill guardrails, web lint/typecheck/test, and workflow checks all pass. Commit on the branch; never edit protected `main` directly.
