# Cross-Agent Build Performance Policy Design

**Date:** 2026-07-12

## Goal

Reduce build-performance experiment churn across Codex, Claude, and autonomous
lanes by making one policy universally discoverable and by routing host-level
resource coordination through govfolio's existing supervisor.

This design does not create a second workflow coordinator. It adds performance
admission, evidence, retry, and time-budget rules around the coordinator that
already owns lanes, process lifecycle, integration, and recovery.

## Existing coordination

The repository already has host-wide workflow coordination in the supervisor
architecture:

- one fenced supervisor and durable control-store writer;
- lane ownership, generation fencing, heartbeats, and recovery state;
- atomic jurisdiction claims;
- one worktree and branch per lane;
- process cancellation and reaping;
- singleton receipt integration; and
- read-only status and monitoring surfaces.

These controls prevent duplicate logical work and conflicting integration.
They do not currently provide host-wide admission for expensive Cargo work.
Cargo's build lock is scoped to one target directory, while lanes normally use
private targets. Two lanes can therefore compile concurrently without sharing a
Cargo lock, saturating CPU, memory, and disk or colliding on Windows executables.

`jurisdiction-lease` is not a build-resource lease. `cargo-agent.ps1` configures
the compiler cache and environment but does not schedule host capacity.

## Architecture

The design has three layers.

### 1. One canonical policy

`docs/decisions/build-performance-policy.md` is the single source of truth for
performance experiment behavior, budgets, evidence, and outcomes.

The policy supplements repository invariants. It never overrides authority,
data durability, goal-queue integrity, or production safety rules.

### 2. Thin cross-agent discovery

- Root `AGENTS.md` remains the Codex-compatible entry point. It references the
  authority-pinned `CLAUDE.md` for shared repository instructions and requires
  the canonical policy for compile, link, test-build, cache, benchmark, and CI
  build-time work.
- `.claude/rules/build-performance.md` is Claude's automatic discovery shim. It
  points to the same canonical policy.

Neither shim copies the policy body. Individual `.claude/agents/*.md` files are
not edited, avoiding drift and preserving their existing user changes.

### 3. Supervisor-owned resource admission

The existing supervisor is the only authority allowed to schedule host build
capacity. A future executable extension adds resource admission to its durable
store; agents must not invent filesystem mutexes, database side channels, or a
parallel coordinator.

The resource classes are:

| Resource | Capacity | Applies to |
|---|---:|---|
| `cargo-exclusive` | 1 | Recorded benchmarks, cold builds, full workspace tests, workspace clippy, epoch gates, and complete matrix reruns |
| `cargo-focused` | 2 | Package-scoped check/build/test commands using private target directories |

`cargo-exclusive` conflicts with both resource classes. Two
`cargo-focused` holders may coexist. Capacity is host configuration owned by
the supervisor; the defaults above target the current 16-logical-CPU Windows
host and remain conservative.

Each admission record contains:

- resource class;
- lane or interactive-session identity;
- supervisor generation/fence;
- command category, not secret command contents;
- target directory;
- acquisition time;
- heartbeat time; and
- deadline.

The supervisor rejects acquisition when capacity or conflict rules would be
violated. A stale holder is recovered through the existing fenced recovery
path; agents never delete another holder's record.

Until executable resource admission lands, the policy labels exclusivity as an
unenforced precondition: an agent must inspect supervisor status and live
Cargo/Rust processes, then halt rather than claim an exclusive measurement if
the host is busy. Documentation must not claim this fallback is race-proof.

## Performance workflow

Every performance experiment follows this sequence:

1. Declare the workload, acceptance threshold, regression veto, target path,
   expected duration, and evidence format.
2. Obtain the applicable supervisor resource admission, or perform the honest
   preflight fallback while executable admission is unavailable.
3. Capture one exploratory before/after pair with audit-ready output.
4. Stop as `NO-GO` when the exploratory result has no useful signal.
5. Only a promising result expands to three samples and additional edit
   classes.
6. Release admission on completion, cancellation, timeout, or failure.

The required workload vocabulary is:

- cold build;
- warm no-op;
- representative edit rebuild; and
- `cargo test --workspace --no-run` or the explicitly scoped equivalent.

## Time, retry, and churn budgets

- Exploratory phase deadline: 30 minutes.
- Whole experiment deadline: 60 minutes unless explicitly extended before it
  expires.
- Progress heartbeat: at least every 10 minutes, containing completed samples,
  current command category, failures, and remaining work.
- No-progress escalation: halt after 15 minutes without a completed command or
  other measurable result.
- Transient retry budget: one retry per failed command.
- Matrix rerun budget: zero automatic complete reruns.
- A complete rerun requires a checkpoint stating why existing evidence is
  insufficient, which commands will repeat, and the estimated wall-clock cost.
- Review evidence requirements are fixed before execution. A reviewer can
  reject invalid evidence, but cannot silently expand the evidence contract and
  trigger hours of work.

Timeout or exhausted retry produces `INCONCLUSIVE`, not another hidden loop.

## Evidence contract

Audit-ready evidence is captured from the first recorded command and includes:

- source commit and dirty-state check;
- exact Cargo command;
- stable toolchain and target triple;
- lockfile hash and profile configuration;
- target directory and proof of cold-directory freshness when applicable;
- start time, end time, Cargo time, and host wall time;
- exit status and stderr;
- rebuilt workspace packages;
- concurrent Cargo/Rust process check;
- raw sample values and calculated median; and
- hashes of retained evidence artifacts.

Reports distinguish measured facts, calculations, estimates, and hypotheses.
Every lever ends as `GO`, `NO-GO`, or `INCONCLUSIVE`. Rejected and inconclusive
levers remain documented so agents do not repeat them without new evidence.

## Task sizing

An “easy” measurement task must not hide dozens of commands. One agent task may
contain at most:

- one cold command;
- three warm or edit samples; and
- their required reset commands.

Larger matrices are split into separately reviewed tasks. Review happens after
the exploratory pair, before confidence sampling, so evidence-format defects
are caught before expensive repetition.

## Authority and precedence

- `CLAUDE.md` remains unchanged because it is authority-pinned.
- `agents/goals/000-INDEX.md` remains unchanged because it is authority-pinned.
- A task-specific approved plan may impose stricter requirements.
- Performance policy cannot authorize destructive cleanup, shared-target
  mutation, Bronze movement, production access, or unplanned external spend.
- The supervisor remains the single host coordinator; no ad-hoc coordination
  mechanism may bypass its fencing or recovery model.

## Implementation scope

The cross-agent policy structure lands first:

- `docs/decisions/build-performance-policy.md`;
- root `AGENTS.md` pointer; and
- `.claude/rules/build-performance.md` pointer.

The supervisor resource-admission mechanism is a separate implementation plan
and PR because it changes durable runtime state and provider enforcement. Until
that PR lands, the canonical policy explicitly marks process inspection as an
advisory, race-prone fallback.

No benchmark harness is added in the policy PR. A harness follows only after
the evidence schema has been exercised once without requiring a rerun.

## Verification

The policy-structure PR is complete when:

- both agent families discover the same canonical policy;
- the canonical policy contains the exact budgets and evidence schema above;
- it accurately describes existing supervisor capabilities and the missing
  build-resource admission;
- it never claims the process-inspection fallback is enforceable;
- no policy text is duplicated across shims;
- no authority-pinned file changes;
- no existing `.claude/agents/*.md` user change is touched; and
- Markdown and repository authority validation pass.

The later supervisor PR is complete only when resource conflict, capacity,
heartbeat, timeout, stale recovery, fencing, cancellation, and cross-provider
admission tests pass on Windows-compatible paths.

## Rollout

1. Land the canonical policy and discovery shims.
2. Use the advisory preflight and time budgets immediately.
3. Implement supervisor-owned resource admission in a separate PR.
4. Change the policy status from advisory to enforced only after executable
   cross-provider tests pass.
5. Add a benchmark harness in a later PR if the evidence schema remains stable.

Policy amendments update the canonical document's change log. Discovery shims
change only when discovery mechanics change.
