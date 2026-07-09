# Admin observability dashboard (goal 091)

Internal, founder-only dashboard: coverage (what's covered worldwide, what's left),
backfill progress, pipeline health, review/data quality, storage, serving, money/infra
guardrails, and the autonomous-loop meta. Nine pages under `/admin/*`, one composite
`GET /v1/admin/*` endpoint per page, admin-token gated. Runs fully against local Postgres;
migrates to Cloud Run with an env flip only.

## Local boot sequence

```
scripts/dev/pg-local.ps1 start                                  # Postgres on :5433
DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio \
ADMIN_TOKEN=<any-dev-value> \
GOVFOLIO_REPO_ROOT=<absolute path to this checkout> \
  cargo run -p api                                               # API on :8080

GOVFOLIO_ADMIN_TOKEN=<same value as ADMIN_TOKEN> pnpm --filter web dev
```

Open `http://localhost:3000/admin`.

## Env vars

| Var | Local | Cloud |
|---|---|---|
| `DATABASE_URL` (api, worker) | `postgres://postgres:postgres@localhost:5433/govfolio` | Secret Manager (existing) |
| `ADMIN_TOKEN` (api) | any dev value | Secret Manager (existing) |
| `GOVFOLIO_REPO_ROOT` (api) | absolute path to the checkout | **unset** — `/admin/loop` answers 503 by design (repo isn't mounted in the deployed container) |
| `GOVFOLIO_API_URL` (web) | `http://localhost:8080` (default) | deployed API service URL |
| `GOVFOLIO_ADMIN_TOKEN` (web, server-only) | same value as `ADMIN_TOKEN` | Secret Manager (existing — already wired for the reviewer surface) |

## What's local-only vs cloud-ready

Sections A–F (coverage, backfill, pipeline, quality, storage, serving) need only
`DATABASE_URL` and work identically in both environments — the SQL is stock Postgres
(`pg_catalog`, `percentile_cont`, `jsonb`), no GCP dependency.

Section G (infra) is a static v1 mirror: it shows the HARD CAP figure and a paused-scheduler
list, but explicitly states live spend and Cloud Tasks depth are "unavailable in this
environment" — no GCP API calls are made, so this never breaks locally, and nothing is
fabricated when GCP credentials aren't present.

Section H (loop meta) is repo-root-gated: it parses `agents/goals/000-INDEX.md` and runs
`git log` against the checkout named by `GOVFOLIO_REPO_ROOT`. In a deployed Cloud Run
container the repo isn't mounted, so this env var stays unset and the page renders the
shared `Unavailable` panel — an expected state, not an error.

## Cloud migration (when the day comes)

No new service and no new infrastructure — the dashboard ships inside the existing
`govfolio-api` and `govfolio-web` Cloud Run services. Deploy as usual; set the same env
var names above via Secret Manager (all but `GOVFOLIO_REPO_ROOT`, which stays unset).
Before making `/admin` reachable in production, add an auth layer in front of it beyond
the shared `X-Admin-Token` (IAP or a middleware cookie check) — today's posture (anyone who
reaches the URL with the token can read admin data) mirrors the existing reviewer console
and is acceptable only because nothing is public yet.

## Verification

```
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
docker compose up -d && cargo test --workspace -- --ignored
cargo run -p api --bin openapi                      # must produce no diff
pnpm --filter @govfolio/contracts generate           # must produce no diff
pnpm --filter web typecheck && pnpm --filter web lint && pnpm --filter web test
pnpm --filter web build                              # needs a live API (sitemaps ISR-fetch at build time)
pnpm --filter web e2e                                # needs pg + a seeded, running API (see e2e/admin.spec.ts header comment)
```
