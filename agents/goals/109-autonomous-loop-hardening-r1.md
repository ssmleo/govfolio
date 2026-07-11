# 109 — autonomous-loop-hardening-r1

## Objective

Ship Release 1: immutable producer receipts plus a singleton clean-merge integrator so
shared registry state can never run ahead of the exact green source commit on
`origin/main`.

## Scope

In:

- Allocate the next free core migration at integration time (currently 0014).
- `jurisdiction.lease_generation`, `pending_integration_id`, immutable
  `integration_receipt`, CAS lifecycle projection, and append-only events.
- Generation-fenced claim/renew/release; pending integration blocks resume, targeted
  claim, stale reclaim, and next phase.
- Producers validate, commit locally, submit a receipt, then wait; they never push,
  merge, journal, or mutate phase.
- Repository-owned validation matrix; producer commands are evidence, never executed
  as instructions.
- Clean candidate from exact `origin/main`, exact-SHA merge commit, one canonical
  receipt JOURNAL line, checks, integration-branch push, GitHub PR, merge-commit
  auto-merge, required `rust`/`db`/`web`/`guardrails`, merged-SHA CI verification,
  source ancestry proof, and atomic apply.
- Moving-main rebuilds without force-push. Conflicts become `rework_required`, two
  bounded repairs, then defer while unrelated work continues.
- Startup reconciliation for crash-after-merge/before-apply.
- Supersede goal 107 and retire direct producer phase advancement.

## Acceptance

```bash
scripts/check-migration-safety.sh crates/core/migrations
cargo test -p core integration_receipt
cargo test -p worker lease
cargo test -p loop-supervisor integration
docker compose up -d && cargo test --workspace -- --ignored
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Tests cover the full adjacent phase matrix, built→live real-source proof, generation
CAS/stale rejection, pending exclusion, idempotent submit/apply/reconcile, repair bound,
moving main, conflict/push/CI failures, all four required checks, exact ancestry, exactly
one JOURNAL line, and no registry transition before apply.

## Checklist

- [x] Next-free expand-only migration and immutable receipt contract
- [x] Generation-fenced lease APIs and pending exclusion
- [x] Producer submit/repair and sole-authority apply transactions
- [x] Git/GitHub clean integrator with fake-backed failure tests
- [x] Prompt/workflow/runbook producer behavior amended
- [x] Legacy lane evidence preserved and fenced; no pre-contract lane is allowed direct integration
- [x] Full acceptance and interim JOURNAL write-back; committed for protected-main integration

## BLOCKED (human)

(empty)
