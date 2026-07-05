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
cargo run -p pipeline --bin conformance -- canada_ciec
```

## Checklist
- [x] regime doc (2026-07-05, spec leg: docs/regimes/canada_ciec.md + evidence archived same commit under docs/regimes/canada_ciec/evidence/ — 45 pages + retrieval log; fixture pins ×7 + pinning rule in its §7; v1 scope = 10 financial-substance declaration types × 3 politician roles, record_type interest + change_notification, value NULL always)  - [x] fixtures (2026-07-05, leg B: 7 conformance cases under crates/adapters/canada_ciec/fixtures/, byte-pinned input.html + MANIFEST; scaffold crate keeps `cargo test --workspace` green)  - [x] expected (2026-07-05, leg B: expected.silver/gold.json authored independently — deterministic re-derivation + fresh-context second-model cross-check, zero divergences, per automation-policy; publishes `unverified`)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human)
- ~~expected.*.json completion~~ SUPERSEDED 2026-07-05 by docs/decisions/automation-policy.md
  ("FIXTURE expected outputs … auto-resolved"): test-designer authors expected.silver.json /
  expected.gold.json independently (high-confidence extraction + second-model cross-check);
  records publish `unverified` and flow to the sampling-audit queue. Step 3 of the template
  above reads accordingly. No human gate remains on this goal.
- NOTE (not a gate): the source is an officially TEMPORARY website in phased transition to
  ethicscanada.ca (docs/regimes/canada_ciec.md, E40) — expect layout drift; sentinel priority.
