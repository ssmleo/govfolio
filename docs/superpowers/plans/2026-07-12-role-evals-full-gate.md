# Non-Recursive Role Evals with Explicit Full Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` or `superpowers:executing-plans` and follow
> test-first red/green/refactor.

**Goal:** Keep ordinary workspace tests process-free while restoring the explicit E2
gate as the fail-closed owner of real Rust-builder repository acceptance.

**Architecture:** A process-free evaluator consumes explicit Rust-builder evidence. A
typed command-plan and injected runner produce that evidence only for `full_gate`, which
is the sole path called by the binary.

**Tech stack:** Rust 2024, Cargo subprocesses, GitHub Actions, PowerShell measurements.

## Global constraints

- Do not change `Role::ALL`, 1.00 thresholds, lock pins, `NOT_APPLICABLE`, blockers, or
  E2 semantics.
- Do not add `#[ignore]`, a second ordinary CI suite, or recorded-prose acceptance.
- Do not edit `CLAUDE.md` or `agents/goals/000-INDEX.md`.
- Use private scratch targets; never write to or delete the main `target/`.
- Keep the work in one reversible PR.

## Task 1 — Prove the boundary red

- [x] Add `ordinary_gate_evaluation_does_not_request_repository_acceptance`; confirm the
  old two-argument gate fails to compile with explicit Rust-builder evidence.
- [x] Add `explicit_full_gate_requests_complete_rust_builder_block`; confirm the typed
  spec/runner/full-gate API is missing.
- [x] Add `explicit_full_gate_rejects_unwired_epoch_before_commands`; confirm the initial
  implementation incorrectly requests commands before rejecting E3.

Focused commands:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'govfolio-role-evals-impl-20260712'
cargo test -p pipeline --lib evals::tests::ordinary_gate_evaluation_does_not_request_repository_acceptance -- --exact
cargo test -p pipeline --lib evals::acceptance::tests::explicit_full_gate_requests_complete_rust_builder_block -- --exact
cargo test -p pipeline --lib evals::acceptance::tests::explicit_full_gate_rejects_unwired_epoch_before_commands -- --exact
```

## Task 2 — Implement the separated paths

- [x] Create `crates/pipeline/src/evals/acceptance.rs` with `CommandSpec`,
  `CommandRunner`, `ProcessCommandRunner`, and the exact four-command planner.
- [x] Replace `score_role` with `score_artifact_role`; remove recorded goal/JOURNAL
  Rust-builder scoring.
- [x] Add `evaluate_gate`, `full_gate`, and the private injected full-gate function.
- [x] Validate the epoch before requesting commands and preserve all four results.
- [x] Change `epoch_gate.rs` to call only `full_gate`.
- [x] Update the integration suite to use synthetic evidence and remove the standalone
  recorded-evidence Rust-builder assertion.

## Task 3 — Prove failure semantics and focused correctness

- [x] Use a recording runner to prove exact command order, marker, Cargo program, and
  empty environment overrides without spawning.
- [x] Use a failing runner to prove a workspace-test failure scores Rust-builder below
  threshold, remains a named blocker, and closes E2.
- [x] Run all evaluator, acceptance, and role-eval tests plus fmt and strict Clippy.
- [x] Confirm active code contains no `role-evals-nested` or recursion environment.

## Task 4 — Clarify CI and runbooks

- [x] Amend `role-eval-thresholds.md` to supersede recorded prose with explicit command
  evidence while preserving governance semantics.
- [x] Keep one ordinary CI workspace suite and explain why CI does not call full gate.
- [x] Mark factory/runbook epoch-gate calls as intentional heavyweight certification.
- [x] Supersede the old role-gate section in the build-performance plan.

## Task 5 — Measure after implementation

Use separate fresh scratch targets for cold/warm no-run, cold/warm full tests, and the
explicit gate. Record raw values in this document and the append-only JOURNAL. Do not
rerun the old recursive baseline. The cold full sample is a conservative comparison
against the accepted warmed-target baseline; do not add a duplicate full-suite sample
after the required cold/warm matrix is complete.

GO requires:

- cold full suite at most 696.722 seconds (at least 40% below 1161.204);
- cold no-run at most 137.912 seconds;
- zero nested Cargo workspace runs and no nested target directory;
- explicit E2 gate green with unchanged output/exit behavior;
- injected required-check failure remains closed; and
- focused, workspace, fmt, and strict-Clippy correctness green.

## Task 6 — Integrate

- [x] Append measured results and the ownership correction to `agents/JOURNAL.md`.
- [ ] Verify the exact diff, commit all intended files, and merge the verified branch to
  `main`.
- [ ] Re-run focused tests plus the ordinary workspace suite on the merged commit using
  a private scratch target.

## Implementation results

Candidate source commit: `3e6a9b427d1e0293c525371309b5b01ab602503e`.
All samples used Windows stable Rust, `CARGO_BUILD_JOBS=14`, and separate previously
absent targets under `%TEMP%`; the repository `target/` was neither used nor changed.

| Measurement | Accepted before | Measured after | Threshold | Result |
| --- | ---: | ---: | ---: | --- |
| Cold `cargo test --workspace --no-run` | 122.912 s | 141.412 s | <= 137.912 s | NO-GO by 3.500 s |
| Warm `cargo test --workspace --no-run` | not supplied | 0.822 s | informational | PASS |
| Cold `cargo test --workspace` | 1161.204 s (warmed-target baseline) | 571.420 s | <= 696.722 s | GO; 50.79% lower |
| Warm `cargo test --workspace` | not supplied | 15.099 s | informational | PASS |
| Explicit `cargo run -p pipeline --bin epoch-gate -- E2` | heavyweight by contract | 715.334 s | unchanged green exit | GO |

The cold full comparison is deliberately conservative because the accepted baseline
was measured after no-run warming, whereas the after sample included a cold build. It
saved 589.784 seconds and exceeded the required 40% improvement. The likely
theoretical saving was large because the removed block represented roughly 61% of the
accepted full-suite wall time, but no after-value was claimed until this matrix ran.

Correctness and ownership evidence:

- evaluator tests: 1 passed;
- command-planning/runner tests: 3 passed, including a fake required workspace-check
  failure that kept `rust-builder` blocking and closed E2 without spawning Cargo;
- role-eval integration tests: 13 passed;
- focused strict Clippy and formatting: green;
- cold and warm ordinary workspace suites: green;
- explicit E2 output: all seven roles `PASS`, `verdict: E2 GATE OPEN`, exit 0;
- `role-evals-nested` directories beneath all three measurement targets: zero;
- active pipeline recursion markers and `#[ignore]` on role evals: zero;
- CI workspace command: one ordinary `cargo test --workspace`; the ignored-only nextest
  job cannot select these tests because none are ignored.

Decision: the direct behavior/performance fix is GO for integration under the founder's
instruction to tackle the main issue and merge. The cold no-run ceiling remains a
recorded 3.500-second deviation, not a hidden pass; no core-model or crate-splitting
follow-up is included in this PR.
