# 019 — skill-imports gateway (founder-originated; supersedes quarantined 018's intent)

## Objective
Complete the governed import of external skills: locate+pin remaining sources, full
line-audit, merges, and flip every ARMED allocation to ACTIVE.

## Done in-session 2026-07-04 (see docs/decisions/skill-imports.md)
- superpowers @ d884ae04edeb: pinned, vendored to agents/skills/imported/, screened
  (13 clean; writing-skills 2 doc-link hits classified benign). ACTIVE allocations live.

## Remaining checklist
- [ ] Locate canonical impeccable repo; pin sha; vendor; audit; activate pack:impeccable
      (Phase A 2026-07-05: located pbakaus/impeccable — single canonical candidate, NOT
      ambiguous; pinned 582f23eae3c9; vendored plugin/ subtree; screened — code-bearing
      scripts flagged for Phase B. Audit + activate remain.)
- [ ] Locate+pin sources: rust-best-practices, rust-async-patterns, typescript-expert,
      typescript-advanced-types, typescript-react-reviewer, frontend-design
      (Phase A 2026-07-05: 4 of 6 pinned+vendored+screened — see
      docs/decisions/skill-imports.md §019 Phase A. typescript-react-reviewer
      SKIPPED(no-license), typescript-expert SKIPPED(ambiguous) — see Phase A findings.)

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
- [ ] Full line-audit of all vendored files (auditor) + founder sample sign-off
- [ ] Merge (post-audit): verification-before-completion -> chassis DoD;
      test-driven-development -> distill into bespoke rust-tdd (bespoke authoritative)
- [ ] Flip ARMED -> ACTIVE in role files + agents/PROMPT.md approvals log
- [ ] Record every verdict + license in docs/decisions/skill-imports.md

## Acceptance criteria
```bash
! grep -R "ARMED" agents/roles agents/PROMPT.md   # nothing left armed
test -f docs/decisions/skill-imports.md            # verdicts recorded
```

## BLOCKED (human)
- founder sample sign-off; impeccable canonical-repo choice if ambiguous
  (Phase A: impeccable NOT ambiguous — pbakaus/impeccable is sole candidate)
- typescript-expert canonical-source pick (or bespoke per A3) — candidates in Phase A findings
- typescript-react-reviewer: unlicensed upstream — accept bespoke authoring per A3?
