# Runbook — start the coverage factory: subagent-driven, source-parallel

Place at: `C:\projects\govfolio.io\docs\runbooks\parallel-factory.md`

Two workflows: the calibration run must have already turned the E2 gate green (see
run-factory.md). This runbook is the STANDING parallel factory — driven by
`GOVFOLIO_LANES=N ./agents/run-loop.sh` lanes since goal 097. Authority:
`docs/decisions/automation-policy.md` (no human gates), `agents/workflows/orchestration.md`
(selection + step 2d), `agents/workflows/source-exploration.md` (phase->role chain).

---

## Pre-checks the goal MUST pass before scaling past ONE worker
(Under full autonomy these fail SILENTLY if missing — double-work or multiplied spend, no error.)

1. ATOMIC LEASE — IMPLEMENTED (goal 097): `cargo run -p worker --bin jurisdiction-lease`
   (claim --next is one UPDATE with FOR UPDATE SKIP LOCKED; a SELECT-then-UPDATE races and
   two workers grab the same source). Race-proven by `crates/worker/tests/lease.rs`
   (--ignored suite). Never claim by hand-rolled SQL — use the bin.
2. WORKTREE PER WORKER. Each worker runs in its own git worktree/branch and PRs into protected
   main. N workers sharing one working tree collide on git. (skill: using-git-worktrees.)
3. GLOBAL GUARDRAILS. The billing HARD CAP and terraform DESTROY_BUDGET must read SHARED state,
   not per-process. N workers each under a per-action limit can COLLECTIVELY blow the monthly
   ceiling. Confirm caps are global counters before parallel + full-auto.

If any pre-check fails: run ONE worker until it is fixed. One worker is always safe.

---

## Legibility discipline (keeps "what's being done / done / left" answerable WHILE parallel)
Reason: leases show the in-flight SET; without per-phase heartbeat you go blind between a claim
and a completion, especially across workers. So every worker MUST:
- CLAIM VISIBLY: atomic lease before touching a jurisdiction (this is also the shared
  "who's doing what" board other workers read).
- JOURNAL PER PHASE, not just per jurisdiction: append date | jurisdiction | phase | outcome
  | blockers to agents/JOURNAL.md at every phase boundary.
- COMMIT PER PHASE BOUNDARY: a commit at each phase advance (promote-on-review), so "being done"
  is never staler than one phase.
- RELEASE ON DONE/BLOCK: clear the lease when advancing to live or setting blocked:<reason>.
Then any agent/human answers all three questions mechanically:
  DONE   -> git log + JOURNAL + promoted docs/regimes/<x>/ folders
  LEFT   -> registry: coverage_phase < live, ordered by priority_score (blocked:<reason> = stuck)
  DOING  -> registry: rows with a live lease (claimed_by set, claimed_at < 24h)

---

## The lane (run-loop.sh spawns one per worktree; they self-coordinate via the lease)

Standing driver: `GOVFOLIO_LANES=N ./agents/run-loop.sh` (lanes execute
`agents/workflows/factory-lane.md`). The prompt below is what each lane iteration
effectively does (kept for single-session/manual runs):

```
Run the coverage factory as a subagent-driven, source-parallel loop.

PRE-CHECK (once, before N>1): confirm atomic lease claim, worktree-per-worker, and GLOBAL
billing/destroy caps (see parallel-factory.md). If any is missing, run as a single worker only.

LOOP:
1. SELECT (orchestration step 2d): highest priority_score jurisdiction in the current epoch
   with coverage_phase < live and NO live lease.
2. CLAIM atomically: `cargo run -p worker --bin jurisdiction-lease -- claim --next --epoch <n>`
   (never hand-rolled SQL). Exit 1 (`none`) means nothing claimable — stop. Never work a
   row whose claimed_by is not your identity.
3. EXECUTE the jurisdiction's CURRENT phase with the mapped specialist (source-exploration.md:
   scout->surveyor->sampler->spec-writer/test-designer->builder). Phases within one jurisdiction
   are a dependency chain — strictly sequential. Intra-source fetches stay concurrency-1 (politeness).
   Fan out with SUBAGENTS only WITHIN a single phase's work where independent (e.g. builder
   sub-tasks, auditor evidence sweep) — never to skip the phase order.
4. REVIEW: independent auditor pass (goal 022 bounded loop) — a BOUNCE routes back to the
   producer with notes and re-reviews; MAX bounces -> blocked:review_failed:<phase>.
5. TEST/VALIDATE: the phase's validator / conformance must be GREEN (real command exit codes;
   never a model judging a model). Stage artifacts; promote into docs/regimes/ only on PASS.
6. LABEL: set extraction_tier per record; non-deterministic tiers -> unverified + sampling audit
   and SAF refinement_trigger recorded (goal 023).
7. ADVANCE coverage_phase; JOURNAL the phase line; COMMIT; RELEASE the lease.
8. Repeat. Drive ALL work through registry state transitions — NEVER write goal files
   (invariant 9). Honor the epoch gate before entering each new epoch.

STOP when every jurisdiction in the epoch is live or blocked:<reason>. A guardrail breach or
MAX-bounce halts THAT jurisdiction (blocked + lease released) and you continue other sources.
```

Parallelism axes (so expectations match reality):
- ACROSS SOURCES (breadth): run N lanes (`GOVFOLIO_LANES=N`, one worktree each); leases
  keep them on different jurisdictions. This is the throughput win — more sources/hour.
- WITHIN A TASK (depth): subagent fan-out inside one phase (step 3). Composes with the above.
- NOT PARALLELIZABLE: a single source's phase chain (dependency-ordered) and intra-source
  fetches (politeness). Parallelism scales breadth, never a single source's finish time.

## Monitor (answers your three questions live)

```
./agents/monitor.sh
```

Read-only `loop-board` snapshot every 15s (override with `GOVFOLIO_MONITOR_REFRESH`): DONE /
DOING / LEFT from the registry, live claude/codex procs, structured journal + commit digests,
dual-stack log tails, aggressive tripwires (stale lease, dead proc + lease, log quiet, active
but no commit, claimable-but-idle, journal/HEAD desync). One-shot: `cargo run -q -p worker
--bin loop-board`. A leased row aging with no journal/commit signal = stalled or crashed
worker (lease reclaim >24h still applies).

