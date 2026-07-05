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

## Progress
- 2026-07-05 (leg A, rust-builder): admin `/v1` surface landed — `GET /v1/review-tasks`
  (ranked queue: priority_score desc, created_at asc; ULID cursor keyset pagination;
  target-record summary), `GET /v1/review-tasks/{id}` (task + full 040a RecordDetail +
  extraction context incl. extraction_cache evidence), `POST /v1/review-tasks/{id}/resolve`
  (thin door onto `pipeline::promote::resolve_review_task`; confirm/reject/edit;
  409 on double-resolve), `GET /v1/review-tasks/{id}/audit`. Migration 0006
  `review_audit` (expand-only; one row per resolve attempt, `outcome` column added to
  the goal's list to distinguish applied/conflict/failed). Contract regen committed.
  Contract-tested: ranking order, pagination, confirm/reject/edit round-trips via the
  real promote path (original row byte-identical probe), 409, exact audit rows.
  - Decision: audit writes live in promote (applied in-txn; conflict/failed post-hoc) —
    promote stays the single write authority.
  - Documented gap (follow-up): silver staging payload not in the detail response —
    Gold rows store no staging linkage and stg tables are per-regime; needs expand-only
    `stg_table`/`stg_id` columns (or equivalent) before the UI can show Silver verbatim.
  - Known limitation: a correction failing the details contract surfaces as 500 (promote
    errors are untyped anyhow); typed promote errors would allow 422/400 — follow-up.
  - Auth: none (goal 050); `reviewer` is required free text, recorded in review_audit.
  - UI checkboxes above remain for leg B (web).
