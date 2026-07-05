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
- [x] queue  - [x] side-by-side  - [x] actions  - [x] audit log

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
- 2026-07-05 (leg B, web-builder): reviewer UI shipped under `app/(reviewer)/review/…`
  (route group; robots noindex; absent from all sitemaps). Queue `/review` renders the
  API's ranking VERBATIM (no client re-sort) with reason, target-record summary,
  confidence, extracted_by, age, cursor pagination + open/resolved filter. `/review/[id]`
  is the side-by-side: every extracted field (incl. raw description, band as decimal
  strings, dates, owner, details payload) beside the Bronze document — official PDF via
  `raw_document.source_url` in an iframe with fallback link + sha256 (serving our
  archived GCS copy is post-020-apply); pre-review note panel shows extracted_by,
  confidence, and extraction-cache evidence (model, cached_at, cross-check provenance)
  when present. Actions confirm/edit/reject go ONLY through POST /resolve via a server
  action; edit is a field-level form seeded with current values (typed via `Pick` from
  the generated `DisclosureRecord`; money stays strings); success shows outcome +
  affected record ids + links; 409 shows an honest conflict notice and reloads server
  state. Audit log panel lists every attempt (reviewer, verdict, outcome, note, affected
  ids, time). Tests: 22 vitest units under `src/components/reviewer/` + 4 playwright
  flows in `e2e/reviewer.spec.ts` (queue→task→confirm, edit-supersedes asserting the
  chain via promote, reject→disputed, 409 race) against the real seeded API.
  - E2E seeding: `e2e/reviewer-db.ts` clones a pipeline-produced unverified record
    (fresh id + fingerprint, same filing/politician/regime/details) and opens a task on
    it — promote's `transition` fails closed on non-unverified rows, so runs stay
    repeatable without exhausting fixtures (same footing as contract-test `seed_task`).
  - Env note: local dev DBs seeded before 041a need migration 0006 — one
    `cargo run -p worker --bin local` applies it (its scanned_paper fail-closed exit is
    the documented adapter behavior, not an error here).
  - pnpm 11 forwards the literal `--`, so the verbatim acceptance commands run the full
    (green) web suites as a superset; `vitest run reviewer` / `playwright test reviewer`
    select exactly the reviewer tests.
