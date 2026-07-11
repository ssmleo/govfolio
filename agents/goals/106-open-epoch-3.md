# 106 — open-epoch-3

## Objective
Open E3 (Europe, long tail beyond the E1 seeds) so factory lanes have real,
priority-ranked jurisdiction rows to claim: wire the E3 epoch-gate and survey/seed
`priority_score` for the European stub countries currently sitting at `epoch=NULL`.

## Motivation (observed 2026-07-11, founder-directed)
E2 is exhausted (`br` is the only E2 row, already `live`). Factory lanes correctly
report "nothing claimable" and idle. `epoch-gate`'s own usage string admits E3 isn't
wired (`only E2 is wired`), and the `jurisdiction` registry has zero rows with
`epoch=3` — every non-E1/E2 country sits at `coverage_phase='stub'`, `epoch=NULL`.
Two separate prerequisites are missing, not one: gate logic, and seeded rows.

## Scope
In:
- **Gate logic**: extend `crates/pipeline/src/bin/epoch_gate.rs` +
  `pipeline::evals::gate` to support `E3` — mirror goal 016's E1→E2 pattern (frozen
  reference bundle + role-eval scoring vs threshold for scout/surveyor/sampler, or
  whichever roles E3 entry actually requires per `agents/EPOCHS.md`). Exit criteria
  for E1→E2 was role evals green against the frozen E1 reference; E2→E3's equivalent
  bundle/threshold must be defined and frozen the same way (commit + lock file,
  matching the existing `evals::LOCK_PATH` convention) before the gate can open.
- **Registry seeding**: Surveyor-class work — for each European stub country (the
  ~30-40 not already E1-seeded: fr/de/gb/eu are done, so this is the *remaining*
  European tail), research and set `epoch=3` + `priority_score` per EPOCHS.md's
  formula (regime richness × feasibility × market interest − blocked penalties).
  Undocumented/opaque scores are not acceptable — each row's score must trace to a
  written rationale (SAF-style note under `docs/regimes/` or equivalent), consistent
  with invariant 3 (never guess) and the project's evidence-discipline norm for
  Surveyor output ("UNVERIFIED — evidence discipline applies", per EPOCHS.md's E2
  description, applies equally here).
- A jurisdiction that's genuinely unresearchable or has no viable disclosure regime
  gets `coverage_phase='blocked'` + `blocked_reason`, not a fabricated score — mirrors
  invariant 6 (fail closed) and the E2 scorecard convention ("blocked reasons
  documented on the scorecard").
- Migration if the registry needs new columns/constraints to support this (expand-only).

Out:
- Actually running factory lanes against the newly-opened E3 (that's the founder
  turning `GOVFOLIO_LANES` back on afterward, not this goal)
- E4 (Asia) / E5 (Oceania) — sequential per EPOCHS.md, not this goal's scope
- Building new adapters for any specific European country — that's factory-lane work
  once a row is claimable, not this goal

## Context (read first)
- agents/EPOCHS.md (epoch definitions, E2 exit criteria, priority_score formula)
- crates/pipeline/src/bin/epoch_gate.rs + crates/pipeline/src/evals.rs (E1→E2 gate
  pattern to mirror; `evals::LOCK_PATH`, `Outcome::Scored`/`NotApplicable`)
- agents/goals/016-role-evals-e1-calibration.md (the goal that built the E1→E2 gate —
  template for this goal's E2→E3 equivalent)
- `jurisdiction` table schema (epoch, coverage_phase, priority_score, blocked_reason
  columns already exist — confirmed via live registry inspection 2026-07-11)
- docs/regimes/ (SAF convention for per-jurisdiction research notes)
- agents/workflows/orchestration.md step 2d (how factory lanes select once rows exist)

## Acceptance criteria (all must pass)
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored
cargo run -p pipeline --bin epoch-gate -- E3   # must no longer print "only E2 is wired"
# Registry check: at least N European jurisdictions have epoch=3 with a non-null
# priority_score OR coverage_phase='blocked' with a written blocked_reason — zero
# rows silently left at epoch=NULL within the intended E3 country set.
```

## Checklist
- [ ] Task 1: define E3 entry criteria (roles + reference bundle) in EPOCHS.md/evals.rs,
      mirroring goal 016's frozen-bundle pattern
- [ ] Task 2: `epoch_gate.rs` + `evals::gate` wired for E3, tested (green + red cases)
- [ ] Task 3: survey the European stub tail; write per-jurisdiction rationale notes
- [ ] Task 4: seed `priority_score`/`epoch=3` (or `blocked`+reason) for every surveyed
      row via a migration/backfill bin, idempotent, dry-run reviewed before `--execute`
- [ ] Task 5: full acceptance block green; JOURNAL write-back; 000-INDEX ticked

## BLOCKED (human)
(empty)
