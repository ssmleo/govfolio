# Admin observability dashboard (goal 091)

Internal, founder-only dashboard: coverage (what's covered worldwide, what's left),
backfill progress, pipeline health, review/data quality, storage, serving, money/infra
guardrails, and the autonomous-loop meta. Nine pages under `/admin/*`, one composite
`GET /v1/admin/*` endpoint per page, admin-token gated. Runs fully against local Postgres;
migrates to Cloud Run with an env flip only.

## Visual system (goal 094)

The admin console was reskinned end-to-end as a dark instrument panel — a full swap
from the light theme it originally shipped with in goal 091, not an incremental
palette tweak. `(admin)/admin.css` defines the entire token set on `.admin-root`
(ground, type, state, series, component tokens); nothing from the public site's
`(site)/globals.css` light theme carries over.

- **Ground**: page background `--adm-bg: #0B0D12` (near-black), card surface
  `--adm-surface: #12151C`, sunken surface `--adm-surface-sunken: #171B24`.
- **Brand accent**: warm gold, `--adm-accent: #C2A15E` / `--adm-accent-deep: #E0C084`
  — used for links, active nav state, and section-letter chips. This **replaces** the
  old light-theme's green `--seal` brand color; gold is admin-only, `--seal` is
  untouched everywhere else in the app.
- **Saturated color is reserved for state**, never decoration: success (green),
  warning (amber), danger (red), info (blue) — see the state→variant map in
  `components/admin/ui/Badge.tsx`.
- **Cards get a visible shadow** (`--adm-card-shadow`, an inset highlight + a soft
  drop shadow) — a deliberate reversal of the old light-theme "flat, no shadow" card
  rule. On this dark ground, shadow is the depth cue; its presence is correct, not
  a regression.

### Shell

The old flat `AdminNav` top bar + single-row `StatusStrip` were replaced with a
three-part shell (`(admin)/layout.tsx`, now a true second Next.js root with its own
`<html><body>`):

- **`Masthead`** (`components/admin/Masthead.tsx`, `--adm-masthead-h: 58px`):
  wordmark + "Administrative Console" tag, an environment badge, an operator-role
  tag, and a live UTC clock.
- **`SentinelTicker`** (`--adm-ticker-h: 38px`), directly below the masthead: polls
  the overview snapshot every 15s and renders a derived one-word state
  (NOMINAL/WATCH/INCIDENT) plus frozen/running/failed/review-open/drift-open/DLQ
  counts.
- **`AdminSidebar`** (`--adm-sidebar-w: 230px`), replacing the old flat single-row
  nav: 9 links regrouped into 5 pipeline-phase groups — **Command** (Overview),
  **Acquisition** (Coverage, Backfill), **Refinery** (Pipeline, Quality),
  **Platform** (Storage, Serving), **Autonomy** (Infra, Loop) — each link carrying
  its section-letter chip (A–H, the same codes every page's card eyebrows use).
  Digits 1–9 jump straight to the matching screen (ignored while a form field has
  focus or a modifier key is held); the current path is written to
  `localStorage` (`govfolio-admin-last-screen`) on every navigation, read by
  nothing yet — a hook for a future "resume where you left off", not a redirect.

### Fonts

Three Google Fonts, self-hosted via `next/font/google` in `(admin)/fonts.ts` and
scoped **only** to the `(admin)` route group (applied as `.variable` classes on
`(admin)/layout.tsx`'s `<html>`; the public `(site)` group keeps its own
system-font stacks untouched):

| Role | Font | CSS var | Used via |
|---|---|---|---|
| Headings | Libre Baskerville (serif) | `--adm-font-display-family` | `.admin-root h1/h2/h3` |
| Body/UI | Public Sans (sans) | `--adm-font-body-family` | default body font + `.adm-eyebrow` |
| Numbers/dates/ids | IBM Plex Mono (monospace) | `--adm-font-data-family` | `.adm-num` (every count, timestamp, id, code snippet) |

Each `admin.css` font token (`--adm-font-display` / `-body` / `-data`) falls back to
the public site's own system-font stack if the `.variable` class is ever missing
from an ancestor — a defensive fallback, not the normal path; in the real app the
three fonts load as real `.woff2` network fetches, and `getComputedStyle` on
rendered elements reports `"Libre Baskerville", "Libre Baskerville Fallback"` /
`"Public Sans", ...` / `"IBM Plex Mono", ...` respectively (verified during Task 10
visual QA — see below).

### Regime Dossier

New feature on `/admin/coverage` (`RegimeDossier.tsx` + `CoverageRegimeExplorer.tsx`
+ `dossier-data.ts`): clicking a row in the regime-coverage table opens a 520px-wide
(`--adm-dossier-w`) slide-over from the right edge, showing that regime's
jurisdiction/phase, bridge code(s) into the adapter layer, politician/filing counts,
first/last-filed dates, tier composition (bronze/silver/gold), a gold-records-by-year
bar chart, integrity/freshness notes, a synthesized regime note, an honest
"politeness not observable from here" caveat, and a link to the adapter crate
(`crates/adapters/<x>`). Closes via its × button, a backdrop click, or Escape.

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
