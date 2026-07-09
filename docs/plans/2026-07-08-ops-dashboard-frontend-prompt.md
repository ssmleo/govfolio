# govfolio ops dashboard — complete frontend build prompt

> **How to use this file (govfolio repo note, not part of the prompt):** paste this ENTIRE
> document into claude.ai as one message. It is self-contained: mission, constraints, design
> direction, per-view specs, the literal generated API types, complete typed mock data, the
> data layer, and the acceptance checklist. Everything below was generated against the real
> `/v1/admin/ops/*` API landed in goal 090 (`crates/api/src/routes/ops.rs`, contract regen
> committed in `packages/contracts/`). Nothing here is speculative — every type is a literal
> excerpt of the generated contract and every mock fact mirrors real pipeline state.

---

## 1. Mission

You are building the **govfolio operations dashboard**: the internal instrument panel for
govfolio.io, a worldwide politician financial-disclosure tracker. One founder-operator uses it
to watch a fully autonomous data pipeline: per-regime backfill progress, pipeline runs, drift
freezes, review-queue health, alert deliveries, and LLM extraction spend against a hard
USD 200.00/month cap. The dashboard is **read-only** — it observes, it never mutates. It is
**polling-live** (5–60 s per panel, paused when the tab is hidden). It will be built by you
now as a standalone React SPA running entirely from embedded mock data, then swapped to the
live API with a single flag, and eventually ported into the repo's Next.js app. Honesty is the
product's core value and the dashboard's too: it must never fabricate, smooth over, or hide a
number — empty states say empty, errors say what failed, unknown data is labeled unknown.

## 2. Build target & constraints

- **React SPA, TypeScript `strict: true`. `any` is banned** (repo invariant — do not use
  `any`, `as any`, or `@ts-ignore`; unknown JSON is `JsonValue`, defined in §5).
- **No Tailwind. No CSS framework.** Plain CSS with custom properties (the token block in §3
  is the single source of truth). One global stylesheet is fine.
- **No chart libraries.** All charts are hand-rolled **inline SVG** per the chart rules in §4.
  No d3, no Recharts, no canvas kit — the charts here are simple enough that a library would
  only add weight and genericness.
- **No external UI dependencies** beyond `react` and `react-dom`. System font stacks only (the
  tokens use system serif/sans/mono — no webfonts).
- Project shape: Vite + React + TS is the assumed dev harness (`vite.config.ts` proxy snippet
  in §7), but the component code must not depend on Vite specifics. Suggested files:
  `src/types.ts` (§5), `src/mocks.ts` (§6), `src/data.ts` (§7), `src/App.tsx` + one file per
  view, `src/styles.css`.
- **Polling, not streaming.** Each panel polls its own endpoint on its own cadence (given per
  view in §4, range 5–60 s). All polling **pauses when `document.hidden` is true** and resumes
  (with an immediate refresh) on `visibilitychange`. No SSE, no websockets — deferred by
  design.
- Renders correctly from 360 px phone width to wide desktop. Keyboard focus always visible.
  `prefers-reduced-motion` respected (motion is minimal anyway, see §3).
- The app must run **fully offline from mocks** — that is the default mode and the mode you
  demo in. Live mode is a settings toggle (§7).

## 3. Design direction

### 3.1 Identity: a public register's machine room

The public govfolio site is deliberately **"a public register, not a SaaS dashboard"** — cool
paper, engraving green (currency intaglio / official seals), bookish serif display, monospace
wherever bytes are evidence. The ops dashboard is the **machine room behind that register**:
same materials, same restraint, but denser and more instrumental — a day-book, not a brochure.
Do NOT restyle it as a generic dark-mode DevOps dashboard; do not reach for the default
SaaS look (gradient stat cards, rounded-2xl, purple accents). It should look like it was typeset
by the same hand as the public register.

**Light mode only, deliberately.** The public identity is cool paper; there is no validated
dark palette for it, and inventing one ad hoc would break chart-color guarantees. Commit to the
single light look.

### 3.2 Literal design tokens (paste verbatim as your `:root`)

This block is copied verbatim from the production site (`apps/web/src/app/globals.css`). It is
the law for every color and font decision:

```css
:root {
  --bg: #f5f7f6;
  --surface: #ffffff;
  --ink: #16211c;
  --muted: #51605a;
  --rule: #d7deda;
  --rule-strong: #a9b6b0;
  --seal: #176a4f;
  --seal-deep: #0e4a37;

  --v-unverified-ink: #4a5550;
  --v-unverified-bg: #eef1ef;
  --v-unverified-rule: #c5ccc8;
  --v-verified-ink: #14573f;
  --v-verified-bg: #e3f0e9;
  --v-verified-rule: #9dc4b2;
  --v-corrected-ink: #7a4a00;
  --v-corrected-bg: #f7eddc;
  --v-corrected-rule: #d8be92;
  --v-disputed-ink: #8f1d12;
  --v-disputed-bg: #f9e7e4;
  --v-disputed-rule: #dfa79f;

  --font-display: "Iowan Old Style", "Palatino Linotype", Palatino,
    "Book Antiqua", Georgia, serif;
  --font-body: -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --font-data: ui-monospace, "Cascadia Mono", "Segoe UI Mono", Consolas,
    Menlo, monospace;
}
```

Ops-only additions (derive nothing else):

```css
:root {
  /* sequential green ramp for magnitude fills — light -> dark, single hue,
     monotone lightness (the two lightest steps reuse production tokens) */
  --ramp-green-100: #e3f0e9;
  --ramp-green-250: #9dc4b2;
  --ramp-green-450: #4f9377;
  --ramp-green-600: #176a4f;
  --ramp-green-700: #0e4a37;

  /* status -> chart-fill aliases (see 3.5 for the rules) */
  --ok-fill: #14573f;      /* succeeded / healthy — --v-verified-ink   */
  --alert-fill: #8f1d12;   /* failed / frozen     — --v-disputed-ink   */
}
```

### 3.3 Typography & density (concrete scale — follow exactly)

The public site is airy; ops is **dense but never cramped**. Base the whole layout on a 4 px
spacing grid.

| Role | Face | Size / weight | Notes |
|---|---|---|---|
| App wordmark | `--font-display` | 1.2rem / 700 | `govfolio ops` with the seal-green terminal period (`.` colored `--seal`) — mirrors the public wordmark |
| View title (h1) | `--font-display` | 1.45rem / 600 | serif is for TITLES ONLY |
| Section head (h2) | `--font-display` | 1.05rem / 600 | hairline `--rule` underneath |
| Eyebrow / column label | `--font-body` | 0.6875rem / 600, uppercase, `letter-spacing: 0.06em`, color `--muted` | the structural voice of the dashboard |
| Body / cell text | `--font-body` | 0.875rem | line-height 1.45 |
| Numeric cells, IDs, hashes, JSON, timestamps | `--font-data` | 0.8125rem | **every number the dashboard shows is data — set it in mono** |
| Stat-tile value | `--font-data` | 1.65rem / 600 | mono digits are inherently equal-width; do NOT also apply `tabular-nums` here |
| Chart axis ticks | `--font-body` | 0.6875rem, color `--muted` | `font-variant-numeric: tabular-nums` |

Numeral rules (these come from data-viz practice — they matter):

- **Serif never touches a numeral.** The display face is for titles; a serif hero number reads
  as decoration, not instrumentation.
- **`tabular-nums` only where numbers align vertically** (sans-face table columns, axis
  ticks). Large standalone values do not get it.
- Thousands separators everywhere (`216,316`), UTC timestamps as `2026-07-08 14:02:11 UTC`
  plus a relative form (`18 min ago`) — always both, absolute first.

Layout: a slim left nav rail (the eight views as a ledger index — eyebrow-styled links, active
one marked with a 3 px `--seal` left rule), content column `max-width: 90rem`. Cards are
`--surface` white with a 1 px `--rule` border and **2 px border-radius** (near-square; the
register aesthetic — never rounded-xl). Page background `--bg`.

### 3.4 The one signature element (spend your boldness here, nowhere else)

The public site's signature is the provenance ledger — an archival catalog card with a
`3px double var(--seal)` top rule. The ops dashboard inherits it as **the pulse ledger** on the
Overview: `last_activity` rendered as a catalog card (double seal rule on top, label column in
muted eyebrows, mono UTC values + relative age), reading like the stamped inside-cover of a
lending-library book. Every other panel stays quiet and disciplined. One signature, executed
precisely — do not scatter double rules across the app.

Motion: near-none. 120 ms ease on hover states and the settings drawer; no entrance
animations; no skeleton screens (see §4 refetch rule); everything honors
`prefers-reduced-motion`.

### 3.5 Status pills & chart color (the rules, with validation receipts)

Status pills reuse the production verification-badge pattern (1 px border, 2 px radius,
0.75rem/600, `padding: 0.05rem 0.45rem`) with this fixed mapping:

| Ops state | Triplet | Used for |
|---|---|---|
| `ok` | `--v-verified-*` | succeeded runs, healthy sources, `sent` deliveries, db ok |
| `warn` | `--v-corrected-*` | stale running (> 1 h), backlog aging, pending review, cap ≥ 60 % |
| `alert` | `--v-disputed-*` | failed runs, frozen regimes, dead deliveries, cap ≥ 85 %, db error |
| `idle` | `--v-unverified-*` | running/pending/neutral, paused schedulers, zero-data states |

Pills always carry a **text label** (and, for warn/alert, a leading glyph `▲` / `●`) — state is
never color alone.

**Chart color carries exactly two jobs in this app — status and magnitude. There is no
categorical palette anywhere.** Series identity is always carried by rows, facets (small
multiples), or position — never by assigning hues to adapters. This is deliberate: it keeps
the register aesthetic (green + paper + one red) and sidesteps categorical-palette pitfalls
entirely.

- **Status fills:** `--ok-fill` (#14573f) vs `--alert-fill` (#8f1d12) are the only two fills
  that ever sit adjacent in one chart (succeeded/failed stacks). This exact pair was run
  through a colorblind-safety validator: **CVD ΔE 18.0 under protanopia (≥ 12 target), both
  ≥ 3:1 contrast on white** — legal as adjacent fills. Keep the pair; do not substitute.
- **Amber (#7a4a00 / `--v-corrected-ink`) is NEVER a chart fill next to red** — the validator
  puts the amber↔red pair at ΔE 5.8 under deuteranopia (indistinguishable). Amber appears only
  in pills and the meter (where it is the sole fill at that moment, labeled).
- **Magnitude fills** use the single-hue green ramp (`--ramp-green-*`), light→dark = less→more.
  Never a multi-hue ramp.
- **Text never wears a series color.** All labels, values, legends, axis text use `--ink` /
  `--muted`; a small colored swatch or line-key beside the text carries identity.
- Color follows the entity, never its rank: filtering never repaints survivors.

## 4. Information architecture — eight views

Global chart rules (apply to every chart below; they are the difference between an instrument
and a toy):

- **Form follows the data's job.** A single number is a **stat tile** (value + label + optional
  delta), never a one-bar chart. A ratio against a limit is a **meter**. Magnitude over time is
  **columns**. Comparing categories is **horizontal bars**. More than ~7 meaningful classes is
  a **table**, not more colors.
- **One axis per chart. Never two y-scales.** Two measures of different scale → two charts or
  small multiples.
- Marks: bars/columns **≤ 24 px thick**, 4 px rounded data-end, square at the baseline; lines
  2 px round-capped; gridlines and axes are **solid 1 px hairlines** in `--rule` (never dashed),
  recessive. Stacked segments and touching bars are separated by a **2 px gap in the surface
  color**, never by a stroke.
- **Legend for ≥ 2 series; none for a single series** (the title names it). Direct-label
  selectively — the endpoint, the extreme, the one that matters — never every point. Y-ticks
  round to clean thousands-comma'd numbers.
- **Every chart is hoverable.** Bars/cells: the mark is the hit target (≥ 24 px effective, so
  give thin marks an invisible widened hit rect), hovered mark lightens slightly, tooltip shows
  category + all values at that x (value first and bold, label second). Time series: a vertical
  hairline crosshair snaps to the nearest bucket. Keyboard focus shows the same tooltip. Build
  tooltip DOM with `textContent`, never innerHTML concatenation. **Tooltips enhance, never
  gate** — every charted value is also reachable in a table (each chart card gets a "table"
  toggle or sits next to its table twin).
- **Refetch keeps the frame:** while a poll is in flight, hold the previous render at
  `opacity: 0.6` — no skeletons, no layout jump. On error, keep the last good render and show
  the error banner (§7).
- Chart containers size to include the x-axis label band — no nested scrollbars.

### 4.1 Overview — `GET /v1/admin/ops/overview` — poll 10 s

The at-a-glance health wall. Widgets, top to bottom:

1. **KPI row** (stat tiles, wrap on narrow): `gold_records`, `filings`, `bronze_documents`,
   `politicians`, `review_open`, `gold_unverified`. Each: eyebrow label, mono value,
   no deltas in v1 (the API reports state, not history).
2. **Attention strip** — four wide tiles that are pills-at-tile-scale, colored by state:
   `frozen_regimes` (alert if > 0), `drift_open` (warn if > 0), `deliveries_dead` (alert if
   > 0), `outbox_undispatched` (warn if > 0, idle at 0). At zero they read quietly in
   `--v-unverified-*` — an all-quiet board is visibly all-quiet.
3. **Runs 24 h strip** (from `runs_24h`): `started`, `succeeded` (ok pill), `failed` (alert
   pill if > 0), `running_now` (idle), `stale_running` (warn if > 0 — label it
   "stale > 1 h, likely crashed").
4. **Cost-vs-cap meter** (from `extraction_month`): a horizontal meter, track in
   `--ramp-green-100`, fill in `--seal` while `cap_utilization_pct < 60`, `--v-corrected-ink`
   60–85, `--v-disputed-ink` ≥ 85. Right-aligned mono caption:
   `$12.84 of $200.00 (6.4%) — July 2026`. The values `estimated_cost_usd` and `hard_cap_usd`
   are **decimal strings — render verbatim, never parseFloat for display** (parse only to
   compute the fill ratio). Links to the Cost view.
5. **The pulse ledger** (signature element, §3.4) from `last_activity`: five rows — last run
   started, last publish succeeded, last sentinel check, last outbox dispatch, last gold row
   created. Each: absolute UTC + relative age; age > 24 h renders the relative part in
   `--v-corrected-ink` (the founder's "is it alive?" glance). `null` → an honest em-dash and
   "never recorded".
6. Footer line: `generated_at` ("computed 14:02:11 UTC · refreshed 3 s ago") plus a tiny
   healthz dot (§4.9 healthz poll): ok green / degraded amber / unreachable red, with text.

### 4.2 Runs — `GET /v1/admin/ops/runs` (poll 15 s) + `GET /v1/admin/ops/runs/summary` (poll 60 s)

The pipeline's flight recorder.

- **Filter row** (one row, above everything it scopes — never per-chart): adapter (text/select),
  stage (`fetch|parse|normalize|publish` and free text — enum is open), status
  (`running|succeeded|failed`), since/until (datetime-local, sent as RFC 3339). All filters
  apply to the table. The summary charts honor adapter/stage/status by filtering
  `summary.groups` and `summary.series` client-side (both carry `adapter`; `groups` also
  carries stage/status — `series` has neither, so stage/status filtering applies to groups
  only). **since/until cannot be expressed exactly for the summary** — the API's `hours`
  window always ends at *now* (there is no historical-range parameter). Use this exact
  mapping, do not invent another: when `since` is set,
  `hours = clamp(ceil((now − since) / 1 hour), 1, 720)`; ignore `until` for the summary;
  default `hours=24` when `since` is unset. Whenever since or until is active, render a mono
  caption directly under the summary charts:
  `summary window: trailing {hours} h ending now — wider than the table's since/until range`.
  Honesty rule: never let the charts silently pretend they honor the range.
- **Throughput chart** (from `summary.series`): columns per `bucket_start`, each column a
  succeeded/failed stack (`--ok-fill` / `--alert-fill`, 2 px surface gaps, legend present —
  two series). If more than one adapter is in play, **facet into small multiples per adapter**
  (shared y-scale, adapter name as facet eyebrow) — do NOT hue-code adapters into one chart.
  Crosshair tooltip: bucket time, succeeded, failed, gold_inserted, review_tasks.
- **Duration table** (from `summary.groups`): adapter × stage × status with runs, p50_seconds,
  p95_seconds — this is a table on purpose (three crossed dimensions is past chart-class
  limits). Mono numerals, `tabular-nums`, status as pills. `null` p50/p95 (nothing finished)
  → em-dash.
- **Run table** (from `runs`): columns id (mono, truncated with full value in `title`),
  adapter, stage, status pill, started_at, finished_at (or "running"), duration, error
  (truncated, alert-ink). Row click expands to a `<details>`-style panel: full
  `idempotency_key`, full error, and the `stats` JSON pretty-printed in a mono `<pre>`.
- **Pagination:** keyset. "Load older runs" passes `next_cursor` as `cursor`; append below;
  `next_cursor: null` → "end of history". Poll refreshes only the newest page (cursor-less
  query) and prepends new runs; it never disturbs loaded older pages.

### 4.3 Backfill — `GET /v1/admin/ops/backfill` — poll 60 s

The founder's "how much of history do we have?" wall. One section per regime
(`regimes[]`, ordered as returned):

- **Regime header:** `regime_code` (mono; `null` → "unmapped regime" + `regime_id`),
  `jurisdiction_id`, and the totals as a compact stat row (filings, bronze documents, silver
  rows, gold records, gold unverified, review open).
  - Footnote when the adapter is multi-body (repeat verbatim): *"Silver rows are attributed
    per document and review counts per adapter code — a document or task shared by both br
    chambers is counted under each chamber's row."*
- **Stage strip** (from `stages`): one row per stage — stage name, succeeded / failed /
  running counts as mini-pills. Caveat line (verbatim): *"Failure attribution is
  regime+stage level only — a failed run cannot be pinned to a year."*
- **Year matrix** (from `years`, oldest first): a table, one row per year. Columns:
  - `year` — mono; the `null` bucket renders LAST as **"unknown year"** in
    `--v-corrected-ink` with tooltip "filings whose filed_date is unknown" — never hide it,
    never lump it into a real year.
  - `filings`, `documents`, `gold_records` — mono numbers; `gold_records` cell carries an
    inline proportional bar (single-hue `--ramp-green-450` fill on `--ramp-green-100` track,
    scaled to the regime's max year) so magnitude scans vertically. Single series → no legend.
  - `gold_unverified` — number; > 0 gets a small warn pill.
  - **Coverage bar**: `politicians_with_filings` vs `roster_members` as a meter-style bar
    (fill `--seal` on `--ramp-green-100` track) with mono caption `431 / 435 (99.1%)`.
    Rules: `roster_members` null (and the unknown-year row) → no bar, caption
    `431 / — (roster unknown)`. **Coverage may legitimately exceed 100 %** (br is a
    candidacy regime: ~9.5 k filers vs 513 seats) — clamp the BAR at 100 % but print the true
    numbers and add the caption `(candidacy regime — filers exceed seated roster)` when
    ratio > 1.
- Optional `?adapter=` filter wired to the filter row (single input).

### 4.4 Freezes & drift — `GET /v1/admin/ops/freezes` — poll 30 s

Fail-closed surveillance. Two cards:

- **Watched sources** (from `sources`): table — regime_code (mono), state pill (**FROZEN**
  alert pill with `frozen_kind`, or ok "watching"), frozen_at, last_checked_at (absolute +
  relative; > 8 days old → warn ink, the sentinel is weekly), last_status (HTTP code, non-200
  alert ink), last_count, last_etag / last_modified / last_layout_hash (mono, truncated,
  full in `title`). Nulls → em-dash.
- **Drift reports** (from `drift`, already ranked worst-first — preserve order): table —
  priority_score (mono, bold for the top row), regime_code, drift_kind (mono),
  `freezes_publication` (`true` → alert pill "freezes publication", else muted "advisory"),
  detections, first/last detected, status, review_task_id (mono). Row expands to
  pretty-printed `detail` JSON in mono `<pre>`.
- Scope toggle `status=open` (default) / `all` in the filter row.
- Empty state (verbatim): *"No open drift. All watched sources match their baselines."*

### 4.5 Review health — `GET /v1/admin/ops/review-health` — poll 30 s

The human-judgment backlog (the pipeline never guesses — below-threshold matches become
review tasks).

- **Status row:** open / resolved / dismissed as stat tiles (open gets warn tint when > 0);
  `oldest_open_at` as a tile with relative age (> 7 days → warn ink).
- **Open by reason** (from `open_by_reason`, largest first): horizontal bars — one series, so
  every bar is the SAME hue (`--ramp-green-600`); do NOT rainbow the reasons and do NOT
  value-ramp them (length already encodes the count). Value direct-labeled at each bar end
  (mono); reason as the row label (mono, e.g. `unresolved_filer`); tooltip adds
  oldest_created_at + max_priority. No legend (single series).
- **Resolution throughput** (from `resolved_by_day`, 14 days): small column chart, single
  series `--ramp-green-450`, y from zero, clean ticks; crosshair tooltip. Zero-days render as
  zero-height columns on the visible baseline — an idle fortnight must look idle.
- **Sampling audit** (from `sample_audit`): table per month × regime — pending / confirmed /
  discrepancy, plus derived precision `confirmed / (confirmed + discrepancy)` as
  `97.7%` (em-dash when both are 0). Discrepancy > 0 gets a warn pill. Caption: *"Monthly
  random sample of published gold, human-verified (design §7.4)."*

### 4.6 Deliveries — `GET /v1/admin/ops/deliveries` — poll 30 s

Alert-delivery ledger and DLQ.

- **Ledger row** (from `by_status`): pending (idle), pending_digest (idle), sent (ok), dead
  (alert when > 0) tiles + `sent_24h` tile.
- **Outbox backlog:** `outbox_undispatched` tile; when > 0 show `oldest_undispatched_at`
  absolute + relative (relative in warn ink past 15 min — dispatch should be near-real-time).
- **Dead letters** (from `dead_recent`, 20 max): table — id (mono), alert_rule_id (mono),
  channel, attempts, updated_at, and **last_error verbatim** in mono, alert ink, wrapped not
  truncated (the founder debugs from this string; e.g. an HTTP 410 means the webhook endpoint
  is gone for good). Empty state: *"No dead deliveries. Every alert reached its destination."*

### 4.7 Cost — `GET /v1/admin/ops/extraction-costs?months=6` — poll 60 s

LLM extraction spend vs the founder's hard cap.

- **Monthly spend chart** (from `months`, oldest first): columns, single series `--seal`, one
  per month (every requested month is present — zeros render as flat baseline columns).
  **The cap line is ALWAYS drawn**: a horizontal 1 px line at `hard_cap_usd` in
  `--v-disputed-ink` with a right-aligned mono label `cap $200.00`. This means the y-domain is
  **always ≥ 200** even when every month is zero — the point of the chart is headroom, and
  honest headroom needs the cap in frame. Direct-label the current month's column with its
  value (`$12.84`); other months live in ticks + tooltip.
- **Month table** (twin of the chart): month, tokens_in, tokens_out, estimated_cost_usd
  (decimal string verbatim, mono), extraction_runs, cache_entries_created. `tabular-nums`.
- **Provenance note, render verbatim in the card footer:** *"Cost telemetry lands with goal
  021 Phase 2 (`pipeline_run.stats.extraction`). Until it ships, the live API honestly
  reports zeros — a zero here means 'not yet metered', and the hard cap line still stands.
  Hard cap: founder decision 2026-07-08, USD 200.00/month."*
- Money display rule (repeat of §5): `estimated_cost_usd` and `hard_cap_usd` are decimal
  strings — display verbatim; parse to number ONLY for bar/line geometry.

### 4.8 Cloud — no dedicated endpoint (static facts + `overview.last_activity` + `/healthz`)

The honest v1 cloud view: the substrate exists, but **Cloud Scheduler jobs are deliberately
PAUSED and the worker's HTTP entrypoints are not built yet** — so there is no live cloud job
signal to chart. Do not fake one. This view is a fact sheet + deep links:

- **Facts card** (render these statements verbatim):
  - *"All Cloud Scheduler jobs are created PAUSED — unpausing a tier is the explicit go-live
    act (fail closed: nothing polls a source before its adapter passes conformance)."*
  - *"The worker's HTTP stage endpoints (`/stages/discover`, `/stages/watch`) are not yet
    built; Scheduler targets exist but have nothing to call. Pipeline activity currently runs
    via local worker bins."*
  - *"Liveness signal on this page is DB-derived (overview.last_activity) plus `/healthz` —
    not GCP job status (native Tasks/Scheduler/Run status APIs are deferred)."*
- **Scheduler table** (static data: the `schedulerJobs` constant exported from `src/mocks.ts`,
  §6 — it is a repo fact, not an API payload, and is used in BOTH mock and live mode):
  `govfolio-discover-tier1` (`*/5 * * * *`, US House/Senate tier 1), `govfolio-discover-tier2`
  (`0 * * * *`, UK/AU/CA), `govfolio-discover-tier3` (`0 6 * * *`, annual regimes),
  `govfolio-sentinel-watch` (`0 6 * * 1`, weekly drift defense) — each with an idle **PAUSED**
  pill and its cadence in mono.
- **Liveness card:** the pulse-ledger rows relevant to cloud (last run started, last sentinel
  check) + the healthz dot with body values (`status`, `db`).
- **Console deep-links** (plain `<a>` list, `target="_blank" rel="noreferrer"`; project
  `govfolio`, region `us-central1`):
  - Cloud Run services (api / worker / web): `https://console.cloud.google.com/run?project=govfolio`
  - Cloud Scheduler: `https://console.cloud.google.com/cloudscheduler?project=govfolio`
  - Cloud Tasks queues (discover/fetch/parse/normalize/publish): `https://console.cloud.google.com/cloudtasks?project=govfolio`
  - Cloud SQL: `https://console.cloud.google.com/sql/instances?project=govfolio`
  - GCS buckets (`govfolio-bronze`, `govfolio-exports`): `https://console.cloud.google.com/storage/browser?project=govfolio`
  - Logs Explorer: `https://console.cloud.google.com/logs/query?project=govfolio`
  - Billing: `https://console.cloud.google.com/billing?project=govfolio`

### 4.9 `/healthz` — poll 15 s, global

Polled app-wide (not a view): unauthenticated, un-ETagged, always HTTP 200 with
`{ status: "ok"|"degraded", db: "ok"|"error" }`. Drives the header dot: green `ok`, amber
`degraded` (process up, DB down), red when the request itself fails (API unreachable). In mock
mode it is always ok.

## 5. The API contract (literal)

These types are extracted from the repo's GENERATED contract
(`packages/contracts/src/api.d.ts`, regenerated from the live Rust API in goal 090). Field
names, optionality, and nullability are exact — **treat this as read-only law; do not "fix"
or rename anything**. Save as `src/types.ts`.

One deliberate widening: the generator renders opaque JSON (`serde_json::Value`) as
`Record<string, never>`, which cannot hold realistic mock payloads. In this standalone bundle
those two fields (`PipelineRun.stats`, `DriftReportEntry.detail`) are typed `JsonValue`
instead. When porting into `apps/web` you will consume the generated types directly and
narrow at the boundary.

```ts
// src/types.ts — govfolio ops API wire shapes (generated-contract excerpts, goal 090)

/** Arbitrary JSON (widened from the generator's opaque Record<string, never>). */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

/** The error envelope every non-2xx response carries. */
export interface ErrorBody {
  error: ErrorDetail;
}
/** Machine-readable error code plus a human-readable message. */
export interface ErrorDetail {
  /** Stable machine-readable code (e.g. `invalid_cursor`, `not_found`). */
  code: string;
  message: string;
}

/** GET /healthz — liveness probe body. Served WITHOUT auth and WITHOUT ETag. */
export interface Healthz {
  /** `ok` | `error` — result of `select 1` on the pool. */
  db: string;
  /** `ok` when process and database both answer; `degraded` when the db does not. */
  status: string;
}

/** GET /v1/admin/ops/overview */
export interface OpsOverview {
  /** Current-month extraction spend vs the HARD CAP. */
  extraction_month: ExtractionMonth;
  /** When this body was computed (UTC, RFC 3339). */
  generated_at: string;
  last_activity: LastActivity;
  /** Trailing-24h run pulse. */
  runs_24h: Runs24h;
  /** Whole-system totals. */
  totals: OpsTotals;
}

export interface ExtractionMonth {
  /** Spend as a percentage of the cap (derived display number, not money). */
  cap_utilization_pct: number;
  /** Estimated spend, DECIMAL STRING (invariant 7). */
  estimated_cost_usd: string;
  /** The monthly HARD CAP, decimal string — always `"200.00"`. */
  hard_cap_usd: string;
  /** Month label, `YYYY-MM` (UTC). */
  month: string;
  tokens_in: number;
  tokens_out: number;
}

export interface LastActivity {
  last_gold_created_at?: string | null;
  last_outbox_dispatch_at?: string | null;
  last_publish_succeeded_at?: string | null;
  last_run_started_at?: string | null;
  /** Latest sentinel probe (`sentinel_watch.last_checked_at`). */
  last_sentinel_check_at?: string | null;
}

/** Running counts are global, not window-bounded. */
export interface Runs24h {
  failed: number;
  /** Runs currently `running` (any age). */
  running_now: number;
  /** Runs `running` for over an hour — likely crashed before finish. */
  stale_running: number;
  started: number;
  succeeded: number;
}

export interface OpsTotals {
  bronze_documents: number;
  deliveries_dead: number;
  drift_open: number;
  filings: number;
  frozen_regimes: number;
  gold_records: number;
  gold_unverified: number;
  outbox_undispatched: number;
  politicians: number;
  review_open: number;
  /** Silver staging rows across the per-regime `stg_*` tables. */
  silver_rows: number;
}

/** GET /v1/admin/ops/runs — one `pipeline_run` row. */
export interface PipelineRun {
  /** Adapter regime code, e.g. `us_house`. */
  adapter: string;
  /** Failure message, for `failed` runs. */
  error?: string | null;
  /** When it finished; `null` while running. */
  finished_at?: string | null;
  /** Run ULID, minted at claim — time-ordered, the pagination cursor. */
  id: string;
  /** Deterministic stage-unit key (crash-safe replay short-circuit). */
  idempotency_key: string;
  /** Stage name, e.g. `fetch` | `parse` | `normalize` | `publish`. */
  stage: string;
  started_at: string;
  /** Stage audit stats (e.g. PublishStats; `{}` until finish). */
  stats: JsonValue;
  /** `running` | `succeeded` | `failed`. */
  status: string;
}

export interface PipelineRunPage {
  /** Runs in descending id (= claim-time) order. */
  items: PipelineRun[];
  /** Pass back as `cursor` for the next (older) page; `null` at the end. */
  next_cursor?: string | null;
}

/** GET /v1/admin/ops/runs/summary */
export interface RunsSummary {
  /** Series bucket unit: `hour` | `day`. */
  bucket: string;
  groups: RunGroup[];
  series: RunSeriesPoint[];
  window_hours: number;
}

export interface RunGroup {
  adapter: string;
  /** Median run seconds (finished runs only; `null` when none finished). */
  p50_seconds?: number | null;
  p95_seconds?: number | null;
  runs: number;
  stage: string;
  status: string;
}

export interface RunSeriesPoint {
  adapter: string;
  /** Bucket start (UTC). */
  bucket_start: string;
  failed: number;
  /** Gold rows inserted (summed from publish-run `stats.gold_inserted`). */
  gold_inserted: number;
  /** Review tasks opened (summed from publish-run `stats.review_tasks`). */
  review_tasks: number;
  succeeded: number;
}

/** GET /v1/admin/ops/backfill */
export interface BackfillProgress {
  /** Per-regime progress, ordered by regime id. */
  regimes: RegimeBackfill[];
}

export interface RegimeBackfill {
  jurisdiction_id: string;
  /** Adapter regime code (`us_house`, `br`, ...); `null` when unmappable. */
  regime_code?: string | null;
  regime_id: string;
  /** Per-stage run progress for the regime's ADAPTER CODE (shared across
      regime rows of a multi-body adapter, e.g. both br chambers). */
  stages: StageProgress[];
  totals: RegimeTotals;
  /** Year-by-year progress, oldest first; the `null` year bucket last. */
  years: YearProgress[];
}

export interface RegimeTotals {
  bronze_documents: number;
  filings: number;
  gold_records: number;
  gold_unverified: number;
  /** Open review tasks attributed to this regime (code-keyed counts repeat
      on every regime row sharing the adapter code). */
  review_open: number;
  /** Attributed per DOCUMENT — a document shared by filings of more than one
      regime reports its full count under each. */
  silver_rows: number;
}

/** Failure attribution is regime+stage level only — no year. */
export interface StageProgress {
  failed: number;
  running: number;
  stage: string;
  succeeded: number;
}

export interface YearProgress {
  documents: number;
  filings: number;
  gold_records: number;
  gold_unverified: number;
  politicians_with_filings: number;
  /** Roster denominator; `null` for the unknown-year bucket (or no roster). */
  roster_members?: number | null;
  /** Filing year; `null` = filings whose `filed_date` is unknown. */
  year?: number | null;
}

/** GET /v1/admin/ops/freezes */
export interface FreezeStatus {
  /** Drift reports, worst first. */
  drift: DriftReportEntry[];
  /** Every watched source's baseline (freeze state included). */
  sources: SentinelSource[];
}

export interface DriftReportEntry {
  /** Dedup key (`regime_code:kind:signature`). */
  dedup_key: string;
  detail: JsonValue;
  /** How many times the same open anomaly re-detected. */
  detections: number;
  /** Anomaly kind, e.g. `layout_shift`. */
  drift_kind: string;
  first_detected_at: string;
  /** Whether this anomaly froze publication. */
  freezes_publication: boolean;
  id: string;
  last_detected_at: string;
  /** Severity rank — worst first. */
  priority_score: number;
  regime_code: string;
  review_task_id?: string | null;
  /** `open` | `resolved` | `superseded`. */
  status: string;
}

export interface SentinelSource {
  frozen: boolean;
  frozen_at?: string | null;
  frozen_kind?: string | null;
  last_checked_at: string;
  last_count?: number | null;
  last_etag?: string | null;
  last_layout_hash?: string | null;
  last_modified?: string | null;
  last_status?: number | null;
  regime_code: string;
}

/** GET /v1/admin/ops/review-health */
export interface ReviewHealth {
  by_status: ReviewStatusCounts;
  oldest_open_at?: string | null;
  /** Open tasks rolled up by reason, largest first. */
  open_by_reason: OpenReason[];
  /** Tasks resolved per UTC day over the trailing 14 days. */
  resolved_by_day: ResolvedDay[];
  /** Sampling-audit slices, newest month first. */
  sample_audit: SampleAuditSlice[];
}

export interface ReviewStatusCounts {
  dismissed: number;
  open: number;
  resolved: number;
}

export interface OpenReason {
  max_priority: number;
  oldest_created_at: string;
  open: number;
  /** Task reason, e.g. `unresolved_filer`. */
  reason: string;
}

export interface ResolvedDay {
  /** Day start (UTC). */
  day: string;
  resolved: number;
}

export interface SampleAuditSlice {
  confirmed: number;
  discrepancy: number;
  /** Drawn records awaiting a verdict. */
  pending: number;
  regime_id: string;
  /** Batch label, `YYYY-MM`. */
  sample_month: string;
}

/** GET /v1/admin/ops/deliveries */
export interface DeliveryHealth {
  by_status: DeliveryStatusCounts;
  /** The 20 most recently dead-lettered deliveries. */
  dead_recent: DeadDelivery[];
  /** Oldest undispatched event's creation time (backlog age). */
  oldest_undispatched_at?: string | null;
  outbox_undispatched: number;
  sent_24h: number;
}

export interface DeliveryStatusCounts {
  dead: number;
  pending: number;
  pending_digest: number;
  sent: number;
}

export interface DeadDelivery {
  alert_rule_id: string;
  attempts: number;
  /** `email` | `webhook`. */
  channel: string;
  id: string;
  /** The final failure. */
  last_error?: string | null;
  updated_at: string;
}

/** GET /v1/admin/ops/extraction-costs */
export interface ExtractionCostReport {
  /** The monthly HARD CAP, decimal string — always `"200.00"`. */
  hard_cap_usd: string;
  /** Months, oldest first — every requested month present, zeros when idle. */
  months: ExtractionCostMonth[];
}

export interface ExtractionCostMonth {
  cache_entries_created: number;
  /** Estimated spend, DECIMAL STRING (invariant 7). */
  estimated_cost_usd: string;
  /** Parse-stage runs that recorded a `stats.extraction` block. */
  extraction_runs: number;
  /** Month label, `YYYY-MM` (UTC). */
  month: string;
  tokens_in: number;
  tokens_out: number;
}

// ---- query params (all optional; server defaults noted) ----

export interface RunsQuery {
  adapter?: string;
  stage?: string;
  /** `running` | `succeeded` | `failed` */
  status?: string;
  /** RFC 3339 */
  since?: string;
  until?: string;
  /** run id of the last item on the previous page */
  cursor?: string;
  /** 1..=200, default 50 */
  limit?: number;
}
export interface RunsSummaryQuery {
  /** 1..=720, default 24 */
  hours?: number;
  /** `hour` (default) | `day` */
  bucket?: string;
}
export interface BackfillQuery {
  adapter?: string;
}
export interface FreezesQuery {
  /** `open` (default) | `all` */
  status?: string;
}
export interface ExtractionCostsQuery {
  /** 1..=24, default 3 */
  months?: number;
}
```

### Protocol rules (all verified against the live implementation)

- **Base paths:** `GET /healthz` (no auth) and `GET /v1/admin/ops/{overview, runs,
  runs/summary, backfill, freezes, review-health, deliveries, extraction-costs}` (admin-gated).
- **Auth:** every `/v1/admin/ops/*` request needs the `X-Admin-Token` header. Exactly three
  401 codes exist — surface each with its own guidance:
  - `admin_disabled` — the API was started without `ADMIN_TOKEN` configured; the admin
    surface is disabled server-side (fail closed). Message: "the API has no admin token
    configured — set ADMIN_TOKEN and restart it."
  - `admin_token_required` — the header is missing. Message: "enter the admin token in
    settings."
  - `invalid_admin_token` — the header is present but wrong.
- **Errors:** every non-2xx body is the `ErrorBody` envelope `{"error":{"code","message"}}`.
  400s carry codes like `invalid_status`, `invalid_cursor`, `invalid_limit`. Parse
  defensively: if the body isn't the envelope, fall back to `code: "unknown"` + HTTP status.
- **ETag / 304:** every authenticated GET returns a strong `ETag` and honors `If-None-Match`
  with `304 Not Modified` (empty body). `/healthz` deliberately has NO ETag. In the browser
  (same-origin/proxied), standard HTTP caching handles this automatically; optionally keep a
  per-URL `{etag, body}` map, send `If-None-Match`, and reuse the cached body on 304. Never
  treat a 304 as an error or as empty data.
- **Money is decimal strings** (`"12.84"`, `"200.00"`) — repo invariant 7, no floats on the
  wire. Display verbatim; convert to number only for geometry (bar heights, meter fill).
- **Cursor pagination** (runs only): keyset on descending ULID `id`; response `next_cursor`
  echoes into request `cursor`; `null` = end.
- **Timestamps** are RFC 3339 UTC strings; dates are `YYYY-MM-DD`; months `YYYY-MM`.

## 6. Embedded mock data (complete, typed)

Save as `src/mocks.ts` verbatim. It typechecks against §5 (`satisfies` requires TS ≥ 4.9) and
is seeded from real ops facts: us_house backfilled 2012–2026, br live for the 2022 nationwide
DEPUTADO FEDERAL candidacy sweep (~35 k gold rows, 678 `unresolved_filer` review tasks), one
frozen regime (`br`, `layout_shift`, publication-freezing), one dead webhook delivery (HTTP
410), July 2026 extraction spend $12.84 of the $200.00 cap. Mock "now" is
**2026-07-08T14:20:00Z** — compute relative ages against a frozen `MOCK_NOW`, not
`Date.now()`, so the mock demo is deterministic.

```ts
// src/mocks.ts — one realistic payload per endpoint. Facts mirror real pipeline
// state on 2026-07-08; the cost figures preview the post-021 world (the live
// endpoint returns zeros until stats.extraction lands).
import type {
  BackfillProgress,
  DeliveryHealth,
  ExtractionCostReport,
  FreezeStatus,
  Healthz,
  OpsOverview,
  PipelineRunPage,
  ReviewHealth,
  RunsSummary,
} from "./types";

export const MOCK_NOW = "2026-07-08T14:20:00Z";

export const mockHealthz = {
  status: "ok",
  db: "ok",
} satisfies Healthz;

export const mockOverview = {
  generated_at: "2026-07-08T14:19:58Z",
  totals: {
    bronze_documents: 26_142,
    silver_rows: 226_470,
    filings: 34_184,
    gold_records: 216_316,
    gold_unverified: 35_238,
    politicians: 10_675,
    review_open: 707,
    frozen_regimes: 1,
    drift_open: 2,
    deliveries_dead: 1,
    outbox_undispatched: 3,
  },
  runs_24h: {
    started: 42,
    succeeded: 38,
    failed: 3,
    running_now: 2,
    stale_running: 1,
  },
  extraction_month: {
    month: "2026-07",
    tokens_in: 3_182_400,
    tokens_out: 396_910,
    estimated_cost_usd: "12.84",
    hard_cap_usd: "200.00",
    cap_utilization_pct: 6.42,
  },
  last_activity: {
    last_run_started_at: "2026-07-08T14:05:11Z",
    last_publish_succeeded_at: "2026-07-08T14:02:47Z",
    last_sentinel_check_at: "2026-07-06T06:02:41Z",
    last_outbox_dispatch_at: "2026-07-08T13:58:20Z",
    last_gold_created_at: "2026-07-08T14:02:46Z",
  },
} satisfies OpsOverview;

export const mockRuns = {
  items: [
    {
      id: "01K01YAW7Q4R8ZJ3T9WVH2E5DB",
      adapter: "us_house",
      stage: "fetch",
      status: "running",
      idempotency_key: "fetch:us_house:2026-07-08:doc-20031187",
      stats: {},
      error: null,
      started_at: "2026-07-08T14:05:11Z",
      finished_at: null,
    },
    {
      id: "01K01Y9T2M6XW3VNCPE5H9RD7F",
      adapter: "us_house",
      stage: "publish",
      status: "succeeded",
      idempotency_key: "publish:us_house:sha256:9c53a91cce5db4e2",
      stats: {
        filings: 1,
        gold_inserted: 12,
        gold_skipped: 0,
        review_tasks: 0,
        outbox_events: 12,
      },
      error: null,
      started_at: "2026-07-08T14:02:44Z",
      finished_at: "2026-07-08T14:02:47Z",
    },
    {
      id: "01K01Y8G5C2DAKQ4T7XWM9NE3H",
      adapter: "us_house",
      stage: "parse",
      status: "succeeded",
      idempotency_key: "parse:us_house:sha256:9c53a91cce5db4e2",
      stats: { rows: 12, zero_row: false },
      error: null,
      started_at: "2026-07-08T14:01:58Z",
      finished_at: "2026-07-08T14:02:01Z",
    },
    {
      id: "01K01XZQ8V4B6ND2GSJ7R3TCWA",
      adapter: "br",
      stage: "publish",
      status: "failed",
      idempotency_key: "publish:br:sha256:457751d5acb51120",
      stats: {},
      error:
        "publication frozen: open drift 'layout_shift' on br (report 01K01T2H9Q6WVXR3PBM7E4CDN8) — fail closed, rejected before write",
      started_at: "2026-07-08T11:40:03Z",
      finished_at: "2026-07-08T11:40:09Z",
    },
    {
      id: "01K01WM2E9HTPX5A8ZQCV4RD6G",
      adapter: "us_house",
      stage: "fetch",
      status: "failed",
      idempotency_key: "fetch:us_house:2026-07-08:doc-20031162",
      stats: {},
      error: "HTTP 500 from disclosures-clerk.house.gov after 3 polite retries",
      started_at: "2026-07-08T09:12:40Z",
      finished_at: "2026-07-08T09:13:22Z",
    },
    {
      id: "01K01V5D3W7JBRF6MYA2QH8KE9",
      adapter: "us_house",
      stage: "normalize",
      status: "running",
      idempotency_key: "normalize:us_house:sha256:2f81aa04cd7712e0",
      stats: {},
      error: null,
      started_at: "2026-07-08T07:02:19Z",
      finished_at: null,
    },
  ],
  next_cursor: "01K01V5D3W7JBRF6MYA2QH8KE9",
} satisfies PipelineRunPage;

export const mockRunsSummary = {
  window_hours: 24,
  bucket: "hour",
  groups: [
    { adapter: "us_house", stage: "fetch", status: "succeeded", runs: 10, p50_seconds: 2.1, p95_seconds: 4.8 },
    { adapter: "us_house", stage: "parse", status: "succeeded", runs: 10, p50_seconds: 0.9, p95_seconds: 2.2 },
    { adapter: "us_house", stage: "normalize", status: "succeeded", runs: 9, p50_seconds: 0.6, p95_seconds: 1.4 },
    { adapter: "us_house", stage: "publish", status: "succeeded", runs: 9, p50_seconds: 1.4, p95_seconds: 3.1 },
    { adapter: "us_house", stage: "fetch", status: "failed", runs: 1, p50_seconds: 42.0, p95_seconds: 42.0 },
    { adapter: "us_house", stage: "fetch", status: "running", runs: 1, p50_seconds: null, p95_seconds: null },
    { adapter: "us_house", stage: "normalize", status: "running", runs: 1, p50_seconds: null, p95_seconds: null },
    { adapter: "br", stage: "publish", status: "failed", runs: 2, p50_seconds: 5.8, p95_seconds: 6.1 },
  ],
  series: [
    { bucket_start: "2026-07-08T09:00:00Z", adapter: "us_house", succeeded: 5, failed: 1, gold_inserted: 41, review_tasks: 0 },
    { bucket_start: "2026-07-08T10:00:00Z", adapter: "us_house", succeeded: 6, failed: 0, gold_inserted: 87, review_tasks: 1 },
    { bucket_start: "2026-07-08T11:00:00Z", adapter: "us_house", succeeded: 6, failed: 0, gold_inserted: 52, review_tasks: 0 },
    { bucket_start: "2026-07-08T11:00:00Z", adapter: "br", succeeded: 0, failed: 1, gold_inserted: 0, review_tasks: 0 },
    { bucket_start: "2026-07-08T12:00:00Z", adapter: "us_house", succeeded: 6, failed: 0, gold_inserted: 64, review_tasks: 0 },
    { bucket_start: "2026-07-08T13:00:00Z", adapter: "us_house", succeeded: 8, failed: 0, gold_inserted: 93, review_tasks: 2 },
    { bucket_start: "2026-07-08T13:00:00Z", adapter: "br", succeeded: 0, failed: 1, gold_inserted: 0, review_tasks: 0 },
    { bucket_start: "2026-07-08T14:00:00Z", adapter: "us_house", succeeded: 7, failed: 0, gold_inserted: 38, review_tasks: 0 },
  ],
} satisfies RunsSummary;

const usHouseYears = [
  { year: 2012, filings: 1_402, documents: 1_402, gold_records: 9_811, gold_unverified: 0, politicians_with_filings: 419, roster_members: 435 },
  { year: 2013, filings: 1_366, documents: 1_366, gold_records: 9_452, gold_unverified: 0, politicians_with_filings: 411, roster_members: 435 },
  { year: 2014, filings: 1_451, documents: 1_451, gold_records: 10_240, gold_unverified: 0, politicians_with_filings: 422, roster_members: 435 },
  { year: 2015, filings: 1_538, documents: 1_538, gold_records: 11_102, gold_unverified: 0, politicians_with_filings: 426, roster_members: 435 },
  { year: 2016, filings: 1_611, documents: 1_611, gold_records: 11_874, gold_unverified: 0, politicians_with_filings: 428, roster_members: 435 },
  { year: 2017, filings: 1_690, documents: 1_690, gold_records: 12_490, gold_unverified: 0, politicians_with_filings: 430, roster_members: 435 },
  { year: 2018, filings: 1_744, documents: 1_744, gold_records: 13_006, gold_unverified: 0, politicians_with_filings: 425, roster_members: 435 },
  { year: 2019, filings: 1_803, documents: 1_803, gold_records: 13_562, gold_unverified: 0, politicians_with_filings: 431, roster_members: 435 },
  { year: 2020, filings: 1_918, documents: 1_918, gold_records: 14_777, gold_unverified: 0, politicians_with_filings: 433, roster_members: 435 },
  { year: 2021, filings: 2_050, documents: 2_050, gold_records: 15_940, gold_unverified: 0, politicians_with_filings: 434, roster_members: 435 },
  { year: 2022, filings: 1_986, documents: 1_986, gold_records: 15_204, gold_unverified: 0, politicians_with_filings: 429, roster_members: 435 },
  { year: 2023, filings: 1_871, documents: 1_871, gold_records: 14_388, gold_unverified: 0, politicians_with_filings: 427, roster_members: 435 },
  { year: 2024, filings: 1_940, documents: 1_940, gold_records: 14_902, gold_unverified: 12, politicians_with_filings: 431, roster_members: 435 },
  { year: 2025, filings: 1_114, documents: 1_114, gold_records: 8_461, gold_unverified: 96, politicians_with_filings: 402, roster_members: 435 },
  { year: 2026, filings: 812, documents: 812, gold_records: 5_899, gold_unverified: 128, politicians_with_filings: 344, roster_members: 435 },
  { year: null, filings: 14, documents: 14, gold_records: 96, gold_unverified: 4, politicians_with_filings: 13, roster_members: null },
];

export const mockBackfill = {
  regimes: [
    {
      regime_code: "us_house",
      regime_id: "0HSEREG0000000000000000001",
      jurisdiction_id: "us",
      totals: {
        bronze_documents: 24_310,
        silver_rows: 190_450,
        filings: 24_310,
        gold_records: 181_204,
        gold_unverified: 240,
        review_open: 29,
      },
      stages: [
        { stage: "fetch", succeeded: 26_054, failed: 12, running: 1 },
        { stage: "parse", succeeded: 26_042, failed: 3, running: 0 },
        { stage: "normalize", succeeded: 24_310, failed: 2, running: 1 },
        { stage: "publish", succeeded: 24_298, failed: 5, running: 0 },
      ],
      years: usHouseYears,
    },
    {
      regime_code: "br",
      regime_id: "0BRAREG0000000000000000001",
      jurisdiction_id: "br",
      totals: {
        bronze_documents: 27,
        silver_rows: 36_020,
        filings: 9_874,
        gold_records: 35_112,
        gold_unverified: 34_998,
        review_open: 678,
      },
      stages: [
        { stage: "fetch", succeeded: 27, failed: 0, running: 0 },
        { stage: "parse", succeeded: 27, failed: 0, running: 0 },
        { stage: "normalize", succeeded: 27, failed: 0, running: 0 },
        { stage: "publish", succeeded: 25, failed: 2, running: 0 },
      ],
      years: [
        {
          year: 2022,
          filings: 9_874,
          documents: 27,
          gold_records: 35_112,
          gold_unverified: 34_998,
          politicians_with_filings: 9_513,
          roster_members: 513,
        },
      ],
    },
  ],
} satisfies BackfillProgress;

export const mockFreezes = {
  sources: [
    {
      regime_code: "us_house",
      frozen: false,
      frozen_at: null,
      frozen_kind: null,
      last_checked_at: "2026-07-06T06:02:41Z",
      last_status: 200,
      last_count: 24_310,
      last_etag: '"5f0c-63a1b9e4d02c1"',
      last_modified: "Tue, 07 Jul 2026 22:10:04 GMT",
      last_layout_hash:
        "9c53a91cce5db4e201889fb580df5e4d43db4df9157fefbece42e7a1019dd5e7",
    },
    {
      regime_code: "br",
      frozen: true,
      frozen_at: "2026-07-06T06:02:41Z",
      frozen_kind: "layout_shift",
      last_checked_at: "2026-07-06T06:02:41Z",
      last_status: 200,
      last_count: null,
      last_etag: null,
      last_modified: null,
      last_layout_hash:
        "457751d5acb511207eff08be810cd01238569c628da95773481abbb588984e31",
    },
  ],
  drift: [
    {
      id: "01K01T2H9Q6WVXR3PBM7E4CDN8",
      dedup_key: "br:layout_shift:457751d5acb51120",
      regime_code: "br",
      drift_kind: "layout_shift",
      detail: {
        expected_layout_hash: "0481e1b5602294554f9327955c04f7da",
        observed_layout_hash: "457751d5acb511207eff08be810cd012",
        selector_hits: { table_rows: 0, expected_min: 25 },
      },
      detections: 3,
      first_detected_at: "2026-06-22T06:01:12Z",
      last_detected_at: "2026-07-06T06:02:41Z",
      freezes_publication: true,
      priority_score: 87.5,
      review_task_id: "01K01T2J4A8NEYC5QVD9W6RBH2",
      status: "open",
    },
    {
      id: "01K01SVF2B7DGXK4WTA8M3PEC6",
      dedup_key: "us_house:filing_count_drop:2026-07",
      regime_code: "us_house",
      drift_kind: "filing_count_drop",
      detail: { previous_count: 24_355, observed_count: 24_310, drop_pct: 0.18 },
      detections: 1,
      first_detected_at: "2026-07-06T06:02:41Z",
      last_detected_at: "2026-07-06T06:02:41Z",
      freezes_publication: false,
      priority_score: 41.0,
      review_task_id: null,
      status: "open",
    },
  ],
} satisfies FreezeStatus;

export const mockReviewHealth = {
  by_status: { open: 707, resolved: 1_412, dismissed: 96 },
  oldest_open_at: "2026-06-19T03:14:52Z",
  open_by_reason: [
    { reason: "unresolved_filer", open: 678, oldest_created_at: "2026-06-19T03:14:52Z", max_priority: 74.0 },
    { reason: "below_threshold_instrument", open: 21, oldest_created_at: "2026-06-27T18:40:09Z", max_priority: 55.5 },
    { reason: "drift", open: 5, oldest_created_at: "2026-06-22T06:01:12Z", max_priority: 87.5 },
    { reason: "zero_rows", open: 3, oldest_created_at: "2026-07-01T10:22:30Z", max_priority: 62.0 },
  ],
  resolved_by_day: [
    { day: "2026-06-25T00:00:00Z", resolved: 34 },
    { day: "2026-06-26T00:00:00Z", resolved: 51 },
    { day: "2026-06-27T00:00:00Z", resolved: 12 },
    { day: "2026-06-28T00:00:00Z", resolved: 0 },
    { day: "2026-06-29T00:00:00Z", resolved: 88 },
    { day: "2026-06-30T00:00:00Z", resolved: 64 },
    { day: "2026-07-01T00:00:00Z", resolved: 42 },
    { day: "2026-07-02T00:00:00Z", resolved: 57 },
    { day: "2026-07-03T00:00:00Z", resolved: 23 },
    { day: "2026-07-04T00:00:00Z", resolved: 0 },
    { day: "2026-07-05T00:00:00Z", resolved: 19 },
    { day: "2026-07-06T00:00:00Z", resolved: 71 },
    { day: "2026-07-07T00:00:00Z", resolved: 46 },
    { day: "2026-07-08T00:00:00Z", resolved: 18 },
  ],
  sample_audit: [
    { sample_month: "2026-06", regime_id: "0HSEREG0000000000000000001", pending: 12, confirmed: 86, discrepancy: 2 },
    { sample_month: "2026-05", regime_id: "0HSEREG0000000000000000001", pending: 0, confirmed: 97, discrepancy: 3 },
  ],
} satisfies ReviewHealth;

export const mockDeliveries = {
  by_status: { pending: 2, pending_digest: 5, sent: 1_284, dead: 1 },
  sent_24h: 37,
  outbox_undispatched: 3,
  oldest_undispatched_at: "2026-07-08T14:11:05Z",
  dead_recent: [
    {
      id: "01K01M8T5XWQC2VJRAE94HN7DB",
      alert_rule_id: "01JZY8Q4T2M6XW3VNCPE5H9RD7",
      channel: "webhook",
      attempts: 8,
      last_error:
        "HTTP 410 Gone from https://hooks.example.dev/govfolio-alerts: endpoint permanently retired; dead-lettered after 8 attempts",
      updated_at: "2026-07-07T21:44:18Z",
    },
  ],
} satisfies DeliveryHealth;

export const mockExtractionCosts = {
  hard_cap_usd: "200.00",
  months: [
    { month: "2026-02", tokens_in: 0, tokens_out: 0, estimated_cost_usd: "0.00", extraction_runs: 0, cache_entries_created: 0 },
    { month: "2026-03", tokens_in: 0, tokens_out: 0, estimated_cost_usd: "0.00", extraction_runs: 0, cache_entries_created: 0 },
    { month: "2026-04", tokens_in: 0, tokens_out: 0, estimated_cost_usd: "0.00", extraction_runs: 0, cache_entries_created: 0 },
    { month: "2026-05", tokens_in: 0, tokens_out: 0, estimated_cost_usd: "0.00", extraction_runs: 0, cache_entries_created: 0 },
    { month: "2026-06", tokens_in: 2_204_800, tokens_out: 261_450, estimated_cost_usd: "8.91", extraction_runs: 151, cache_entries_created: 149 },
    { month: "2026-07", tokens_in: 3_182_400, tokens_out: 396_910, estimated_cost_usd: "12.84", extraction_runs: 214, cache_entries_created: 209 },
  ],
} satisfies ExtractionCostReport;

/** §4.8 Cloud view scheduler table. Static repo facts, NOT an API shape (no
 *  scheduler endpoint exists; every job is deliberately PAUSED) — the type
 *  lives here, not in types.ts, because types.ts is generated-contract law. */
export interface SchedulerJob {
  name: string;
  cron: string;
  scope: string;
  state: "PAUSED";
}

export const schedulerJobs: SchedulerJob[] = [
  { name: "govfolio-discover-tier1", cron: "*/5 * * * *", scope: "US House/Senate tier 1", state: "PAUSED" },
  { name: "govfolio-discover-tier2", cron: "0 * * * *", scope: "UK/AU/CA", state: "PAUSED" },
  { name: "govfolio-discover-tier3", cron: "0 6 * * *", scope: "annual regimes", state: "PAUSED" },
  { name: "govfolio-sentinel-watch", cron: "0 6 * * 1", scope: "weekly drift defense", state: "PAUSED" },
];
```

Internal-consistency facts you may rely on (and must not break if you extend the mocks):
overview `review_open` 707 = 678+21+5+3 across reasons; `frozen_regimes` 1 = the one frozen
sentinel source; `deliveries_dead` 1 = the one DLQ row; the July overview `extraction_month`
equals the July row of `mockExtractionCosts`; br coverage 9,513 filers vs 513 seats
intentionally exceeds 100 % (§4.3 rule); the us_house year list intentionally ends with the
`year: null` unknown bucket; a regime's `totals.bronze_documents` can never exceed the sum
of its years' `documents` (every counted document appears in at least one year bucket) —
us_house is exactly 24,310 = its year-sum, while the overview's whole-system 26,142 is
legitimately larger (it also counts Bronze documents that produced no filing).

## 7. Data layer & swap-to-live

One module (`src/data.ts`) owns ALL data access. Views never call `fetch` directly.

- **`dataSource: 'mock' | 'live'`** app state, default `'mock'`. In mock mode every "fetch"
  resolves the §6 constant (wrap in `Promise.resolve` with a ~150 ms delay so loading states
  are exercised). In live mode it fetches `${baseUrl}${path}` with headers.
  **One mock-mode carve-out — cursor pagination:** a `runs` request that carries a `cursor`
  resolves `{ items: [], next_cursor: null }`, NOT the §6 constant (whose `next_cursor` is its
  own last item id). So in mock mode one "Load older runs" click cleanly reaches "end of
  history"; resolving the same six runs again would append duplicate ULID keys (React
  console errors) and an unreachable end — never do that.
- **Settings drawer** (gear icon, top right): dataSource toggle; base URL text input (default
  `http://localhost:8080`); admin token password input. **The token lives in React state
  only — never localStorage, never sessionStorage, never cookies, never the URL.** It is a
  bearer credential; a page refresh forgetting it is correct behavior. Show a muted note in
  the drawer saying exactly that.
- **Request shape (live):** `GET` with `accept: application/json` and, for `/v1/admin/ops/*`,
  `X-Admin-Token: <token>`. Query params via `URLSearchParams`, omitting undefined.
- **Per-panel poll loop:** each view registers `(fetcher, intervalMs)`; a shared hook runs
  `setTimeout` chains (not `setInterval` — no overlapping requests), pauses when
  `document.hidden`, refreshes immediately on becoming visible, and exposes
  `{ data, error, lastSuccessAt, isStale, refresh }`.
- **Error banner (per panel, not global):** on any failure render a slim banner inside the
  panel: the envelope `code` + `message` (or `network_error` when fetch throws), and
  `last successful refresh: 14:02:11 UTC (3 min ago)` — or "never" if no success yet. The
  panel KEEPS rendering its last good data at reduced emphasis with a "stale" idle pill.
  **Never fabricate, zero-fill, or hide data on failure** — an error plus honest stale data,
  always. Map the three 401 codes to the §5 guidance strings.
- **304 handling:** a 304 means "unchanged" — keep current data, update `lastSuccessAt`.
- **CORS — read this before trying live mode in a browser.** The API deliberately sends **no
  CORS headers** (`Access-Control-Allow-Origin` is absent; CORS support is an explicitly
  deferred item). A browser page served from any other origin — including a claude.ai
  artifact — will have every live request blocked by the browser. Three sanctioned paths:
  1. **Artifact / preview: always mock mode.** Do not attempt live calls from an artifact.
  2. **Local dev: same-origin via the Vite proxy.** Serve the SPA from Vite and proxy to the
     API; set base URL to empty string (same-origin):
     ```ts
     // vite.config.ts
     import { defineConfig } from "vite";
     import react from "@vitejs/plugin-react";

     export default defineConfig({
       plugins: [react()],
       server: {
         proxy: {
           "/healthz": "http://localhost:8080",
           "/v1": "http://localhost:8080",
         },
       },
     });
     ```
  3. **The eventual home: port into `apps/web` (Next.js).** There the ops pages fetch
     **server-side** using the repo's existing typed client pattern — the admin header comes
     from the server env, never the browser. This is the production pattern to preserve
     (excerpt from `apps/web/src/lib/api.ts`):
     ```ts
     function adminHeaders(): Record<string, string> {
       const token = process.env.GOVFOLIO_ADMIN_TOKEN;
       return token !== undefined && token !== "" ? { "x-admin-token": token } : {};
     }
     ```
     Keep your view components pure functions of the §5 types, with all fetching in
     `data.ts`, and the port is mechanical: swap `data.ts` for server loaders over the
     generated `@govfolio/contracts` client.

## 8. Acceptance checklist

Build is done when every line holds:

- [ ] `tsc --noEmit` clean under `strict: true`; zero `any` (including casts); the §6 mocks
      typecheck against the §5 types unmodified.
- [ ] All eight views render **offline from mocks** with no network access and no console
      errors; mock mode is the default.
- [ ] Honest states everywhere: distinct loading (first load only), empty ("No open drift…",
      "No dead deliveries…"), and error (envelope code + message + last-success time, stale
      data kept visible) — never a fabricated or silently-zeroed value.
- [ ] Every timestamp shows **absolute UTC + relative age**, absolute first; relative ages in
      mock mode computed against `MOCK_NOW`.
- [ ] Cost view: the **$200.00 cap line is visible even when all months are zero** (y-domain
      always includes the cap); money strings rendered verbatim, never floated for display.
- [ ] Backfill: the **unknown-year (`year: null`) bucket is visible and labeled** on
      us_house; br coverage shows `9,513 / 513` with a clamped bar and the candidacy-regime
      caption; the multi-body/silver-attribution and stage-level-failure caveats are rendered.
- [ ] Polling: per-panel cadences per §4, `setTimeout` chains, **paused on `document.hidden`**,
      immediate refresh on return; refetch holds the previous render at reduced opacity (no
      skeleton flash, no layout jump).
- [ ] Charts follow §4 rules: inline SVG only; one axis; bars ≤ 24 px with 4 px rounded
      data-ends; solid hairline grid; 2 px surface gaps in stacks; legend iff ≥ 2 series;
      selective direct labels; adapters faceted, never hue-coded; only the validated
      green/red pair ever adjacent; every chart has a table twin; tooltips on hover AND
      keyboard focus, built with `textContent`.
- [ ] Status pills always pair color with a text label (and glyph for warn/alert); state is
      never color alone.
- [ ] Typography: serif for titles only, mono for all data values, `tabular-nums` only in
      aligned columns/ticks; the pulse ledger is the single signature flourish.
- [ ] Settings drawer works; admin token held in React state only (verify: not in
      localStorage/sessionStorage/cookies after entry); the three 401 codes each show their
      specific guidance.
- [ ] Keyboard: visible focus (`--seal` outline) on links, rows, marks, and controls;
      `prefers-reduced-motion` disables the two transitions.
- [ ] Responsive to 360 px: nav rail collapses to a top bar; tables gain horizontal scroll
      within their card (`overflow-x: auto`), never page-level horizontal scroll.
