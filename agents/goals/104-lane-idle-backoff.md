# 104 — lane-idle-backoff

## Objective
Stop factory lanes from spending full `claude -p` sessions when the registry has no
claimable work: a zero-spend claimable-work pre-check per lane iteration, mirroring the
existing epoch-gate pre-flight pattern.

## Motivation (observed 2026-07-11)
First live `GOVFOLIO_LANES=4` run: lanes 1–3 each ran repeated full-context no-op
iterations ("E2 exhausted (br live), E3 rows not seeded" — see lane logs + their JOURNAL
lines), respinning every `GOVFOLIO_LANE_SLEEP` (30s default) forever. Correct protocol,
pure token burn. Founder-directed fix 2026-07-10 (local evening).

## Scope
In:
- New **read-only** subcommand on `crates/worker/src/bin/jurisdiction-lease.rs`:
  `jurisdiction-lease claimable --epoch <n|En>` — exit 0 with machine-readable first
  stdout line `claimable epoch=<n> rows=<N>` when ≥1 row is claimable at that epoch;
  exit 1 with `none` when zero. No writes, no lease taken, no `--as` identity needed.
- The claimable predicate MUST be the same code path `claim_next` uses (factor a shared
  fn/SQL in `crates/worker/src/lease.rs`) — a probe that drifts from the claim predicate
  either starves lanes (probe says none, claim would succeed) or burns sessions (probe
  says yes, claim returns none).
- `agents/run-loop.sh` lane loop (the `while :; do … claude -p … sleep …; done` block):
  before each session spawn, run the probe from the lane worktree. Nonzero exit → append
  one log line (`lane-N: no claimable rows at epoch E<n> — sleeping <s>s (zero claude
  spend)` + probe output tail, mirroring the epoch-gate pre-flight's logging rationale:
  a compile/pg failure must read differently from a legitimately empty registry) and
  `sleep "${GOVFOLIO_LANE_SLEEP_IDLE:-3600}"` instead of spawning. Exit 0 → spawn as today.
- `run-loop.sh` header doc for the new env var; note in `agents/workflows/factory-lane.md`
  that the wrapper pre-screens for claimable work (in-session claim step unchanged).

Out:
- E3 seeding / epoch progression / exit certification (EPOCHS.md-driven, lane-0 work)
- Any change to `agents/PROMPT-FACTORY-LANE.md` or the in-session lease lifecycle
- Lane-0 orchestrator cadence
- Retrofitting the already-running loop: a live `run-loop.sh` never re-reads itself —
  close-out must note the founder restarts the loop to pick this up

## Context (read first)
- agents/run-loop.sh (lane fn: epoch-gate pre-flight + session loop — the two patterns
  this goal copies and extends)
- crates/worker/src/lease.rs (`claim_next` — source of the claimable predicate) +
  crates/worker/src/bin/jurisdiction-lease.rs (CLI surface, exit-code convention:
  0 done / 1 nothing / 2 usage; machine-readable first stdout line)
- agents/workflows/factory-lane.md (lane workflow the wrapper wraps)
- agents/goals/097-parallel-lanes-xhigh.md Task 5 (stubbed-claude smoke pattern — reuse
  for acceptance here)

## Acceptance criteria (all must pass)
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored   # incl. new claimable probe tests
sh -n agents/run-loop.sh
# Probe against local pg (docs/runbooks/dev-host-windows.md URL):
#   current all-live registry  -> exit 1, first line `none`
#   test-seeded claimable row  -> exit 0, first line `claimable epoch=… rows=…`
#   (both shapes covered by --ignored tests; manual run optional)
# Stubbed-claude smoke (mirror 097 Task 5): GOVFOLIO_LANES=2 + stub claude on PATH +
#   zero-claimable registry -> lane logs show the idle-sleep line and ZERO stub
#   sessions spawned during the observation window.
```

## Checklist
- [x] Task 1: shared claimable predicate in lease.rs + `claimable` subcommand + tests
      (--ignored, live pg: empty case, claimable case, epoch filter)
- [x] Task 2: run-loop.sh lane-loop pre-check + `GOVFOLIO_LANE_SLEEP_IDLE` (default 3600)
      + header doc + factory-lane.md note
- [x] Task 3: stubbed-claude smoke green (zero spawns on empty registry)
- [ ] Task 4: close-out — JOURNAL line, this checklist, 000-INDEX tick, note that the
      running loop needs a restart to pick up the new wrapper

## BLOCKED (human)
(empty)
