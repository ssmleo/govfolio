# 095 — ticker-gold-estimate-and-admin-rate-exempt

## Objective
Land the two follow-ups deferred from goal 094's final review: a live Gold-records
planner estimate on the sentinel ticker, and exemption of validated admin-token
requests from the anonymous rate-limit backstop (the confirmed e2e 429-flake source).

## Scope
In:
- `gold_records_estimate: Option<i64>` on `/v1/admin/overview` (`AdminOverview`),
  sourced from `pg_class.reltuples` (O(1) planner estimate; `-1`/never-analyzed → null,
  honest absence, not zero) + contract regen (openapi.json, api.d.ts)
- SentinelTicker renders the real value (`toLocaleString()`) or keeps the honest dash on null
- `authenticate`/`resolve` in `crates/api/src/auth.rs`: requests bearing a VALID
  `X-Admin-Token` skip `limit_anonymous` entirely (mirrors the Bearer-key bypass)
- Integration tests for both (contract.rs null→number transition across `analyze`;
  tiers.rs exempt / invalid-token-still-limited / anonymous-unchanged triple)

Out:
- Any `count(*)` over Gold in the overview path
- CDN/edge rate limiting, new `AuthContext` variants
- Changing `DEFAULT_UNAUTH_PER_MINUTE` or setting `UNAUTH_REQUESTS_PER_MINUTE` for e2e
- ops.rs `/v1/admin/ops/overview` (separate DTO, already has exact `gold_records`)

## Context (read first)
- crates/api/src/routes/admin/overview.rs (DTO + handler + read-only SQL test)
- crates/api/src/auth.rs (`authenticate`, `resolve`, `limit_anonymous`, `require_admin`)
- apps/web/src/components/admin/SentinelTicker.tsx (+ .test.tsx, src/test/fixtures.ts)
- crates/api/tests/tiers.rs (`tiers_anonymous_backstop_limits_per_ip` — template for the new test)
- agents/goals/094-admin-redesign-port.md (close-out notes; both items deferred there)

## Acceptance criteria (all must pass)
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored
cargo run -p api --bin openapi && pnpm --filter @govfolio/contracts generate && git diff --exit-code packages/contracts
pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test
# e2e stack per apps/web/playwright.config.ts header (pg + API with
# ADMIN_TOKEN=govfolio-e2e-admin-dummy), repeated to prove the 429 flake is gone:
pnpm e2e   # 2-3 back-to-back runs, zero rate_limited failures
```

## Checklist
- [x] Task 1: contract.rs red — `gold_records_estimate` presence + null→number across `analyze`
- [x] Task 2: overview.rs green — `GOLD_ESTIMATE_SQL` + field + read-only-test entry
- [x] Task 3: contract regen (openapi.json + api.d.ts), committed with the code
- [x] Task 4: SentinelTicker red→green — fixture, number + null→dash tests, component render
- [x] Task 5: tiers.rs red — admin-exempt / invalid-token-limited / anonymous-limited triple
- [x] Task 6: auth.rs green — valid-token exemption; playwright.config.ts header note
- [x] Task 7: full acceptance block green incl. repeated `pnpm e2e`; regen zero-diff
- [x] Extra (found during acceptance): /admin/loop e2e spec raced its streamed main
      region on the 503/Unavailable posture — deterministic fail with the API launched
      per its own header docs (no GOVFOLIO_REPO_ROOT); fixed with a web-first
      retrying assertion (0c95e10). Pre-existing spec bug, distinct from the 429 flake.
- [x] Extra (adversarial review sweep, dfa1a5a): reltuples oracle assertion in
      contract.rs (a -1→0 regression could previously pass), relkind in ('r','p')
      for the planned Gold partitioning, not-counted (not merely not-rejected)
      proof in tiers.rs, auth.rs module-doc drift, en-US locale pin in the ticker.
      Known accepted residual: the tiers 429 asserts share the sibling test's
      sub-0.05% UTC-minute-rollover flake window (established file pattern).
- [x] Close-out: checklist + 000-INDEX row ticked; merged back to main

## BLOCKED (human)
(empty)
