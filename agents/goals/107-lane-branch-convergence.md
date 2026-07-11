# 107 — lane-branch-convergence

## Status (superseded 2026-07-11)

Superseded by goal 109 and
`docs/plans/2026-07-11-autonomous-loop-hardening.md` §6. The earlier proposal used
`coverage_phase = live|blocked` plus a released lease as permission to merge a lane.
That ordering is unsafe: the shared phase must instead be the final atomic result of a
green integration receipt whose exact source SHA is already present on `origin/main`.

Do not implement the direct lane→main merge or push described below. It remains as the
historical proposal and rationale; goal 109 owns the replacement.

## Objective
Give lane-0 an explicit, repeatable step to merge completed factory-lane work back into
main. Today nothing does this: `factory-lane.md` merges main INTO lane branches and
explicitly forbids lanes from pushing to main themselves — so real jurisdiction work a
lane finishes has no path to ever reach main.

## Motivation (observed 2026-07-11, founder-directed)
Confirmed by direct inspection: every historical `lane/N` merge commit in this repo is
main flowing INTO the lane, never the reverse (`git log --all --merges --grep=lane`).
`agents/workflows/factory-lane.md` step 6 says "merge main INTO the lane branch" and
`agents/PROMPT-FACTORY-LANE.md` step 5 says "Never commit to main." Neither
`orchestration.md` nor `factory-lane.md` has a step that merges a lane branch into main.
Right now `lane/1`/`lane/2`/`lane/3` sit 7-10 commits ahead of main each, stranded from
the overnight run — low-stakes today (mostly no-op JOURNAL entries), but a real
adapter/backfill landing in a lane with this gap open would be invisible on main forever.

Compare: GOAL branches (100, 097, 104) reliably converge because the orchestrator
treats "goal fully done, tick 000-INDEX" as its own merge trigger — that pattern
doesn't exist for lane branches because they never "finish a goal file"; they advance a
jurisdiction's `coverage_phase` in Postgres instead.

## Scope
In:
- A new orchestration step (or an addition to existing step 6/RECORD) that identifies
  lane branches whose held jurisdiction reached `live` this session (or, for a periodic
  sweep, any lane branch ahead of main with no lease currently held on it) and merges
  each one into main (`--no-ff`, referencing the jurisdiction/phase in the merge
  message), then pushes. Conflicts are NOT auto-resolved — a real conflict halts that
  specific lane's merge, files a goal/notes it in JOURNAL, and does not block other
  lanes' convergence.
- Decide and document: does lane-0 do this as part of ITS OWN iteration (cheap, but
  lane-0 doesn't hold the lease so must merge by branch name + a live/blocked check
  against the registry, not by "reached live" observed in-session), or is it a
  dedicated periodic sweep step? Recommend the former (folds into existing step 6
  semantics) unless investigation shows a real reason not to.
- Safety: never merge a lane branch whose jurisdiction is still actively claimed
  (`claimed_by` non-null, session possibly mid-work) — only merge once
  `coverage_phase='live'` (or `blocked`, if there's real completed-but-blocked work
  worth preserving) AND the lease is released.
- Update `factory-lane.md`/`orchestration.md` docs to describe the new step precisely
  (no drift between doc and behavior — same discipline the existing files already use).

Out:
- Changing how lanes commit or what they commit (unaffected)
- Retroactively merging the CURRENT stranded `lane/1-3` branches as part of this goal's
  code changes — do that as a one-time manual cleanup (either by this goal's own
  acceptance run once the mechanism exists, or separately); don't hand-merge them
  before the mechanism is built and reviewed, or the goal can't prove the new step
  actually works against real stranded branches
- Codex's own lane convergence (goal 105's territory, not this goal's)

## Context (read first)
- agents/workflows/factory-lane.md (step 6 — merge main INTO lane, never the reverse)
- agents/PROMPT-FACTORY-LANE.md (step 5 — "Never commit to main")
- agents/workflows/orchestration.md (step 6/RECORD — where the new merge-back logic
  most naturally belongs, on the lane-0 side)
- crates/worker/src/lease.rs (`claimed_by`/`coverage_phase` — the signal for "safe to
  merge this lane branch now")
- Live evidence: `lane/1` (8 ahead), `lane/2` (10 ahead), `lane/3` (7 ahead) as of
  2026-07-11 — use these as the acceptance-test fixtures for "does the new step
  correctly identify and converge (or correctly skip) real stranded branches"

## Acceptance criteria (all must pass)
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored
sh -n agents/run-loop.sh
# Manual/scripted proof: run the new merge-back step against the CURRENT repo state
# (lane/1, lane/2, lane/3 as they exist today) and confirm each is either correctly
# merged (lease released, coverage_phase live/blocked) or correctly skipped (lease
# still held) — no branch merged while its lease is live.
```

## Checklist
- [x] Task 0: superseded by receipt-authoritative goal 109 before direct merge work began
- [~] Task 1: design note — where the step lives (orchestration.md step 6 vs new step),
      the exact registry-state check that gates a merge, conflict-halt behavior
- [ ] Task 2: implement the merge-back logic + doc updates
      (orchestration.md/factory-lane.md)
- [ ] Task 3: prove it against the real stranded lane/1-3 branches (converge or
      correctly skip each, per their actual lease/phase state at run time)
- [ ] Task 4: full acceptance block green; JOURNAL write-back; 000-INDEX ticked

## BLOCKED (human)
(empty)
