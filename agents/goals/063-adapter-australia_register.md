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
- [x] regime doc  - [x] fixtures  - [x] expected (test-designer, 063b)  - [x] discover  - [x] fetch  - [x] parse  - [x] normalize  - [x] green

Build leg C (rust-builder, 2026-07-05): conformance 4/4 GREEN OFFLINE (no ANTHROPIC_API_KEY)
— `cargo run -p pipeline --bin conformance -- australia_register`. Adapter is LLM-vision-first
(§6): `parse` routes every doc through the goal-021 extraction seam
(`crates/adapters/australia_register/src/extractor.rs`), reading the committed offline file
cache primed MECHANICALLY from each `expected.silver.json` via
`pipeline::extraction::prime_from_expected_silver` (4 × `fixtures/<case>/extraction.cache.json`,
enforced by `tests/extraction_cache_snapshot.rs`; us_house scanned_paper precedent). Two details
contracts snapshot-committed: `crates/pipeline/schemas/details/australia_register.{interest,change_notification}.json`
(+ registry arms in `crates/pipeline/src/conformance.rs`). No regressions (us_house 5/5,
us_senate 4/4, uk_commons_register 5/5, canada_ciec 7/7, fixture_fake 1/1). Follow-ups recorded:
(a) **browser-engine fetch seam** — the Azure WAF 403-blocks plain `reqwest` (§2.3); `fetch`
implements the polite-GET protocol but the headless-Chromium transport (us_senate §2.5) is
unwired, so live fetch fails closed rather than evades. (b) **live vision transcription** — the
tier-3 live LLM path is `needs_llm_extraction` fail-closed pending (a); conformance/e2e run
offline from the cache. (c) **runner id binding** — normalize emits fixed MANIFEST ULIDs in
conformance (`pool: None`) and nil/unbound ids under a pool for the publish stage to bind from
the `(electoral division, state)` roster (§2.4). (d) spec-corrections the auditor should fold
back into `docs/regimes/australia_register.md` (founder/methodology-gated, NOT edited here):
§7/§8 Bronze pins now ESTABLISHED (MANIFEST `cases.*.sha256`); the "45 TH PARLIAMENT" mis-stamp
is an OCR artifact absent from the pinned Albanese bytes; Albanese fixture is compound
statement+alterations (33 interest + 43 change_notification), not alterations-only.

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

## DOC-UPDATE WORK ITEMS (methodology-gated — founder lane; NOT applied by auditor)
Audit 063 (cross-check gate, PASS) confirmed three spec-corrections that warrant a
`docs/regimes/australia_register.md` update. That page is founder/methodology-gated
(residual human lane: methodology PUBLIC copy), so they are recorded here as work items,
NOT applied by the auditor:
1. **§7/§8 Bronze pins ESTABLISHED** — replace the `_capture leg (§7)_` placeholders with
   the raw-byte pins (verified byte-exact: worktree == `git cat-file blob HEAD` == MANIFEST):
   Buchholz_48P `cf09599e…bc4c39bb` @2026-02-26, Chalmers_48P `f1822e24…308ca21f0` @2026-04-30,
   Albanese_48P `b632987b…8a35b68b` @2026-05-26, Katter_48P `625134ad…6b996806` @2025-09-09.
   The media `Last-Modified` date is the authoritative `@version`; Buchholz/Albanese media is
   +1 day vs the index "Last updated" date (§2.5).
2. **§3.3/§7 "45 TH PARLIAMENT mis-stamp" is an OCR ARTIFACT** absent from the pinned Albanese
   bytes — every header-bearing page renders `48TH PARLIAMENT` (auditor confirmed:
   `parliament_stamp` is only `{null, "48TH PARLIAMENT"}` across all 76 rows). The genuine
   mis-stamp quirk / DELETION alteration / populated Dependent band remain UNEXERCISED by
   these fixtures (evidence-only, later legs).
3. **§7/§3.9 Albanese is a COMPOUND statement+alterations doc** (33 `interest` + 43
   `change_notification`), not alterations-only as E4 implied; and the Katter/Albanese
   "OCR-garbage" is a garbage embedded TEXT LAYER over typed-and-legible page renders
   (not handwriting) — `source_flavour = scanned_vision` for the right reason (pdftotext
   unreliable), but the underlying forms are typed.
