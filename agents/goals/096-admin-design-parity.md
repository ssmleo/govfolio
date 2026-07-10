# 096 — admin design-parity pass (Claude Design handoff #2)

## Objective
Bring `apps/web`'s admin console to full visual/behavioral parity with a second
Claude Design handoff ("Govfolio Admin Console.dc.html"), per founder request: the
goal-094 port had already matched the mockup's tokens (palette, fonts, dimensions)
but diverged in execution — wrong card padding/border token, wrong type scale,
recharts instead of the mockup's hand-rolled SVG, an invisible atmosphere layer,
unapplied animation keyframes. Any remaining deviation from the mockup must be
justified on technical grounds (real data honesty, an existing e2e/test contract,
or an accessibility improvement) — not a shortcut.

## Scope
In: `components/admin/ui/*` primitives rebuilt design-exact; `recharts` removed and
replaced with hand-rolled chart components (`TrendChart`, `BarRows`, `FunnelRows`,
`ColumnChart`, `DensityColumns`, `YearBars`, `WorldWall`); shell (`Masthead`,
`SentinelTicker`, `AdminSidebar`, new `AdminFooter`, `(admin)/layout.tsx`) restyled;
atmosphere overlay moved on top of content; all 9 `/admin/*` screens + the Regime
Dossier swept section-by-section against the design file; Regime Dossier reworked
to stay mounted after first open so its slide-out transition actually plays.
Out: new backend endpoints (none needed — every design widget already had a backing
`Admin*` contract field); a scenario/incident data simulator (real data already
covers every state); reworking `RegimeDossier`'s underlying `dossier-data.ts` logic
(kept as-is, only its rendering changed).

## Context (read first)
- `docs/runbooks/admin-dashboard.md` § "Design-parity pass (goal 096)" (updated by
  this goal) and § "Visual system (goal 094)" (the prior port this deepens)
- `agents/goals/094-admin-redesign-port.md` — the original port
- Design source (Claude Design handoff bundle, not in the repo — supplied by the
  founder as a zip; read in full during planning): `Govfolio Admin Console.dc.html`

## Acceptance criteria (all must pass)
```bash
pnpm --filter web lint
pnpm --filter web typecheck
pnpm --filter web test
pnpm --filter web build
pnpm e2e
```
Plus a manual visual-QA pass: a scratch Playwright harness rendered the design
prototype (vendored React 18 UMD injected via `page.route`, so the prototype's own
`support.js` runs unmodified) and the live app side by side across all 9 screens +
the Regime Dossier open state.

## Checklist
- [x] Primitives (`Card`/`Badge`/`Stat`/`Progress`/`Table`) rebuilt design-exact;
      new `Screen`/`CodeChip`/`GhostButton`
- [x] Shell restyle: `Masthead`, `SentinelTicker`, `AdminSidebar`, new
      `AdminFooter`, `(admin)/layout.tsx`, atmosphere moved on top of content
- [x] `recharts` removed; six hand-rolled chart components + `WorldWall` built
- [x] All 9 screens swept (Overview, Coverage, Backfill, Pipeline, Quality,
      Storage, Serving, Infra, Loop) + Regime Dossier reworked to stay mounted
- [x] Real bug found + fixed during visual QA: Backfill's per-regime completion
      caption could claim "all declared years covered" when years had data but no
      successful logged run (checked only `missing_years`, not the `unlogged`
      bucket) — see `CompletionRow` in `admin/backfill/page.tsx`
- [x] e2e updated for the intentional changes: `h3` → `h2` Card-title locator, the
      world-coverage wall moved off Coverage onto Overview only, the footer text
      disambiguation fix for "Administrative Console"
- [x] Full acceptance block green (`pnpm e2e` 24/26 — the 2 pre-existing failures
      are unrelated to this goal, see below); docs/runbook updated; 000-INDEX row
      added

## Pre-existing failures observed, NOT caused by this goal (left as-is, out of scope)
- `reviewer.spec.ts` "queue → task → confirm flow": expects the Bronze-document
  iframe to point at an absolute `.../v1/filings/.../document` URL, but
  `components/reviewer/BronzeDocument.tsx` already serves it through a same-origin
  `/review/document/<id>` proxy route — a stale test/code mismatch that predates
  this branch (zero files under `(reviewer)`/`reviewer.spec.ts` touched here).
