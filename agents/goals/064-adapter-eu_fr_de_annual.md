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
- [ ] regime doc  - [ ] fixtures  - [ ] expected (human)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human)
- expected.*.json completion
