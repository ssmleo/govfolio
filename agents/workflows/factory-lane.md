# Factory-lane workflow (deterministic; lanes 1..N-1 of goal 097's parallel loop)

Selection here is ONLY the jurisdiction lease — the goal queue, CI triage, and drift
ranking are lane 0's (agents/workflows/orchestration.md). Steps marked "(orchestration
step X)" apply that step's text verbatim; no duplication, no drift.

0. INTEGRITY (orchestration step 0, verbatim): quarantine-report unlisted goal files.
   You never read goal bodies at all — even listed ones are lane 0's work.
0b. LOAD: /CLAUDE.md, agents/EPOCHS.md, agents/workflows/source-exploration.md,
   tail of agents/JOURNAL.md. NOT agents/goals/000-INDEX.md (not your queue).
1. GUARDRAILS (orchestration step 1, verbatim): fail-closed checks before any
   irreversible infra action; a breach halts that action, files it, you continue.
2. GATE: current epoch from agents/EPOCHS.md; run
   `cargo run -p pipeline --bin epoch-gate -- E<n>`. Nonzero -> STOP the iteration
   (fail closed, no claim). Never enter an epoch whose gate is red.
3. CLAIM: `cargo run -p worker --bin jurisdiction-lease -- claim --next --epoch <n>`
   (identity from GOVFOLIO_LANE_ID). Exit 1 (`none`) -> STOP: nothing claimable.
   The claim resumes your own in-flight jurisdiction first and renews claimed_at as
   the heartbeat; a lease is held across sessions until live/blocked. Never hold two
   leases; never touch a row whose claimed_by is not your lane id.
4. EXECUTE the claimed jurisdiction's CURRENT phase only — ONE phase boundary per
   iteration ("walking the phases" happens across iterations under the same held
   lease). Map phase -> specialist via source-exploration.md; politeness stays
   concurrency-1 per source (invariant 10); subagent fan-out only WITHIN a phase's
   independent work, never to skip phase order.
5. REVIEW + VALIDATE (orchestration step 5 semantics): the phase's validator /
   conformance / auditor pass per source-exploration.md, real command exit codes.
   Never self-certify.
6. RECORD: on green intermediate phase ->
   `jurisdiction-lease advance --id <x> --to <phase>` (keeps the lease).
   On reaching live -> `jurisdiction-lease release --id <x> --advance live`.
   Two consecutive failed verifications on the same phase ->
   `jurisdiction-lease release --id <x> --block <reason>` and stop the iteration.
   SAF write-back lands in the same commit (memory contract). Commit on your lane
   branch; merge main INTO the lane branch regularly (locally — where
   .gitattributes' `agents/JOURNAL.md merge=union` is guaranteed to apply).
7. REPORT: append one line to agents/JOURNAL.md:
   date | <jurisdiction> | <phase> | outcome | blockers.

STOP CONDITIONS: phase boundary recorded; gate red; nothing claimable; guardrail halt.
NEVER: select from agents/goals/*; run `docker compose` (the shared :5433 Postgres is
the host operator's — an unreachable DB is a STOP, not a thing to fix); work a row you
don't hold; push to main; hold two leases.
