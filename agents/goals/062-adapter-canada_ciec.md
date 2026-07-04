# 062 — adapter: canada_ciec

## Objective
Ship the Canada CIEC public registry (change notifications) adapter to conformance-green, following the adapter template.

## Template steps (design §5.1, plan Task 8)
1. Write docs/regimes/canada_ciec.md (source URLs, cadence, precision, quirks) — it is your context AND the public methodology page draft.
2. tools/capture-fixture.ts ×3+ real filings.
3. HUMAN completes expected.silver.json / expected.gold.json.
4. TDD adapter until conformance green; politeness config mandatory.

## Acceptance criteria
```bash
pnpm conformance --filter adapters/canada_ciec
```

## Checklist
- [ ] regime doc  - [ ] fixtures  - [ ] expected (human)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human)
- expected.*.json completion
