# 061 — adapter: uk_commons_register

## Objective
Ship the UK Register of Members' Financial Interests (categorical interests) adapter to conformance-green, following the adapter template.

## Template steps (design §5.1, plan Task 8)
1. Write docs/regimes/uk_commons_register.md (source URLs, cadence, precision, quirks) — it is your context AND the public methodology page draft.
2. tools/capture-fixture.ts ×3+ real filings.
3. HUMAN completes expected.silver.json / expected.gold.json.
4. TDD adapter until conformance green; politeness config mandatory.

## Acceptance criteria
```bash
cargo run -p pipeline --bin conformance -- uk_commons_register
```

## Checklist
- [x] regime doc (2026-07-05, docs/regimes/uk_commons_register.md + evidence, spec leg)  - [ ] fixtures  - [ ] expected (test-designer, see below)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human) — SUPERSEDED 2026-07-05 per docs/decisions/automation-policy.md
- ~~expected.*.json completion~~ → test-designer authors expecteds (high-confidence
  extraction + second-model cross-check); records publish `unverified` and flow to the
  sampling-audit queue. No human gate (same ruling as goals 001/060).
