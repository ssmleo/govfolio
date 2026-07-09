# Runbook — resume all fronts: US backfill + full fetch + more countries (subagent-driven, parallel)

Place at: `C:\projects\govfolio.io\docs\runbooks\resume-all-fronts.md`

Composes two already-open threads into one parallel-dispatch session:
- Goal 081 Task 5 (US House real backfill execution — the only open task in an otherwise-closed goal)
- The standing coverage factory (goal 015 / `parallel-factory.md`) — Brazil's remaining ~28
  historical election cycles (E2, in progress) plus advancing new jurisdictions.

Authority: `docs/decisions/automation-policy.md` (no human gates), `agents/workflows/orchestration.md`
(selection + step 2d), `agents/workflows/source-exploration.md` (phase→role chain),
`docs/runbooks/parallel-factory.md` (the pre-checks and monitor below are inherited from it, not
duplicated — see that file for the reasoning).

---

## Task zero — verify before dispatching (stale-reference guard)

`docs/runbooks/FACTORY-GOAL.md` Stage 1 step 6 cites "goal 024" as the backfill-machinery
prerequisite. **No such goal exists in `agents/goals/000-INDEX.md`.** The real, already-built,
already-proven-twice mechanism is goal 081 Task 2: `FilingSpec.backfill: bool`
(`crates/pipeline/src/stages/publish.rs`) → `insert_outbox` binds `dispatched_at = now()` when
true, suppressing alerts with zero schema/design changes. It is source-agnostic (used identically
by `us_house` and `br`). Treat 081 Task 2 as the backfill machinery; do not block on or search for
goal 024.

Before scaling past one worker, also re-confirm live state (it moves fast in this repo):
```
cargo run -p worker --bin check-br-identity-collisions          # must read PASS before any new br write pass
cargo run -p pipeline --bin epoch-gate -- E2                    # confirm E2 gate status
git log --oneline -15                                           # tail of agents/JOURNAL.md + 000-INDEX for anything closed since this runbook was written
```

---

## Pre-checks (inherited from `parallel-factory.md` — do not run N>1 workers until all hold)
1. **Atomic lease** on jurisdiction claim (`UPDATE ... WHERE claimed_by IS NULL RETURNING`).
2. **Worktree per worker** — protected `main`, never a shared working tree (skill:
   `using-git-worktrees`).
3. **Global guardrails** — billing HARD CAP and `DESTROY_BUDGET` must read shared/global state,
   not per-process.

If any fails: run ONE worker total until fixed.

---

## Front A — US backfill: finish goal 081 Task 5 (single worker, own worktree)

Strictly sequential — do not parallelize within this front; it may run concurrently with Fronts
B/C in its own worktree.

```
/goal Resume agents/goals/081-us-backfill-execution.md at Task 5 (the only unchecked task; goal
      080/Tasks 1-4.x are already done). Read the goal file's full Context + Research findings
      sections first — do not re-derive them.

      5a LOCAL REHEARSAL (zero cloud cost): run backfill-real for us_house, full range 2012-2026,
         against local dev Postgres (pg-local.ps1, localhost:5433/govfolio). This is the first
         full-scale end-to-end run, not a slice. Verify: Gold row counts roughly track goal 080's
         dry-run baseline (minus BACKFILL_BUDGET skips and the two documented 2026 edge cases),
         pipeline_run shows the full range claimed+finished, outbox dispatched_at is set
         throughout (zero real alerts).
      5b MINIMAL PROD CONNECTIVITY (only after 5a is clean): add ONE additive google_sql_user
         (CLOUD_IAM_USER) via terraform for the operating identity's own ADC, through
         check-tf-plan.sh (1 add / 0 destroy, well within DESTROY_BUDGET). Run Cloud SQL Auth
         Proxy locally against the recorded sql_connection_name; confirm connectivity before
         proceeding. Write the resulting connection string into the database-url secret.
      5c REAL PRODUCTION RUN: backfill-real for us_house, full 2012-2026 range, against the
         now-connected production Cloud SQL, budget-gated exactly as Task 4 built it (chunk by
         year, dry-run record_delta vs BACKFILL_BUDGET before each year's real write). Acceptance:
         prod filing/disclosure_record counts reflect the real backfill (spot-check against goal
         080's per-year dry-run counts less any skipped years), zero real subscriber alerts, a
         second invocation is a no-op.

      Any year that exceeds BACKFILL_BUDGET: skip it, log it in the goal file's own checklist,
      continue — never blocks. A guardrail breach on 5b/5c halts THAT step only; log under the
      goal's own BLOCKED(human) section (currently empty — do not add a human gate here per
      automation-policy; log guardrail breaches as findings instead) and hand off, do not retry
      past the guardrail.

      DONE when Task 5c's acceptance holds for real. Tick Task 5 + goal 081 in
      agents/goals/000-INDEX.md; commit on a branch per goal 081's own checklist convention.
```

---

## Front B — Brazil: finish the historical backfill (own worktree; can itself fan out sub-agents)

Per `agents/JOURNAL.md` 2026-07-07/2026-07-08 entries: Câmara (DEPUTADO FEDERAL) and Senado
(SENADOR) are both live and real-write-proven for 2018 and 2022 only, using the exact same
already-committed `backfill-real-br` bin, zero code changes needed to extend to another year pair.
~28 more election cycles remain (Brazilian general elections are biennial back to 1994). The
`check-br-identity-collisions` sweep must read PASS before starting (task zero, above) — the
JULIO CESAR DOS SANTOS collision is fixed, but re-verify nothing new appeared.

```
/goal Extend br's real historical backfill (Câmara + Senado, both bodies, same proven
      backfill-real-br path — no new adapter/parsing code expected) to the remaining
      pre-2018 and any post-2022 general-election years, one year-pair per invocation,
      mirroring the exact sequence already used for 2018/2022 (seed roster for the year →
      dry-run record_delta vs BACKFILL_BUDGET → real write → idempotency re-check via a
      second invocation → check-br-identity-collisions PASS → journal + commit).
      Fan out with SUBAGENTS across INDEPENDENT year-pairs only if each targets a disjoint
      DB write range and the shared roster-seed step for that pair completes first
      (roster seeding is a dependency of the real write, not parallelizable with it) —
      never run two real-write passes concurrently against the same year.
      Politeness stays concurrency-1 per source (invariant 10) regardless of how many
      year-pairs run in the same session.
      A same-pass identity collision is expected and handled (refused, logged) per the
      existing precedent — do not build new resolution logic; a NEW cross-body/cross-time
      collision (distinct from the already-fixed JULIO CESAR DOS SANTOS case) is a finding,
      not a fix-in-place — flag it in agents/JOURNAL.md and continue with other years.
      JOURNAL every year-pair (date | br | backfill:<years> | outcome | new-row-counts).
      Commit per year-pair, not batched.
```

---

## Front C — more countries: the standing coverage factory (N worktrees, breadth axis)

This is `parallel-factory.md`'s `/goal` verbatim — reproduced here only so all three fronts can be
launched from one runbook; treat that file as authoritative if the two ever diverge.

```
/goal Run the coverage factory as a subagent-driven, source-parallel loop.

PRE-CHECK (once, before N>1): confirm atomic lease claim, worktree-per-worker, and GLOBAL
billing/destroy caps (see parallel-factory.md). If any is missing, run as a single worker only.

LOOP:
1. SELECT (orchestration step 2d): highest priority_score jurisdiction in the current epoch
   with coverage_phase < live and NO live lease.
2. CLAIM atomically (UPDATE ... WHERE claimed_by IS NULL RETURNING). If the claim returns no
   row, another worker took it — select the next. Never work an unclaimed-by-me row.
3. EXECUTE the jurisdiction's CURRENT phase with the mapped specialist (source-exploration.md:
   scout->surveyor->sampler->spec-writer/test-designer->builder). Phases within one jurisdiction
   are a dependency chain — strictly sequential. Intra-source fetches stay concurrency-1 (politeness).
   Fan out with SUBAGENTS only WITHIN a single phase's work where independent — never to skip
   the phase order.
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

---

## Parallelism map (what actually runs concurrently)

| Front | Worktrees | Parallel within? | Blast radius |
|---|---|---|---|
| A — US backfill (081 Task 5) | 1 | No — 5a→5b→5c strictly sequential | Prod DB writes (5c), 1 terraform add (5b) |
| B — Brazil historical backfill | 1 (can fan sub-agents across disjoint year-pairs) | Yes, across year-pairs only | Local dev DB writes only (no prod br path exists yet) |
| C — coverage factory (more countries) | N (breadth) | Yes, across jurisdictions | Local dev DB + docs/regimes/ writes; PR into main |

Run A and B in their own dedicated worktrees first (they're finishing already-scoped work); add
C's N workers once the Front A/B worktrees are live and the pre-checks hold. All three read the
SAME global guardrails (HARD CAP, DESTROY_BUDGET) — that's exactly why pre-check 3 exists.

## Monitor
```
watch -n 15 'echo DONE:; git log --oneline -10; echo; echo DOING:; \
  echo "(081 Task 5: which of 5a/5b/5c; br: which year-pair; registry: claimed_by/claimed_at < 24h)"; \
  echo; echo LEFT:; echo "(081: remaining sub-tasks; br: remaining year-pairs; registry: coverage_phase < live by priority_score)"'
```
Tripwire: a leased row or an in-progress year-pair aging >24h with no new commit/journal line =
stalled/crashed worker — reclaim it, don't just wait.
