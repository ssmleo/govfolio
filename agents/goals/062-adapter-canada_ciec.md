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
- [x] regime doc (2026-07-05, spec leg: docs/regimes/canada_ciec.md + evidence archived same commit under docs/regimes/canada_ciec/evidence/ — 45 pages + retrieval log; fixture pins ×7 + pinning rule in its §7; v1 scope = 10 financial-substance declaration types × 3 politician roles, record_type interest + change_notification, value NULL always)  - [x] fixtures (2026-07-05, leg B: 7 conformance cases under crates/adapters/canada_ciec/fixtures/, byte-pinned input.html + MANIFEST; scaffold crate keeps `cargo test --workspace` green)  - [x] expected (2026-07-05, leg B: expected.silver/gold.json authored independently — deterministic re-derivation + fresh-context second-model cross-check, zero divergences, per automation-policy; publishes `unverified`)  - [x] discover (2026-07-05, leg C: role-scoped windowed `/cards` sweeps, `a[href*=declarationId=]` extraction + dedup; FOLLOW-UP: persistent high-water-mark windowing, client-side declaration-type filtering by card badge, and clientId→roster politician resolution are deferred to the runner binding — out-of-scope types fail closed at parse)  - [x] fetch (2026-07-05, leg C: EN details GET → Bronze sha256; no ETag/cookies so no conditional-GET)  - [x] parse (2026-07-05, leg C: scraper grammar A/B/C with the LOAD-BEARING `<br>`→space rule before whitespace-collapse; §3.10 integrity rejects; §6.2 confidence scoring incl. the 0.98 OCIEC-translation literal)  - [x] normalize (2026-07-05, leg C: record_type interest/change_notification, value NULL always, asset_class other, §3.5 owner map incl. family-C spouse/self, two V1 details contracts + committed schemas)  - [x] green (2026-07-05, leg C: `conformance canada_ciec` 7/7; no regression us_house 5/5 · us_senate 4/4 · uk_commons_register 5/5 · fixture_fake 1/1; `cargo test --workspace` 263 passed + 44 ignored(sqlx) green; clippy -D warnings + fmt + role_evals + openapi-drift all clean. FOLLOW-UP: live discover/fetch never exercised against the network — runner binding + a card fixture pending)

## BLOCKED (human)
- ~~expected.*.json completion~~ SUPERSEDED 2026-07-05 by docs/decisions/automation-policy.md
  ("FIXTURE expected outputs … auto-resolved"): test-designer authors expected.silver.json /
  expected.gold.json independently (high-confidence extraction + second-model cross-check);
  records publish `unverified` and flow to the sampling-audit queue. Step 3 of the template
  above reads accordingly. No human gate remains on this goal.
- NOTE (not a gate): the source is an officially TEMPORARY website in phased transition to
  ethicscanada.ca (docs/regimes/canada_ciec.md, E40) — expect layout drift; sentinel priority.
