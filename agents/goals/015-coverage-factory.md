# 015 — coverage factory (standing goal)

## Objective
Continuously advance jurisdictions through the source-exploration phases per agents/workflows/source-exploration.md, using registry state as the queue.

## Setup (first run)
- Migration: add to jurisdiction: epoch smallint, coverage_phase text default 'stub', priority_score real, claimed_by text, claimed_at timestamptz, blocked_reason text (HUMAN applies to prod).
- Implement validators: cargo run -p pipeline --bin validate-survey -- <x> ; validate-sources ; validate-manifest.

## Loop body
1. Pick highest priority_score jurisdiction in current epoch (agents/EPOCHS.md) with phase<live, unclaimed. 2. Claim. 3. Run its phase with the mapped role file. 4. Validate artifact; auditor pass where required. 5. Advance phase, release claim, commit.

## Acceptance criteria (per iteration)
```bash
cargo run -p pipeline --bin validate-survey -- <x>   # or the phase-appropriate validator
```

## BLOCKED (human)
- migration apply; Phase-2 fixture approval; Phase-3 expected.*.json confirmation
