# 064 — adapter: eu_fr_de_annual

## Objective
Ship the EU Parliament DPI + France HATVP + Germany Bundestag (annual declarations; three sub-adapters) adapter to conformance-green, following the adapter template.

## Template steps (design §5.1, plan Task 8)
1. Write docs/regimes/eu_fr_de_annual.md (source URLs, cadence, precision, quirks) — it is your context AND the public methodology page draft.
2. tools/capture-fixture.ts ×3+ real filings.
3. HUMAN completes expected.silver.json / expected.gold.json.
4. TDD adapter until conformance green; politeness config mandatory.

## Acceptance criteria
```bash
cargo run -p pipeline --bin conformance -- eu_fr_de_annual
```

## Checklist
- [x] regime doc  - [x] fixtures  - [x] expected (test-designer, 064b)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human)
- ~~expected.*.json completion~~ SUPERSEDED per automation-policy (no human gate):
  test-designer authors expecteds independently (FR deterministic XML; EU vision +
  second-model cross-check; DE HTML via browser-engine seam), records publish
  `unverified`, sampling-audit queue. See docs/regimes/eu_fr_de_annual.md.

## Notes (064 leg A, spec — 2026-07-05)
- Architecture: ONE crate `eu_fr_de_annual`, THREE source sub-adapters (eu/fr/de), THREE
  disclosure_regime rows; conformance dispatches by the single name over source-namespaced
  fixtures `fixtures/{eu,fr,de}_<case>/`. Full rationale + contracts in the regime doc.
- Key findings: DE reformed to EXACT euro/cent (2021) — Stufe bands are historical/backfill;
  EU MEPs declare in national currency (Currency enum needs extension); FR patrimony (dsp)
  is legally un-republishable (LO 135-2) → out of scope. All fixtures pinned except DE
  (bot-gated, browser-engine seam, pins deferred to capture leg).
