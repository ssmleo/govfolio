# 041 — reviewer UI

## Objective
Priority-ranked review queue with side-by-side extracted-fields vs Bronze document, LLM pre-review note, approve/edit/reject wired to promote.ts (edits = superseding corrected records), full audit log.

## Context (read first)
- design §7.1–7.2 · crates/pipeline/src/promote.rs (UI calls Rust /v1 admin endpoints)

## Acceptance criteria
```bash
pnpm --filter web test -- reviewer && pnpm e2e -- reviewer
```

## Checklist
- [ ] queue  - [ ] side-by-side  - [ ] actions  - [ ] audit log
