# 097 — parallel-lanes-xhigh (founder-directed 2026-07-10)

## Objective
Three founder directives from chat 2026-07-10 (that chat is the founder-gate record per
`agents/GOVERNANCE.md` §Effort policy): (1) every role runs xhigh effort; (2) the loop can
work multiple jurisdictions in parallel — N OS-level loop processes, each in its own git
worktree, coordinated via the existing Postgres jurisdiction lease; (3) `/goal` (referenced
in LOOP.md + 4 runbooks, defined nowhere) is dropped — `run-loop.sh` is the driver.

## Scope
In:
- `effort: xhigh` in all 11 `.claude/agents/*.md` shims (5 edits) + `agents/EFFORT.md`
  table rewrite + FOUNDER APPROVALS LOG entry in `agents/PROMPT.md`
- `jurisdiction-lease` bin in `crates/worker` (claim --next/--id, advance, release, status;
  single-statement claim with `FOR UPDATE SKIP LOCKED`; 24h stale reclaim) + `--ignored`
  race-proof test suite — closes `docs/runbooks/parallel-factory.md` pre-check 1
- `GOVFOLIO_BRONZE_ROOT` shared durable-Bronze parent env (`crates/pipeline/src/conformance.rs`
  + 4 worker-bin defaults) — closes the JOURNAL:91 cross-worktree Bronze-gap mechanism
- `agents/PROMPT-FACTORY-LANE.md` + `agents/workflows/factory-lane.md` (factory-only lanes:
  gate → claim → one phase boundary per iteration → validate → record → journal)
- `agents/run-loop.sh` `GOVFOLIO_LANES=N` (lane 0 = full orchestration unchanged; lanes
  1..N-1 = factory lanes in `../govfolio-lanes/lane-<n>` worktrees; epoch-gate zero-spend
  pre-flight; Ctrl-C reap) + `agents/monitor.sh` LANES section
- `.gitattributes` `agents/JOURNAL.md merge=union`; `.gitignore` lane logs
- /goal removal: `agents/LOOP.md:3`, `docs/runbooks/{run-factory,resume-all-fronts,parallel-factory,FACTORY-GOAL}.md`
  (prompt bodies kept verbatim; only invocation framing changes) + goal-021 budget
  amendment note (cumulative LLM gate must be DB-backed month-keyed under lanes)

Out:
- Consolidating historical scattered Bronze bytes (env fixes it going forward)
- Epoch/priority scoring for E3+ registry rows (E2 gate verified GREEN during this
  goal's acceptance — see checklist Task 5; the stale "blocked" state was goal-016-era)
- Building the DB-backed cumulative LLM spend gate (021 note only; console HARD CAP is
  the shared backstop today)
- Per-lane journal files (goal 103 owns journal rotation)
- FACTORY-GOAL.md's stale goal-024 reference (documented in resume-all-fronts.md Task zero)

## Context (read first)
- Plan of record: chat-approved plan 2026-07-10 (this file mirrors it)
- `crates/core/migrations/0003_registry_columns.sql` (lease columns — no new migration)
- `docs/runbooks/parallel-factory.md` (3 pre-checks; this goal implements pre-check 1)
- `agents/workflows/orchestration.md` + `agents/workflows/source-exploration.md`
- `crates/worker/src/bin/check-br-identity-collisions.rs` (bin pattern),
  `crates/worker/tests/backfill.rs` (sqlx test convention)
- `crates/pipeline/src/conformance.rs:42` (`workspace_root()` — Bronze per-worktree bug)

## Acceptance criteria (all must pass)
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored   # incl. lease race suite
sh -n agents/run-loop.sh && sh -n agents/monitor.sh
grep -rn -e "/goal" agents/LOOP.md docs/runbooks/ | grep -v "agents/goals/" | wc -l  # 0 (path refs to agents/goals/ are not command references)
grep -c "effort: xhigh" .claude/agents/*.md                    # 11/11
GOVFOLIO_LANES=2 ./agents/run-loop.sh                          # red-gate zero-spend smoke; Ctrl-C reaps
cargo run -p worker --bin jurisdiction-lease -- status         # against localhost:5433
```

## Checklist
- [x] Task 0: goal file + 000-INDEX registration (4cc90ac)
- [x] Task 1: all roles xhigh (5 shims + EFFORT.md + PROMPT.md approvals log) (9dcd6cf)
- [x] Task 2: jurisdiction-lease bin + race-proof --ignored suite, 9/9 green (ef4dc93)
- [x] Task 3: GOVFOLIO_BRONZE_ROOT shared Bronze parent (45d3d2b)
- [x] Task 4: PROMPT-FACTORY-LANE.md + workflows/factory-lane.md + pointers (73bee33)
- [x] Task 5: run-loop.sh lanes + monitor.sh + gitattributes/gitignore + smoke (d13ca8b);
      LEARNING folded in: epoch-gate is minutes-per-run BY DESIGN (goal 016 scores
      rust-builder via the real fmt/clippy/test/conformance gates), so the lane
      pre-flight runs until-green at startup (hourly re-check, zero claude spend),
      NOT per cycle; in-session workflow step 2 owns the per-iteration check.
      Smoke evidence: stubbed-claude run proved banner/worktree/spawn; stubbed-cargo
      run proved red-gate log+sleep loop and TERM-trap reap; real epoch-gate E2 run
      captured separately — SURPRISE: E2 GATE OPEN (exit 0, all 7 roles 1.00; the
      "blocked on scout/surveyor/sampler refs" state from goal 016 era is stale —
      calibration artifacts exist now), so real lanes will claim br immediately
      rather than idling.
- [x] Task 6: /goal dropped from LOOP.md + 4 runbooks (grep = 0 hits) + 021 budget note (a4823b6)
- [x] Task 7: full acceptance green; JOURNAL line; merge to main (2026-07-10) —
      fmt/clippy/test/`--ignored` all re-verified clean (workspace test hit a
      transient linker collision from another agent's concurrent build sharing
      this checkout's `target/`; re-ran under a private `CARGO_TARGET_DIR`,
      fully green, 28 passed `--ignored`); `/goal` grep 0, xhigh 11/11,
      `jurisdiction-lease -- status` runs clean against local Postgres (one
      real in-flight `br` lease found, left untouched). The
      `GOVFOLIO_LANES=2 ./agents/run-loop.sh` line was deliberately NOT re-run
      live at merge time — unlike the other checks it has a real side effect
      (immediately launches a live unattended `claude -p` orchestrator session
      with `--dangerously-skip-permissions`); Task 5's stubbed-`claude` smoke
      test already proves lane startup/spawn mechanics, and actually firing a
      real unattended loop is a founder call, not a merge-gate action. See
      `agents/JOURNAL.md` 2026-07-10 097/Task7 entry.

## BLOCKED (human)
(empty)
