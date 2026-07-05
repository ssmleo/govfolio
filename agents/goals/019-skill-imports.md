# 019 — skill-imports gateway (founder-originated; supersedes quarantined 018's intent)

## Objective
Complete the governed import of external skills: locate+pin remaining sources, full
line-audit, merges, and flip every ARMED allocation to ACTIVE.

## Done in-session 2026-07-04 (see docs/decisions/skill-imports.md)
- superpowers @ d884ae04edeb: pinned, vendored to agents/skills/imported/, screened
  (13 clean; writing-skills 2 doc-link hits classified benign). ACTIVE allocations live.

## Remaining checklist
- [x] Locate canonical impeccable repo; pin sha; vendor; audit; activate pack:impeccable
      (Phase A 2026-07-05: located pbakaus/impeccable — single canonical candidate, NOT
      ambiguous; pinned 582f23eae3c9; vendored plugin/ subtree; screened — code-bearing
      scripts flagged for Phase B. Phase C 2026-07-05: ACTIVATED DOCS-ONLY — scripts
      line-audit waived-not-performed, so scripts/*.mjs must never be executed; see
      docs/decisions/skill-imports.md §019 Phase B/C.)
- [x] Locate+pin sources: rust-best-practices, rust-async-patterns, typescript-expert,
      typescript-advanced-types, typescript-react-reviewer, frontend-design
      (Phase A 2026-07-05: 4 of 6 pinned+vendored+screened — see
      docs/decisions/skill-imports.md §019 Phase A. typescript-react-reviewer
      SKIPPED(no-license), typescript-expert SKIPPED(ambiguous) — both remain fail-closed
      and NOT activated in Phase C; PLANNED(bespoke) markers in role files. 4/6 is the
      completed shape of this item under fail-closed policy.)

## Phase A findings (scout, 2026-07-05)
Vendored to agents/skills/imported/, all AUDIT PENDING (quarantine; nothing activated):
- impeccable@582f23eae3c9 (pbakaus/impeccable, Apache-2.0) — plugin v3.9.1; scripts/*.mjs
  are executable detector/live-server code (86 exec(, 26 fetch(, 6 child_process):
  Phase B must line-audit or exclude scripts before activation.
- rust-best-practices@7df6a608dd71 (apollographql/skills, MIT) — matches untracked .agents copy.
- rust-async-patterns@5cc2549a50fc (wshobson/agents, MIT) — matches untracked .agents copy.
- typescript-advanced-types@5cc2549a50fc (wshobson/agents, MIT)
- frontend-design@9d2f1ae18723 (anthropics/skills, Apache-2.0)
SKIPPED (fail closed):
- typescript-react-reviewer: source is dotneet/claude-code-marketplace @ 07fa7eac95c2,
  review-tool/skills/typescript-react-reviewer — repo has NO license; cannot vendor.
  Candidates: wait for upstream license, or bespoke (A3).
- typescript-expert: AMBIGUOUS — candidates davila7/claude-code-templates vs
  martinholovsky/claude-skills-generator (sickn33/antigravity-awesome-skills excluded as
  aggregator); not present in wshobson/agents or apollographql/skills. Founder pick or bespoke.
- [x] Full line-audit of all vendored files (auditor) + founder sample sign-off
      — WAIVED by founder 2026-07-05 ("the Phase B line-audit is considered DONE; move
      on"). No line-audit was performed; activation rests on Phase A screens alone.
      Impeccable code paths excluded via DOCS-ONLY restriction. Recorded in
      docs/decisions/skill-imports.md §019 Phase B/C.
- [x] Merge (post-audit): verification-before-completion -> chassis DoD;
      test-driven-development -> distill into bespoke rust-tdd (bespoke authoritative)
      (done 2026-07-05 under the waiver; provenance cited in both files)
- [x] Flip ARMED -> ACTIVE in role files + agents/PROMPT.md approvals log
      (done 2026-07-05; non-activated items carry PLANNED(bespoke — import failed
      closed) markers instead)
- [x] Record every verdict + license in docs/decisions/skill-imports.md
      (§019 Phase B/C resolution, 2026-07-05)

## Acceptance criteria
```bash
! grep -R "ARMED" agents/roles agents/PROMPT.md   # nothing left armed
test -f docs/decisions/skill-imports.md            # verdicts recorded
```

## BLOCKED (human)
All founder gates on this goal resolved by the 2026-07-05 waiver decision ("the Phase B
line-audit is considered DONE; move on"), except one open note:
- ~~founder sample sign-off~~ — resolved by waiver 2026-07-05.
- ~~impeccable canonical-repo choice~~ — Phase A: NOT ambiguous, pbakaus/impeccable sole
  candidate; activated DOCS-ONLY.
- ~~typescript-react-reviewer unlicensed upstream~~ — fail closed stands (waiver does not
  cover a missing license); path is bespoke authoring per A3, marked PLANNED(bespoke) in
  auditor role. Not blocking.
- OPEN (non-blocking): typescript-expert canonical-source pick (or bespoke per A3) —
  candidates in Phase A findings. pack:ts-craft runs PARTIAL until resolved.
