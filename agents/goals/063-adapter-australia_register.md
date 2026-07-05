# 063 — adapter: australia_register

## Objective
Ship the Australia Register of Members' Interests (28-day change notifications) adapter to conformance-green, following the adapter template.

## Template steps (design §5.1, plan Task 8)
1. Write docs/regimes/australia_register.md (source URLs, cadence, precision, quirks) — it is your context AND the public methodology page draft.
2. tools/capture-fixture.ts ×3+ real filings.
3. HUMAN completes expected.silver.json / expected.gold.json.
4. TDD adapter until conformance green; politeness config mandatory.

## Acceptance criteria
```bash
cargo run -p pipeline --bin conformance -- australia_register
```

## Checklist
- [x] regime doc  - [x] fixtures  - [x] expected (test-designer, 063b)  - [ ] discover  - [ ] fetch  - [ ] parse  - [ ] normalize  - [ ] green

regime doc: `docs/regimes/australia_register.md` (063 leg A, spec-writer, 2026-07-05).
Scoped to the House of Representatives register (per-member scanned PDFs); Senate register
deferred (separate committee/model). record_type = interest (initial statement) +
change_notification (alterations), both in one compound member PDF. value NULL always
(descriptive register). Extraction = LLM-vision-first (scanned/handwritten forms; goal 021
seam). Fetch = browser-engine seam required (host is Azure-WAF-gated). Fixture raw-byte
sha256 pins deferred to the capture leg via the seam (WAF-gated from spec env; §7 rule).

## BLOCKED (human)
- ~~expected.*.json completion~~ — SUPERSEDED per automation policy
  (`docs/decisions/automation-policy.md`): test-designer authors expecteds independently
  (schema-constrained vision extraction + second-model cross-check), records publish
  `unverified`, sampling-audit queue. No human gate.
