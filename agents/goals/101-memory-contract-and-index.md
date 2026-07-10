# 101 — memory-contract-and-index

## Objective
Generalize the proven SAF pattern into one validated memory-file contract across
regime/subsystem/ops domains, with a generated always-loaded index (design §4.1).

## Scope
In:
- `MemoryFile` contract (schemars, `crates/core`, next to `RegimeSurvey`; validated by
  the `validate-memory` bin). Frontmatter fields per design §4.1: `domain`
  (enum `regime|subsystem|ops`; `business` RESERVED — the schema knows it, the validator
  rejects it until business-execution loops land), `scope`, `description` (trigger-phrased
  one-liner — this line IS the retrieval index entry), `triggers[]`, `aliases[]`,
  `paths[]` (globs), `last_verified` (date), optional `regime_code` (required when
  `domain: regime`), `max_kb` (default 64, validator-enforced). Snapshot-committed schema
  (invariant 5 pattern).
- Body contract (lean three-section shape): `## Context`; `## Log (append-only, dated)`
  with `[YYYY-MM-DD-nn]` entry IDs + `superseded_by:` back-refs (invariant 1 applied to
  memory — entries never edited or deleted, corrections supersede); `## Open questions`
  (each with what was tried).
- Regime SAFs keep their richer shape: the memory-contract fields are ADDED to the
  existing `RegimeSurvey` frontmatter (the five SAF body sections are unchanged); the
  quirks log adopts the `[YYYY-MM-DD-nn]` entry-ID convention. Subsystem/ops files use
  the lean three-section shape.
- `validate-memory` bin (pipeline factory family, fail-closed, same pattern as
  `crates/pipeline/src/bin/validate_survey.rs`): frontmatter validates, mandatory sections
  present, size ≤ `max_kb`, entry IDs well-formed, unknown keys reject; index↔filesystem
  bijection (every memory file has exactly one INDEX row; every row's path exists and
  validates). Runs over the full `docs/memory/` tree + all regime SAFs.
- `memory-index` generator bin: writes `docs/memory/INDEX.md` from frontmatter — one line
  per memory file (`path · domain · description`), ALL domains including regimes; never
  hand-edited. CI drift gate: regen then `git diff --exit-code docs/memory/INDEX.md`
  (same pattern as the openapi contract gate).
- `docs/memory/` tree seeded from a new `docs/memory/_templates/MEMORY.template.md`:
  `subsystems/{api,web,pipeline,worker,infra,contracts}.md` +
  `ops/{prod-incidents,deploys,cost}.md`, each with a real initial `## Context` paragraph
  drawn from existing repo knowledge — no empty shells.
- `.github/workflows/ci.yml` guardrails job: `validate-memory` + `memory-index`
  drift-gate steps (Rust toolchain or prebuilt-bin cache — implementer's choice, §6).
- Amendments (design §6): `agents/PROMPT.md` step-1 load order inserts
  `docs/memory/INDEX.md` between `agents/goals/000-INDEX.md` and the JOURNAL tail;
  `agents/archetypes/_CHASSIS.md` + all `agents/roles/*.md`: every "SAF write-back"
  (completed-state, step-6 RECORD wording, required-context footers) →
  "memory write-back (SAF or domain memory file)", and the required-context footer gains
  "matching domain memory file(s) via docs/memory/INDEX.md"; `docs/regimes/_templates/
  AUTHORITY.template.md` gains the §4.1 memory-contract frontmatter fields (added to
  `RegimeSurvey`, not replacing it) + `[YYYY-MM-DD-nn]` quirks-log entry IDs.
- `agents/skills/memory-authoring/SKILL.md` generalizing `saf-authoring` (the SAF is the
  regime instance of the general contract; same checklist shape: load first → task →
  append dated entry → validate → same-PR write-back). `saf-authoring` either stays
  slimmed to point at it or is folded in — implementer decides and journals why.

Out: SAF file moves / hyphen-underscore normalization (goal 102); consolidation, journal
rotation, staleness report, untrusted-content lint (goal 103); authority lock + validator
(goal 100); any pg schema change; business-domain memory files (enum value reserved only).

> Dependency: do after goal 100 — its `validate-authority` validator must exist so this
> goal's `agents/` edits pass the pre-flight. This goal amends lock-pinned authority files
> (`PROMPT.md`, `_CHASSIS.md`, `roles/*.md`), so its commits ride the §4.2 amendment path
> (`authority/*` branch, `agents/AUTHORITY.lock.json` updated in the same commit,
> commit message references this goal).

## Context (read first)
- docs/plans/2026-07-10-memory-authority-substrate-design.md §4.1, §5, §6
- agents/goals/100-authority-lock-and-validator.md (predecessor — amendment path; no overlap)
- agents/PROMPT.md (load order) · agents/archetypes/_CHASSIS.md · agents/roles/*.md
- agents/skills/saf-authoring/SKILL.md (the skill to generalize)
- docs/regimes/_templates/AUTHORITY.template.md (SAF template gains the memory fields)
- crates/core (where `RegimeSurvey` lives — `MemoryFile` lands beside it)
- crates/pipeline/src/factory.rs + crates/pipeline/src/bin/validate_survey.rs (bin pattern)
- .github/workflows/ci.yml (drift-gate wiring)

## Acceptance criteria (all must pass)
```bash
cargo run -p pipeline --bin validate-memory      # all memory files (regimes + subsystems + ops) green
cargo run -p pipeline --bin memory-index && git diff --exit-code docs/memory/INDEX.md
cargo test -p pipeline memory_contract           # seeded: bad frontmatter, oversize (>max_kb), missing section, index drift
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

## Checklist
- [ ] `MemoryFile` schemars contract in `crates/core` (beside `RegimeSurvey`) + snapshot-committed schema
- [ ] `validate-memory` bin (factory family, fail-closed): frontmatter + sections + size + entry-ID + unknown-key checks
- [ ] Seeded red-path fixtures + `#[test]`s: bad frontmatter, oversize (>max_kb), missing section, index drift — proven red
- [ ] `memory-index` generator bin + `docs/memory/INDEX.md` + CI drift gate (`git diff --exit-code`); index↔filesystem bijection in `validate-memory`
- [ ] `docs/memory/_templates/MEMORY.template.md` + seeded `subsystems/{api,web,pipeline,worker,infra,contracts}.md` + `ops/{prod-incidents,deploys,cost}.md` with real Context paragraphs
- [ ] `docs/regimes/_templates/AUTHORITY.template.md` gains §4.1 memory-contract frontmatter fields + `[YYYY-MM-DD-nn]` quirks-log entry IDs
- [ ] `agents/PROMPT.md` load order inserts `docs/memory/INDEX.md` (§6)
- [ ] `_CHASSIS.md` + `roles/*.md`: "SAF write-back" → "memory write-back (SAF or domain memory file)" + required-context footer entry
- [ ] `agents/skills/memory-authoring/SKILL.md` (generalizes saf-authoring; journal the fold-in vs. point-at decision)
- [ ] Full acceptance block green; amendments ride the §4.2 amendment path; memory/SAF write-back + JOURNAL line; committed; checklist + 000-INDEX row ticked

## BLOCKED (human)
(empty)
