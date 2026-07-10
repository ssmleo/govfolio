# 103 — hygiene-loop

## Objective
Keep the memory substrate trustworthy over time — schema'd episodic entries, rotation,
a standing consolidation pass, staleness reporting, and poisoning/collapse defenses
(design §4.3).

## Scope
In:
- Journal entry schema (design §4.1 episodic, P8/P13): `date | role | goal | outcome |
  evidence pointers | blockers`, hard cap ~120 words/entry; documented in
  `agents/workflows/orchestration.md` step 7 (REPORT) and `agents/LOOP.md`. Long-form
  incident analysis moves into the relevant domain memory file's Log (with a
  `[YYYY-MM-DD-nn]` ID) or `docs/decisions/`; the journal line points at it. Existing
  over-length history stays untouched — append-only, never rewrite past entries.
- Rotation: after each consolidation pass the consolidated span moves to
  `agents/journal/archive/YYYY-MM.md`; `agents/JOURNAL.md` keeps only the unconsolidated
  recent tail; the loop's tail-read behavior is unchanged; archives stay grep-able,
  nothing deleted. Archives are raw episodic — EXCLUDED from `docs/memory/INDEX.md` (not
  INDEX-listed memory) by design (§4.1).
- Consolidation standing work item (NOT a one-shot goal): `orchestration.md` step 2
  SELECT WORK gains a "consolidation due?" check — due = 10 loop iterations since the
  last pass OR >20KB `agents/JOURNAL.md` growth. The pass (P4/§4.3): (1) distill
  unconsolidated journal entries into domain memory files' Logs, every distilled entry
  citing its source journal date(s) + commit(s); (2) promote recurring source quirks
  into the relevant SAF quirks log; (3) rotate the consolidated span to the archive;
  (4) regenerate `docs/memory/INDEX.md`. Delta-only discipline per §4.3.
- Delta-only rule extending goal-101's `validate-memory`: `validate-memory
  --check-delta <ref>` fails CI on any diff that DELETES or edits a dated
  `[YYYY-MM-DD-nn]` Log entry line; corrections are additive only (`superseded_by:`
  back-ref on the old entry + a new entry). Seeded red-path fixtures + `#[test]`s.
- `memory-staleness` bin (pipeline factory family, REPORT-ONLY — never a gate): lists
  load-bearing facts whose `last_verified` is >90 days across all memory files; wired as
  a CI warn step and printable by the loop. Report-only because staleness needs
  re-verification work, not a merge block (§4.3; Project Vend receipt, §8 #5).
- Untrusted-block lint in `validate-memory`: third-party quoted text (scraped
  disclosure/source content, remote-service errors) inside memory entries MUST sit in
  fenced ```untrusted blocks; the lint flags unfenced quoted scrape text where
  detectable (heuristic: URLs + quotation blocks in Log entries). Convention documented
  in `agents/skills/memory-authoring/SKILL.md`. (§4.3 anti-poisoning; §8 #1)
- First consolidation pass executed as proof: dry-run then a real pass over the current
  ~91KB `agents/JOURNAL.md`, producing valid delta-only diffs + the first
  `agents/journal/archive/YYYY-MM.md`.

Out: authority lock + validator (goal 100); the base memory contract + `validate-memory`
/`memory-index` bins + `docs/memory/` tree (goal 101 — this goal EXTENDS `validate-memory`,
it does NOT create it); SAF file moves / hyphen-underscore normalization (goal 102); any
alerting/paging; pg schema changes; business-domain memory files.

> Dependency: do after goal 101 (extends its `validate-memory` bin and the
> `memory-authoring` skill — both must exist). Independent of goal 102 — may run before
> or after it. This goal amends the lock-pinned authority files `agents/LOOP.md` and
> `agents/workflows/orchestration.md`, so its commits ride the §4.2 amendment path
> (`authority/*` branch, `agents/AUTHORITY.lock.json` updated in the same commit, commit
> message references this goal).

## Context (read first)
- docs/plans/2026-07-10-memory-authority-substrate-design.md §4.3, §4.1 (episodic),
  §5 (goal-103), §8
- agents/JOURNAL.md (the artifact this goal reshapes + the first consolidation's input)
- agents/workflows/orchestration.md (step 2 SELECT WORK, step 7 REPORT) · agents/LOOP.md
- agents/goals/101-memory-contract-and-index.md (predecessor — the `validate-memory` bin
  and `memory-authoring` skill this goal extends; no overlap)
- agents/skills/memory-authoring/SKILL.md (from goal 101 — untrusted-block convention lands here)
- crates/pipeline/src/factory.rs (validator factory family — `memory-staleness` joins it)

## Acceptance criteria (all must pass)
```bash
cargo run -p pipeline --bin memory-staleness     # report-only; exits 0 and prints the report
cargo run -p pipeline --bin validate-memory -- --check-delta origin/main   # this goal's consolidation
                                                 # pass is delta-only: no dated entry deleted/edited
cargo run -p pipeline --bin memory-index && git diff --exit-code docs/memory/INDEX.md
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

## Checklist
- [ ] Journal entry schema (`date | role | goal | outcome | evidence pointers | blockers`, ≤120 words) documented in `orchestration.md` step 7 + `agents/LOOP.md`; existing over-length history left untouched (append-only)
- [ ] Rotation mechanics: consolidated span → `agents/journal/archive/YYYY-MM.md`; JOURNAL keeps the recent tail; tail-read unchanged; archive dir excluded from `docs/memory/INDEX.md`
- [ ] Consolidation standing work item wired into `orchestration.md` SELECT WORK ("consolidation due?" = 10 iterations OR >20KB growth); pass distills → domain Logs (cite date+commit) → promote quirks → rotate → regen INDEX
- [ ] Delta-only rule in `validate-memory` (`--check-delta <ref>`): a diff deleting/editing a dated `[YYYY-MM-DD-nn]` entry fails; supersedes additive — seeded red-path fixtures + `#[test]`s proven red
- [ ] `memory-staleness` bin (factory family, REPORT-ONLY): >90-day `last_verified` facts across all memory files; CI warn step; printable by the loop
- [ ] Untrusted-block lint in `validate-memory` (fenced ```untrusted; URL + quotation heuristic) + convention documented in the `memory-authoring` skill
- [ ] First consolidation pass on the current ~91KB JOURNAL (dry-run then real): valid delta-only diffs + first `agents/journal/archive/YYYY-MM.md`. NEVER a monolithic rewrite of any memory file (design §8 #2, ACE receipt: 18,282→122 tokens, accuracy below no-context baseline) — additions and supersedes only
- [ ] Full acceptance block green; §4.2 amendment path; memory/SAF write-back + JOURNAL line; committed; checklist + 000-INDEX row ticked

## BLOCKED (human)
(empty)
