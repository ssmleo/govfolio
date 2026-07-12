# Non-Recursive Role Evals with Explicit Full Gate

- **Date:** 2026-07-12
- **Status:** Approved for implementation
- **Supersedes:** the recorded-evidence Rust-builder boundary in
  `docs/plans/2026-07-11-build-test-performance.md`

## Problem and accepted evidence

The original `role_evals_epoch_gate_e2_open` test invoked a Rust-builder scorer that
started a second `cargo test --workspace` under `target/role-evals-nested`. The recursion
guard prevented infinity but did not prevent every ordinary workspace suite from paying
for the second suite.

Accepted Windows baseline, not to be re-proven:

- fresh scratch `cargo test --workspace --no-run`: 122.912 seconds;
- full workspace suite on the warmed scratch target: 1161.204 seconds;
- the 11-test `pipeline` role-evals binary: approximately 708.76 seconds;
- the first ten tests were fast, but concurrency means the heavy block is reported as
  approximately 708.76 seconds rather than an exact subtraction; and
- the block accounts for roughly 61% of the observed full-suite wall time.

The first non-recursive implementation replaced real Rust-builder execution with frozen
goal/JOURNAL prose. That removed the performance defect but weakened the explicit epoch
gate: intentionally invoking it no longer exercised current repository acceptance. This
design corrects that ownership boundary without restoring recursion.

## Designs considered

1. Add logic-only/full modes to `evals::gate()`. This leaves an ambiguous operational
   entry point whose logic-only result cannot honestly establish current Rust-builder
   acceptance.
2. Put command planning and execution directly in `epoch_gate.rs`. Ownership is visible,
   but domain orchestration becomes difficult to unit-test without executing Cargo.
3. **Selected:** keep process-free evaluation in the library and add a separate typed,
   injectable full-gate path. The binary calls only the full path; ordinary tests call
   only the evaluator with synthetic evidence.

## Architecture

`evals::score_artifact_role(root, role)` scores the six filesystem/schema roles and
rejects `Role::RustBuilder`. `evals::evaluate_gate(root, epoch, rust_builder)` verifies
the frozen lock, inserts explicit Rust-builder evidence in `Role::ALL` order, and applies
the existing 1.00 thresholds and blockers without starting a process.

`evals::full_gate(root, epoch)` is the operational entry point. It validates the epoch
before requesting work, resolves the Cargo executable, and delegates to an injectable
runner over four typed command specifications:

1. `cargo run --quiet -p pipeline --bin conformance -- us_house`, requiring the stdout
   marker `5/5 cases green`;
2. `cargo fmt --check`;
3. `cargo clippy --all-targets -- -D warnings`; and
4. `cargo test --workspace`.

All four results become the Rust-builder `Outcome`. Spawn failure, nonzero exit, or a
missing marker is a failed check and therefore a blocker. The production runner inherits
the caller environment. No command specification sets `GOVFOLIO_ROLE_EVALS_INNER` or
`CARGO_TARGET_DIR`, and no code constructs `target/role-evals-nested`.

The explicit binary retains its existing report format and exit mapping. Ordinary tests
verify artifact scoring, lock integrity, supported/unsupported epochs, `NOT_APPLICABLE`,
threshold reducers, exact command planning, and injected failure behavior without
spawning Cargo.

## CI and operations

The Rust CI job continues to execute fmt, strict Clippy, `cargo test --workspace`, and
us_house conformance once each as visible commit-bound gates. CI does not call
`epoch-gate`, because that would intentionally repeat the same workspace suite.

Factory/runbook calls to `cargo run -p pipeline --bin epoch-gate -- E2` are explicitly
documented as heavyweight repository certification. `cargo test -p pipeline --test
role_evals` is process-free scoring/gate-logic acceptance and never substitutes for the
explicit command.

## Invariants

- `Role::ALL`, thresholds, frozen-reference integrity, `NOT_APPLICABLE`, blockers, and E2
  behavior do not change.
- The Rust-builder role and all four checks remain mandatory; no check is ignored or
  replaced by prose evidence.
- Ordinary workspace tests launch zero nested Cargo workspace suites.
- The ignored-only nextest job is unchanged; no `#[ignore]` workaround is introduced.
- The change creates no schema, generated-contract, persistent receipt, or migration.

## Rollout and rollback

Land code, tests, CI comments, decision text, runbooks, and measurements in one PR. Do
not delete an existing nested target directory or touch the main target. Rollback is a
normal revert of the PR; there is no persisted state to migrate. A Windows lock failure
in the explicit command is a NO-GO requiring runner redesign, never permission to restore
ordinary-test recursion or drop a check.

