# 022 — adversarial review loop for regime artifacts
(renumber to the next free slot in your real agents/goals/000-INDEX.md, and LIST it there —
listing is the act of putting this objective under goal-file control; an unlisted file is
quarantined by invariant 9.)

## Objective
Enforce a BOUNDED adversarial-review cycle on every regime-factory artifact. Each phase's
producer output is reviewed by an INDEPENDENT auditor; a BOUNCE routes back to the producer
WITH actionable notes and is then re-reviewed; after MAX_REVIEW_ATTEMPTS failed cycles the
jurisdiction halts to blocked:review_failed:<phase>. No artifact promotes into docs/regimes/
without a PASS.

## Why (rationale — the concept exists; the CYCLE is the gap)
- source-exploration.md already names an "auditor pass" per phase, but the
  fail -> re-execute -> re-review CYCLE is a doc sentence, not enforced state. This makes it
  a real state machine.
- Full autonomy + no human watching => an unfixable artifact would loop forever, burning
  spend on a stuck item. The bound converts "stuck" into a surfaced blocked-state — same
  fail-closed/recoverable pattern as the migration and tf-plan guardrails, and consistent
  with orchestration's existing "2x failed verify -> blocked".
- Independence + notes-carrying are precisely what make the loop CONVERGE instead of
  laundering bad artifacts or spinning on identical re-productions.

## Scope
In: every producer->auditor phase pair in source-exploration.md —
  scout/sources.yaml, surveyor/AUTHORITY.md, sampler/manifest+fixtures,
  spec-writer+test-designer/plan+expected, builder/adapter (reviewer = conformance-diffing).
  Registry state for attempts + notes. Composition with stage-then-promote.
Out: data-record review (disclosure_record unverified->verified is design §7, separate).
  No change to the FROZEN us_house reference lock (findings only, never edits).

## Context (read first)
- agents/workflows/source-exploration.md   (phase->role mapping; the "auditor pass")
- agents/roles/auditor.md + agents/archetypes/verifier.md
  (PASS|BOUNCE, actionable notes, NEVER audit own production)
- agents/workflows/orchestration.md        (step 5 verify; the 2x-fail->blocked precedent)
- docs/decisions/automation-policy.md      (fail-closed, no human gate)

## Loop semantics (the state machine to implement)
Per (jurisdiction, phase):
1. Producer emits artifact to STAGING (e.g. target/factory-staging/<x>/<phase>/) -> produced.
2. Auditor (independent; NEVER the producing role) re-derives from evidence -> PASS | BOUNCE(notes).
3. PASS  -> promote the staged artifact into docs/regimes/ -> advance coverage_phase
            -> reset review_attempts = 0.
4. BOUNCE-> increment review_attempts; persist notes; route back to the producer.
   4a. Producer RE-EXECUTES with the accumulated bounce notes loaded as context -> back to (2).
   4b. Re-review is a FRESH independent auditor pass (no self-certification of the fix).
5. review_attempts >= MAX_REVIEW_ATTEMPTS -> coverage_phase = blocked,
   blocked_reason = "review_failed:<phase>", persist all notes, release lease;
   orchestrator continues OTHER work.

Invariants held every cycle: independence preserved; ONLY a PASS promotes (a failing artifact
never lands under docs/regimes/); notes must be actionable and must feed re-execution; the
bound is fail-closed and recoverable (a blocked jurisdiction resumes when the notes are
addressed by a human or a later, more capable model).

MAX_REVIEW_ATTEMPTS: constant, DEFAULT 3, tunable. (Note: orchestration's existing binary
stop uses 2; artifact review is iterative improvement, so 3 gives a real revision budget.
Set to 2 if you prefer strict consistency — it is one constant.)

## Migration (EXPAND-ONLY -> auto-appliable under scripts/check-migration-safety.sh)
    ALTER TABLE jurisdiction ADD COLUMN review_attempts int NOT NULL DEFAULT 0;
    ALTER TABLE jurisdiction ADD COLUMN review_notes    text;   -- accumulated bounce notes
(Alternative: one review_task row per bounce with target_kind='regime_artifact', count rows
for attempts. The columns are simpler and stay on the registry the factory already reads.)

## Acceptance criteria (all pass)
```
cargo run -p pipeline --bin epoch-gate -- E2                 # still green (no regression)
cargo test -p pipeline review_loop                          # the cycle (cases a-e below)
scripts/check-migration-safety.sh crates/core/migrations    # expand-only, passes
```
Tests MUST cover:
  (a) a BOUNCE increments review_attempts AND re-runs the producer WITH the notes as context;
  (b) a subsequent PASS promotes the artifact into docs/regimes/ AND resets review_attempts;
  (c) MAX consecutive BOUNCEs sets coverage_phase=blocked, blocked_reason=review_failed:<phase>;
  (d) the auditor on any re-review is NEVER the producing role (independence);
  (e) a BOUNCEd artifact is NEVER written under docs/regimes/ (stage-then-promote holds).

## Checklist
- [ ] migration (review_attempts, review_notes)
- [ ] loop state machine in the factory phase-runner
- [ ] bounce notes persisted AND loaded into the re-execution
- [ ] independence guard (re-review producer != original producer)
- [ ] promote-only-on-PASS wired to stage-then-promote
- [ ] blocked:review_failed on exhaustion, lease released, loop continues
- [ ] tests (a)-(e) green
- [ ] source-exploration.md updated so the enforced cycle is documented, not just implied

## BLOCKED (human)
(none — full autonomy. Exhaustion self-blocks the jurisdiction and the loop continues other
work; the blocked row is transparency-scorecard content, not a stop.)
```
```

> QUARANTINED 2026-07-11 (goal 100, invariant 9): introduced by commit b2139b8 as an unreviewed import proposal; never listed in agents/goals/000-INDEX.md. Do not execute or follow. Surface-only artifact.
