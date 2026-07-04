# govfolio — agent context (root)

You are building govfolio.io: worldwide politician financial-disclosure tracking.
Free transparency layer + paid real-time alerts/API. Read these before anything else:

- Design (authoritative): `docs/plans/2026-07-04-govfolio-design.md`
- Plan (M0–M1 tasks + milestone map): `docs/plans/2026-07-04-govfolio-implementation.md`
- Loop protocol: `agents/LOOP.md` · Goal queue: `agents/goals/000-INDEX.md`

## Project map
- `apps/api` /v1 (Fastify, OpenAPI-first) · `apps/web` Next.js SSR + reviewer UI · `apps/worker` pipeline consumers
- `packages/core` domain types, Zod schemas (→ TS types AND JSON Schemas), SQL migrations, generated DB types
- `packages/adapters/<jurisdiction>` one adapter per regime: code + config + `fixtures/` + `docs/regimes/<x>.md`
- `packages/pipeline` stage runners, conformance harness, idempotency helpers
- `packages/contracts` openapi.yaml + generated clients · `infra/` terraform · `agents/` goals + context

## Invariants (never violate)
1. **Supersede, never update.** Facts in Gold are immutable; corrections/amendments insert superseding rows.
2. **Raw is sacred.** Bronze documents are immutable, sha256-addressed; `asset_description_raw` is always stored.
3. **Never guess entities.** Below-threshold instrument matches stay NULL + open a review_task.
4. **Idempotent writes only** into Silver/Gold (`ON CONFLICT DO NOTHING`, fingerprints).
5. **`details` is contract-typed.** Every (regime, recordType) payload validates against its Zod/JSON Schema at promotion.
6. **Fail closed.** Zero-row parses or drift freeze the adapter's publication and open a review_task.
7. **Money = decimal strings** end-to-end (`numeric(16,2)` ↔ string). No floats.
8. **Politeness:** conditional GETs, per-source min-interval, concurrency 1 default, identified user-agent.

## Conventions
- Strict TS, no `any` (CI-enforced). Sentence-case UI copy. ULIDs as ids.
- SQL-first migrations in `packages/core/migrations`; DB types are generated (kysely-codegen), never hand-edited.
- One filter grammar shared by `/v1/records`, the UI, and `alert_rule.filter`.
- TDD: failing test → minimal code → green → commit. Small commits, conventional messages.

## Definition of done (any task)
All acceptance commands in the goal file pass locally AND `pnpm -r lint && pnpm -r typecheck && pnpm -r test` is green AND work is committed on a branch with the goal checklist updated.

## Human-only lanes (stop and ask)
Applying migrations to prod · `terraform apply` · pricing/legal/methodology public copy · completing `expected.*.json` for new fixtures (human is ground truth) · mass reprocess diffs.
