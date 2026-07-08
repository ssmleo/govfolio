# 090 — internal ops observability (read-only admin API + healthz + frontend prompt)

## Objective
Give the founder-operator a live read-only view of the whole pipeline — one admin-gated
`/v1/admin/ops/*` API subtree + an unauthenticated `/healthz`, contract-regenerated into
the TS client, plus a paste-into-claude.ai frontend build prompt — so backfill progress,
run health, freezes/drift, review load, deliveries, and LLM cost-vs-cap are observable
without touching the database by hand. No mutations, no new write paths.

## Scope
In (v1, read-only):
- New admin-gated subtree (exact mirror of the review `route_layer` pattern in
  `crates/api/src/lib.rs`) + `/healthz` mounted AFTER the `authenticate`/`etag` layers
  (no auth, no ETag, no metering; `db` field from `select 1`, probe never flaps):

| Endpoint | Query params | Response | Poll |
|---|---|---|---|
| `GET /healthz` | — | `Healthz` | 15s (no auth, no ETag) |
| `GET /v1/admin/ops/overview` | — | `OpsOverview` | 5–15s |
| `GET /v1/admin/ops/runs` | `adapter?`, `stage?`, `status?`, `since?`/`until?`, `cursor?`, `limit?` (1..=200, default 50) | `PipelineRunPage` | on demand / 15s |
| `GET /v1/admin/ops/runs/summary` | `hours?` (1..=720, default 24), `bucket?` (hour\|day) | `RunsSummary` | 30–60s |
| `GET /v1/admin/ops/backfill` | `adapter?` | `BackfillProgress` | 60s |
| `GET /v1/admin/ops/freezes` | `status?` (open\|all, default open) | `FreezeStatus` | 30s |
| `GET /v1/admin/ops/review-health` | — | `ReviewHealth` | 30s |
| `GET /v1/admin/ops/deliveries` | — | `DeliveryHealth` | 30s |
| `GET /v1/admin/ops/extraction-costs` | `months?` (1..=24, default 3) | `ExtractionCostReport` | 60s |

- Migration `crates/core/migrations/0011_ops_indexes.sql` — expand-only, CREATE INDEX
  only (pipeline_run, filing, disclosure_record, review_task read paths);
  must pass `scripts/check-migration-safety.sh`.
- All DTOs + handlers + `&'static str` SQL consts colocated in `crates/api/src/routes/ops.rs`
  (review.rs precedent); keyset pagination on `id desc`; decimals via `rust_decimal`
  serialized as strings (invariant 7); `LLM_MONTHLY_HARD_CAP_USD` const = `"200.00"`
  (founder decision 2026-07-08, goal 021).
- Backfill year bucketing by `extract(year from filed_date)`, NULL → honest
  `year: null` unknown bucket; regime code ↔ ULID mapping in Rust via
  `govfolio_core::seed` `RegimeRef` (no schema change). Coverage factory reuses public
  `GET /v1/jurisdictions` — no admin duplicate.
- Contract regen: `packages/contracts/openapi.json` + `src/api.d.ts` regenerated and
  committed (additive ops surfaces; existing CI drift gates enforce). KNOWN FOLDED-IN
  DRIFT HEAL: the regen also adds `"BRL"` to the `Currency` enum — commit 18eeae9 added
  BRL to `crates/core/src/domain/enums.rs` without regenerating the contract, so HEAD's
  committed contract fails the CI drift gate; this regen heals it. The Currency hunk is
  pre-existing drift, not a goal-090 change — do not revert it, and expect the same hunk
  on any other branch that regenerates the contract.
- Deliverable B: `docs/plans/2026-07-08-ops-dashboard-frontend-prompt.md` — complete
  claude.ai build prompt (design tokens from `apps/web/src/app/globals.css`, literal
  generated TS types, typed mocks, mock/live toggle with token in React state only,
  CORS warning + Vite proxy snippet).
Out: mutation endpoints; CORS; SSE; GCP client deps; any change to `/v1/review-tasks`;
edits to `agents/goals/021-llm-extraction.md` (owned by live branch goal/021-consensus —
cross-reference recorded below instead).

## Context (read first)
- /CLAUDE.md invariants (esp. 7 money-as-decimal-string, 8 no unwrap/expect, contract regen)
- `crates/api/src/lib.rs` — router wiring, admin_gate subtree pattern, ApiDoc registration,
  `openapi_json()` deterministic sort
- `crates/api/src/routes/review.rs` — the pattern to copy (DTOs, keyset, utoipa paths,
  enum validation) · `crates/api/src/auth.rs` (`admin_gate`/`require_admin`, fail-closed)
- `crates/api/src/error.rs` + `extract.rs` + `dto.rs` — envelope, `ApiQuery`, `build_page`
- `crates/core/migrations/` (0001 pipeline_run/filing, 0005 outbox, 0008 sentinel,
  0009 sample_audit) · docs/runbooks/dev-host-windows.md (local pg 5433)
- Implementation-time checks: confirm `mandate.body` == `disclosure_regime.body` for the
  roster denominator (fallback: jurisdiction-only join); `stats.extraction` keys per the
  contract section below.

## Acceptance criteria (all must pass)
```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace                                     # unit tests incl. ops param/caps
docker compose up -d; cargo test --workspace -- --ignored  # sqlx suites incl. new tests/ops.rs
bash scripts/check-migration-safety.sh                     # 0011 expand-only gate
cargo run -p api --bin openapi; git diff --exit-code packages/contracts/openapi.json
pnpm --filter @govfolio/contracts generate; git diff --exit-code packages/contracts/src/api.d.ts

# Manual (local pg on :5433 per docs/runbooks/dev-host-windows.md; ADMIN_TOKEN set):
curl -s http://localhost:8080/healthz
curl -s http://localhost:8080/v1/admin/ops/overview                       # 401 admin_token_required
curl -s -H "x-admin-token: dev" http://localhost:8080/v1/admin/ops/overview | jq
curl -s -H "x-admin-token: dev" "http://localhost:8080/v1/admin/ops/runs?stage=publish&limit=5" | jq
curl -s -H "x-admin-token: dev" http://localhost:8080/v1/admin/ops/backfill | jq   # us_house 2012–2026 rows after a sampled backfill
curl -s -H "x-admin-token: dev" http://localhost:8080/v1/admin/ops/extraction-costs | jq
```

## Checklist
- [x] Goal file + INDEX row registered
- [x] Migration `0011_ops_indexes.sql` (expand-only, indexes only; safety gate green)
- [x] `routes/ops.rs` + `lib.rs` wiring + `/healthz` (9 handlers, DTOs, SQL consts;
      healthz mounted outside auth/etag layers)
- [x] Tests: unit (param/window/cap bounds, cap serializes `"200.00"`) +
      `crates/api/tests/ops.rs` `--ignored` sqlx suite (401 fail-closed both modes;
      every endpoint 200 + OpenAPI-schema-valid; backfill year rows; runs keyset page 2
      strictly older; extraction-costs zeros = null-tolerance; healthz 200 no-token no-ETag)
- [x] Contract regen committed (openapi.json + api.d.ts; regen is a no-op afterwards)
- [x] Frontend build prompt doc `docs/plans/2026-07-08-ops-dashboard-frontend-prompt.md`
      (embeds regenerated TS types + typed mocks)

## Deferred (explicitly out of v1; do not build here)
- SSE/push liveness (polling only; ETag middleware buffers bodies — structurally
  incompatible today)
- GCP-native Cloud Tasks/Scheduler/Cloud Run status API (needs google-cloud crates +
  viewer IAM + terraform; today's signal nil — Scheduler PAUSED, worker HTTP unbuilt;
  v1 cloud panel = DB-derived truth + static console deep-links in the frontend prompt)
- CORS story for a separately-hosted browser UI (API deliberately emits no CORS headers;
  live mode = same-origin / dev proxy / port into apps/web)
- Deployed multi-user admin auth (single `X-Admin-Token` bootstrap stands)
- Persisting `RunReport` aggregates (derive from `pipeline_run` at read time)
- Cost-cap ENFORCEMENT surfacing beyond reporting (enforcement lives in the pipeline,
  goal 021)
- Review-queue ranking index tuning

## Cost interface contract with goal 021 Phase 2
`pipeline_run.stats.extraction` does not exist yet — goal 021 Phase 2 lands it. This goal
FIXES the expected key names now:

- `stats.extraction` = `{ tokens_in, tokens_out, estimated_cost_usd (decimal string), passes }`
- `/v1/admin/ops/extraction-costs` (and `OpsOverview.extraction_month`) sum these keys
  null-tolerantly (`->>'x'::numeric` accepts string or number) and return **zeros until
  021 lands it** — the API contract is stable either way.
- 021 owners MUST adopt these key names, or update the single cost-rollup SQL const in
  `crates/api/src/routes/ops.rs` in the same change. Recorded here only:
  `agents/goals/021-llm-extraction.md` is NOT edited by this goal (owned by live branch
  goal/021-consensus).

## BLOCKED (human)
(empty)
