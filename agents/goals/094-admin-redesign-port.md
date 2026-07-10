# 094 — admin redesign port (dark instrument-panel reskin + dossier)

## Objective
Port the "Govfolio admin redesign" Claude-Design mockup into apps/web: detach (admin)
into its own root layout, swap the light token theme for the mockup's dark palette,
replace the flat AdminNav with a grouped sidebar + masthead + Sentinel Ticker, and add
a real-data Regime Dossier slide-over — a visual/IA v2 of the already-shipped goal-091
board, no new backend functionality beyond (at most) one additive field.

## Scope
In: (site)/(admin) route-group split (mechanical file move); admin.css dark token
swap; next/font/google loading scoped to (admin); AdminNav -> AdminSidebar (5 groups,
letter chips, 1-9 keyboard shortcuts, localStorage last-screen); StatusStrip ->
Masthead + SentinelTicker; Card/Badge/Stat/Table/Progress token-driven restyle incl.
the deliberate Card shadow reversal; new RegimeDossier wired to real coverage/backfill/
pipeline/regime data; CPF-sweep client-side async wrapper; world-coverage tooltip
polish; optional additive `gold_records_estimate` on AdminOverview (see BLOCKED note).
Out: new IA/sections (none — same A-H taxonomy); auth/role model changes; the
`scenario` mock-data toggle (dropped — real data already reflects real state); mobile/
narrow-viewport layout (internal tool, desktop-only like today).

## Context (read first)
- docs/runbooks/admin-dashboard.md (to be updated by this goal)
- agents/goals/091-admin-observability-dashboard.md (the shipped board this reskins)
- Patterns to copy: apps/web/src/app/(admin)/**, apps/web/src/components/admin/**
- Next.js multiple-root-layouts + global-not-found.js (experimental,
  next@16.2.10): https://nextjs.org/docs/app/api-reference/file-conventions/not-found

## Acceptance criteria (all must pass)
```bash
pnpm --filter web lint
pnpm --filter web typecheck
pnpm --filter web test
pnpm --filter web build
pnpm e2e
```
If the optional AdminOverview.gold_records_estimate field is added, additionally:
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
cargo run -p api --bin openapi && git diff --exit-code packages/contracts/
```
Plus a manual visual-QA pass: Playwright screenshots of all 9 screens + Dossier-open
state, compared against the mockup's dark palette/typography/spacing (see Task 10).

## Checklist
- [x] Task 2: (site)/(admin) route-group split, full e2e green before any restyle
- [x] Task 3: admin.css dark token swap + Card shadow-reversal comment update
- [x] Task 4: font loading (next/font/google, scoped to (admin))
- [x] Task 5: Masthead + AdminSidebar (shortcuts, localStorage) + SentinelTicker
- [x] Task 6: Regime Dossier (data-wiring + component + Table.onRowClick)
- [x] Task 7: CPF-sweep async wrapper; world-coverage tooltip decision; atmosphere overlay
- [x] Task 8: new/updated unit tests
- [x] Task 9: e2e additions (shell, shortcuts, dossier, 404 split)
- [x] Task 10: docs/runbooks/admin-dashboard.md refresh + visual QA screenshots
- [x] Full acceptance block green; committed; checklist + 000-INDEX row updated

## BLOCKED (human)
(empty unless the gold_records_estimate field's count(*) vs. reltuples-estimate
tradeoff needs a founder call — see Data gaps section of the plan; default is to
proceed with the reltuples estimate without blocking)
