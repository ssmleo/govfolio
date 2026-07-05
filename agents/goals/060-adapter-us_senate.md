# 060 — adapter: us_senate

## Objective
Ship the US Senate eFD PTRs (session dance, HTML tables) adapter to conformance-green, following the adapter template.

## Template steps (design §5.1, plan Task 8)
1. Write docs/regimes/us_senate.md (source URLs, cadence, precision, quirks) — it is your context AND the public methodology page draft.
2. tools/capture-fixture.ts ×3+ real filings.
3. HUMAN completes expected.silver.json / expected.gold.json.
4. TDD adapter until conformance green; politeness config mandatory.

## Acceptance criteria
```bash
cargo run -p pipeline --bin conformance -- us_senate
```

## Checklist
- [x] regime doc (2026-07-05, spec leg: docs/regimes/us_senate.md + evidence archived same commit under docs/regimes/us_senate/evidence/; fixture pins + pinning rule in its §7)  - [ ] fixtures  - [ ] expected (auto, see below)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

## BLOCKED (human)
- ~~expected.*.json completion~~ SUPERSEDED 2026-07-05 by docs/decisions/automation-policy.md
  ("FIXTURE expected outputs … auto-resolved"): test-designer authors expected.silver.json /
  expected.gold.json independently (high-confidence extraction + second-model cross-check);
  records publish `unverified` and flow to the sampling-audit queue. Step 3 of the template
  above reads accordingly. No human gate remains on this goal.
- NOTE (fetch design, not a gate): eFD's bot manager 403s non-browser TLS fingerprints on
  view-page GETs — see docs/regimes/us_senate.md §2.5 before building `fetch`.
