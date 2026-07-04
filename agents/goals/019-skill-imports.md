# 019 — skill-imports gateway (founder-originated; supersedes quarantined 018's intent)

## Objective
Complete the governed import of external skills: locate+pin remaining sources, full
line-audit, merges, and flip every ARMED allocation to ACTIVE.

## Done in-session 2026-07-04 (see docs/decisions/skill-imports.md)
- superpowers @ d884ae04edeb: pinned, vendored to agents/skills/imported/, screened
  (13 clean; writing-skills 2 doc-link hits classified benign). ACTIVE allocations live.

## Remaining checklist
- [ ] Locate canonical impeccable repo; pin sha; vendor; audit; activate pack:impeccable
- [ ] Locate+pin sources: rust-best-practices, rust-async-patterns, typescript-expert,
      typescript-advanced-types, typescript-react-reviewer, frontend-design
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
