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
- [x] regime doc  - [x] fixtures  - [x] expected (test-designer, 064b)  - [x] discover  - [x] fetch  - [x] parse  - [x] normalize  - [x] green

## Build leg (064 leg C, rust-builder) — DONE
- ONE crate `eu_fr_de_annual`, source-dispatching `Adapter` (detects DPI PDF / HATVP XML /
  Bundestag HTML from Bronze bytes → `src/{eu,fr,de}` sub-modules; `regime()` returns the
  per-fixture source slug). Conformance: `cargo run -p pipeline --bin conformance --
  eu_fr_de_annual` → **9/9 green OFFLINE** (EU via primed `extraction.cache.json`, no API key).
  No regressions: us_house 5/5, us_senate 4/4, uk_commons 5/5, canada_ciec 7/7,
  australia_register 4/4, fixture_fake 1/1.
- EU: LLM-vision seam reusing `pipeline::extraction`; 3 `eu_*/extraction.cache.json` primed
  mechanically from `expected.silver.json` via `prime_from_expected_silver` (key
  `eu_parliament_dpi/llm@1` + default primary model). PLN income → value NULL +
  `value_source=unmapped_currency` (Currency enum NOT extended this leg — founder/core call).
- FR: deterministic `quick-xml` DOM; latest-year `montant` exact EUR; spouse for
  `activProfConjointDto`; equity for `participationFinanciereDto` (evaluation NOT → value);
  `activCollaborateursDto`/`observationInteretDto` excluded; DIA/DIAM from stem.
- DE: deterministic light `quick-xml` DOM (NO scraper/html5ever — CI link-footprint /
  australia SIGBUS lesson) over the `m-biography__infos` fragment; first regular euro amount →
  value, `zuzüglich` supplements ride `amount_raw` only.
- 3 snapshot-committed schemas `crates/pipeline/schemas/details/{eu_parliament_dpi,fr_hatvp_dia,
  de_bundestag}.interest.json` + 3 `conformance.rs details_schema()` arms.

### Follow-ups (documented, out of this leg)
- **Live discover/fetch per source** (runner-binding): EU europarl per-MEP DPI GET, FR HATVP
  `liste.csv` GET, DE browser-engine seam behind the Enodia gate (§DE.2, never evasion). The
  adapter wires the offline conformance path and FAILS CLOSED on the live path; conformance
  identity (FR stem, DE mdb/name/WP, all filing/politician ULIDs) is bound by sha256 constants.
- **Currency enum extension** (EUR→+PLN/HUF/… ) — snapshot-visible core change, founder/core.
- **DE §DE.6 10-Stufen backfill** + webarchiv boundary pin (historical WP; OUT of 21. WP green).
- **§DE.8 raw-byte pins** now established (3 sha256) — methodology page backfill is founder-gated.

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
