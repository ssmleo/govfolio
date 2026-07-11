# 108 — autonomous-loop-hardening-r0

## Objective

Ship Release 0 of `docs/plans/2026-07-11-autonomous-loop-hardening.md`: make retry
storms, duplicate supervisors, unfenced lane ownership, mixed/unbounded repository
logs, and paid deterministic-failure rediscovery mechanically impossible.

## Scope

In:

- A dedicated Rust `loop-supervisor` crate and pre-built `govfolio-loop` binary.
- One host-wide OS lock plus SQLite singleton/lane fences at
  `$HOME/.local/state/govfolio-loop/control.sqlite3` (WAL, FULL sync, busy timeout,
  integrity check, atomic backup).
- One-shot Claude and Codex adapters with separate stdout/stderr and structured-first
  terminal classification.
- Zero-spend authority, Git/worktree, compiler/linker, CLI, DB, Bronze, epoch,
  claimability, and disk preflights.
- Provider circuits, a classifier-independent storm fuse, half-open ownership, and
  the exact attempt budget in design §5.
- Atomic content-addressed logs outside the repository, compression, deduplication,
  redaction, rotation, retention, unresolved-evidence preservation, and disk pause.
- Recovery fencing when a failed/interrupted provider leaves repository changes.
- `agents/run-loop.sh` and `agents/monitor.sh` reduced to pre-built-binary shims.

Out: Postgres receipts/phase apply (109), live provider failover/canaries (110),
factory-lane provider scaling (111). Release 0 operates lane 0 only until 109 lands.

## Acceptance

```bash
cargo test -p loop-supervisor release0
cargo test -p loop-supervisor provider
cargo test -p loop-supervisor policy
cargo test -p loop-supervisor preflight
cargo test -p loop-supervisor process
sh -n agents/run-loop.sh && sh -n agents/monitor.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

The tests must prove: 10,000 identical quota outcomes create one provider spawn, one
attempt, one failure bucket/exemplar, and 9,999 bounded suppressions; compiler/Git/
empty/red/DB/Bronze/disk failures cause zero provider calls; corrupt structured output
is ambiguous; completed terminal wins over cleanup exit; operator stop never retries;
duplicate singleton/lane ownership and stale-fence writes fail; provider failures never
create a supervisor commit or JOURNAL line.

## Checklist

- [ ] Structured provider fixtures/classifiers and command builders
- [ ] SQLite control store, singleton/lane fencing, integrity/backup
- [ ] Circuit, storm-fuse, attempt-budget, and 10,000-outcome proof
- [ ] Atomic bounded log/artifact store and retention/disk policy
- [ ] Zero-spend preflights including a real linker canary
- [ ] Process-group ownership, cancellation, and crash recovery
- [ ] Supervisor tick/monitor/shims and failure postconditions
- [ ] Full acceptance, memory/JOURNAL write-back, committed and merged to main

## BLOCKED (human)

(empty)

