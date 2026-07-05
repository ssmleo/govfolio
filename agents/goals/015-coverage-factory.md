# 015 — coverage factory (standing goal)

## Objective
Continuously advance jurisdictions through the source-exploration phases per agents/workflows/source-exploration.md, using registry state as the queue.

## Setup (first run)
- [x] Migration `crates/core/migrations/0003_registry_columns.sql` (2026-07-04): epoch
  smallint, coverage_phase text NOT NULL default 'stub' + CHECK on the §5.8 vocabulary
  (stub|scouted|surveyed|sampled|specced|built|live|blocked), priority_score real,
  claimed_by text, claimed_at timestamptz, blocked_reason text. Expand-only
  (check-migration-safety green); ~~(HUMAN applies to prod)~~ auto-apply per
  docs/decisions/automation-policy.md.
- [x] Validators implemented (2026-07-04): `cargo run -p pipeline --bin
  validate-sources|validate-survey|validate-manifest -- <x>` (crates/pipeline/src/factory.rs;
  35 unit tests, valid+invalid artifacts; us_house MANIFEST.json validates as the
  reference Sampler artifact). Fail closed; derived artifact schemas documented in
  agents/workflows/source-exploration.md §"Artifact schemas".

## Loop body
Selection and dispatch are the orchestrator's job (agents/workflows/orchestration.md, step 2d). Per item: claim. 3. Run its phase with the mapped role file. 4. Validate artifact; auditor pass where required. 5. Advance phase, release claim, commit.

## Acceptance criteria (per iteration)
```bash
cargo run -p pipeline --bin validate-survey -- <x>   # or the phase-appropriate validator
```

## BLOCKED (human)
- ~~migration apply; Phase-2 fixture approval; Phase-3 expected.*.json confirmation~~
  SUPERSEDED 2026-07-04 by docs/decisions/automation-policy.md (same pattern as goal 001):
  expand-only migrations auto-apply (check-migration-safety, pre-apply snapshot);
  fixture/expected.*.json judgments auto-resolve (second-model cross-check, publish
  unverified → sampling-audit queue). No human stops remain in this goal.
