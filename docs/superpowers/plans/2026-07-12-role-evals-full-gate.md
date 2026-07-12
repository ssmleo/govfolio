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
- [ ] Run all evaluator, acceptance, and role-eval tests plus fmt and strict Clippy.
- [ ] Confirm active code/docs contain no `role-evals-nested` or recursion environment.

## Task 4 — Clarify CI and runbooks

- [x] Amend `role-eval-thresholds.md` to supersede recorded prose with explicit command
  evidence while preserving governance semantics.
- [x] Keep one ordinary CI workspace suite and explain why CI does not call full gate.
- [x] Mark factory/runbook epoch-gate calls as intentional heavyweight certification.
- [x] Supersede the old role-gate section in the build-performance plan.

## Task 5 — Measure after implementation

Use separate fresh scratch targets for cold/warm no-run, cold/warm full tests, a
baseline-comparable full run after no-run, and the explicit gate. Record raw values in
this document and the append-only JOURNAL. Do not rerun the old recursive baseline.

GO requires:

- baseline-comparable full suite at most 696.722 seconds (at least 40% below 1161.204);
- cold no-run at most 137.912 seconds;
- zero nested Cargo workspace runs and no nested target directory;
- explicit E2 gate green with unchanged output/exit behavior;
- injected required-check failure remains closed; and
- focused, workspace, fmt, and strict-Clippy correctness green.

## Task 6 — Integrate

- [ ] Append measured results and the ownership correction to `agents/JOURNAL.md`.
- [ ] Verify the exact diff, commit all intended files, and merge the verified branch to
  `main`.
- [ ] Re-run focused tests plus the ordinary workspace suite on the merged commit using
  a private scratch target.

## Implementation results

No after-value is recorded until the post-implementation matrix runs. The accepted
theoretical saving is large because the removed block represented roughly 61% of the
observed full-suite wall time, but concurrency prevents predicting an exact result.

