# 091 — admin observability dashboard (full board, local-first, cloud-migratable)

## Objective
Give the founder one internal site with full observability of the operation: coverage
(what's covered / what's left worldwide), backfill progress, pipeline health, review/data
quality, storage, serving/product, money/infra guardrails, and the autonomous-loop meta —
built to run fully against local Postgres first and migrate to Cloud Run with zero code
changes (env flip only). Distinct from goal 090 (a narrower, already-shipped read-only
`/v1/admin/ops/*` API + `/healthz` + a claude.ai frontend-build prompt, unmerged on its own
branch): 091 is the founder-directed full A–H board with a first-party dashboard UI built
in-repo (Tailwind v4 + Recharts, `apps/web` `(admin)` route group), not an external prompt.

## Scope
In (v1):
- Migration `0011_backfill_run.sql` (expand-only; per-year backfill/seed run bookkeeping).
- Worker instrumentation: `backfill-real.rs`, `backfill-real-br.rs`,
  `seed-historical-rosters.rs`, `seed-br-candidates.rs` insert `backfill_run` rows.
- 9 admin-gated `GET /v1/admin/*` composite endpoints (overview, coverage, backfill,
  pipeline, quality, storage, serving, infra, loop) — one per dashboard page, read-only,
  behind the existing `auth::admin_gate` (`X-Admin-Token`). Contract regenerated.
- `apps/web` `(admin)` route group: 9 pages, Tailwind v4 scoped to admin only (no leak
  into the public site), Recharts charts, TanStack Query for the one live-polling status
  strip, everything else via `router.refresh()`.
- Env-gated honesty: GCP-only panels (infra/G) render explicit "unavailable in this
  environment" locally; `/admin/loop` 503s when `GOVFOLIO_REPO_ROOT` unset (cloud).
Out: mutation endpoints; new infra/services; IAP/auth hardening for cloud (follow-up);
CORS; the reviewer surface (`/v1/review-tasks`, untouched).

## Context
- Full design: plan session 2026-07-08 (this repo's Claude session), architecture and the
  A–H taxonomy enumerated per dashboard section; `docs/runbooks/admin-dashboard.md` (to be
  written in this goal) is the durable reference going forward.
- Backend (migration + worker instrumentation + all 9 route handlers) implemented and
  VERIFIED on this branch prior to this goal file landing: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`, `scripts/check-migration-safety.sh`,
  and `cargo test --workspace -- --ignored` (DB-backed suites) all green against local pg
  :5433; `packages/contracts/{openapi.json,src/api.d.ts}` regenerated and confirmed
  byte-identical (no drift). Remaining work under this goal: the `apps/web` frontend
  (P5–P8) plus final DoD sweep.
- Patterns to copy: `crates/api/src/routes/review.rs` (admin route precedent, already
  followed by the 9 new handlers); `apps/web/src/app/(reviewer)/` (route-group precedent);
  `apps/web/src/lib/api.ts` (typed client door, `adminHeaders()`).

## Acceptance
- [x] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace` — green (backend)
- [x] `docker compose up -d && cargo test --workspace -- --ignored` — green against local pg :5433
- [x] `cargo run -p api --bin openapi` produces no diff; `pnpm --filter @govfolio/contracts generate` produces no diff — confirmed byte-identical
- [x] `pnpm --filter web typecheck && pnpm --filter web lint && pnpm --filter web test` — 50/50 vitest green (also fixed pre-existing BRL drift gap in ResolvePanel.tsx, surfaced by this contract regen)
- [x] `pnpm --filter web build` — green against a live local API (sitemaps ISR-fetch at build time)
- [x] Public Playwright specs pass unchanged (Tailwind-scoping regression gate) — 17/17
- [x] New `apps/web/e2e/admin.spec.ts` passes: `/admin` tiles populate, `/admin/coverage` shows
  a `us_house` row, `/admin/loop` renders without crashing — 3/3 (20/20 full suite)
- [x] Visual QA via Playwright screenshots caught and fixed a real UX bug: the coverage page's
  regime table listed all ~196 jurisdictions (13,000px tall); filtered to active regimes only
  (2,600px), with a "+189 stub regimes" footnote pointing back to the coverage strip
- [x] `docs/runbooks/admin-dashboard.md` written (env vars, local boot sequence, cloud migration note)
- [x] Work committed on `feat/admin-observability`; this checklist ticked
