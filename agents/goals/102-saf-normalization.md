# 102 — saf-normalization

## Objective
Normalize all 7 regimes to one canonical directory-form SAF —
`docs/regimes/<x>/AUTHORITY.md` (+ `evidence/`, optional `sources.yaml`) with `<x>` the
underscore regime code — resolving hyphen/underscore drift and the duplicate legacy
`us-house.md` (design §1 gap 3, §4.1 locations).

## Scope
In:
- Move the 5 flat SAFs to directory form via `git mv` (history-preserving, never
  delete+recreate): `canada_ciec.md`, `australia_register.md`, `eu_fr_de_annual.md`
  (ONE SAF covering its three regime rows — stays one file), `uk_commons_register.md`,
  `us_senate.md` → `<x>/AUTHORITY.md`. The first four already have an underscore `<x>/`
  dir holding `evidence/`; `us_senate` has none — create `us_senate/` for it.
- Resolve hyphen/underscore drift with `git mv`: `us-senate/evidence/` →
  `us_senate/evidence/`; `us-house/evidence/` + `us-house/reference/` → under
  `us_house/` (merge into the existing `us_house/evidence/`). Every evidence path
  referenced from SAF frontmatter/claims is updated in the same commit and re-validated.
- Reconcile the duplicate legacy `docs/regimes/us-house.md` (36.6K) vs canonical
  `docs/regimes/us_house/AUTHORITY.md` (22.7K): diff for unique content, port anything
  unique into the canonical SAF's Quirks log (dated `[YYYY-MM-DD-nn]`, append-only), then
  replace the legacy file with a one-line tombstone pointer OR delete it — implementer
  decides and journals the diff evidence either way.
- SUPERSEDE `E1.lock.json` (which moves to `us_house/reference/`): it pins
  `docs/regimes/us-house.md` and `us-house/evidence/*`, so bump `version`, add a note
  naming this goal, and re-pin the new paths — per the lock's OWN supersede policy
  (version bump + note). `cargo test -p pipeline role_evals` must pass against the
  superseded lock.
- Sweep ALL references to moved paths (adapters' `lib.rs` doc comments, goal files,
  `docs/plans/`, runbooks, `agents/`): update in the same commit. A `git grep` for
  `us-house`, `us-senate`, `regimes/canada_ciec.md`, etc. comes back clean (excepted: renamed
  evidence dirs, and append-only historical records — JOURNAL.md entries, done goals'
  historical text, 000-INDEX done rows, pinned fixture MANIFESTs; the criterion covers LIVE
  references only).
- Re-run validate-survey/validate-sources for every regime after the moves — the standing
  lesson: re-run the relevant validate-* after ANY change to a committed regime artifact.

Out: content rewrites of SAF bodies (append-only quirks entries excepted);
memory-contract frontmatter additions (goal 101 owns the contract — if 101 landed first,
moved files must still validate under it); any adapter code change; pg schema changes.

> Dependency: do after goal 101 (moved SAFs must satisfy the memory-contract validator on
> landing). This goal edits the lock-pinned `agents/goals/000-INDEX.md` (ticking its row),
> so its commit rides the §4.2 amendment path (`authority/*` branch,
> `agents/AUTHORITY.lock.json` updated in the same commit, message references this goal) —
> design §5.

> Risk (the sensitive step): superseding a pinned artifact is the one place this goal
> touches tamper-evident state. A scorer that finds a defect in a pinned artifact surfaces
> a FINDING and NEVER edits it — but this goal supersedes the E1 lock AS A WHOLE, which the
> lock's own policy explicitly permits (version bump + note of what changed and why). The
> lock policy's founder-gate clause is superseded by `docs/decisions/automation-policy.md`
> (the canonical autonomy policy per root `CLAUDE.md`), so the goal-102 agent supersedes the
> lock without halting on that clause — design §5, goal-102 entry.

## Context (read first)
- docs/plans/2026-07-10-memory-authority-substrate-design.md §4.1 (locations), §5 (goal-102)
- exploration facts: `us_house` + `br` already dir-form; 5 flat SAFs; hyphenated
  `us-house/`, `us-senate/`; legacy flat `us-house.md`
- docs/regimes/us-house/reference/E1.lock.json (supersede-policy header; pins moved paths)
- docs/decisions/automation-policy.md (halt semantics; supersedes the lock's founder gate)
- docs/regimes/_templates/AUTHORITY.template.md
- agents/workflows/source-exploration.md (validator contract)
- crates/pipeline/src/factory.rs (validate-survey/sources logic)
- agents/goals/101-memory-contract-and-index.md (predecessor — the contract moved files satisfy)

## Acceptance criteria (all must pass)
```bash
for r in australia_register br canada_ciec eu_fr_de_annual uk_commons_register us_house us_senate; \
  do cargo run -p pipeline --bin validate-survey -- "$r" || exit 1; done
cargo run -p pipeline --bin validate-memory && cargo run -p pipeline --bin memory-index \
  && git diff --exit-code docs/memory/INDEX.md
cargo test -p pipeline role_evals                # E1 lock correctly superseded, pins re-verified
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

## Checklist
- [ ] Inventory + diff legacy `docs/regimes/us-house.md` vs `us_house/AUTHORITY.md`; port unique content into the canonical Quirks log (dated `[YYYY-MM-DD-nn]`); journal the diff evidence
- [ ] `git mv` the 5 flat SAFs → `<x>/AUTHORITY.md` (create `us_senate/`; `eu_fr_de_annual` stays one file)
- [ ] `git mv` hyphenated dirs → underscore: `us-senate/evidence/` → `us_senate/`; `us-house/evidence/` + `us-house/reference/` → `us_house/` (merged into existing `us_house/evidence/`)
- [ ] Replace legacy `us-house.md` with a tombstone pointer or delete it (implementer decides; journaled)
- [ ] Update every evidence path referenced from SAF frontmatter/claims to the new locations
- [ ] Reference sweep: adapters' `lib.rs` doc comments, goal files, `docs/plans/`, runbooks, `agents/`; `git grep` for `us-house`/`us-senate`/`regimes/<x>.md` clean (excepted: renamed evidence dirs, and append-only historical records — JOURNAL.md entries, done goals' historical text, 000-INDEX done rows, pinned fixture MANIFESTs; the criterion covers LIVE references only)
- [ ] SUPERSEDE `E1.lock.json` (version bump + note naming goal 102) re-pinning new paths; `cargo test -p pipeline role_evals` green
- [ ] Re-run validators for all 7 regimes; full acceptance block green; quirks-log move entries; §4.2 amendment path; memory/SAF write-back + JOURNAL line; committed; checklist + 000-INDEX row ticked

## BLOCKED (human)
(empty)
