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
- [x] regime doc (2026-07-05, spec leg: docs/regimes/us_senate.md + evidence archived same commit under docs/regimes/us_senate/evidence/; fixture pins + pinning rule in its §7)  - [x] fixtures (2026-07-05, test-designer leg B: 4 cases under crates/adapters/us_senate/fixtures/, all §7 pins re-verified byte-identical at capture; minimal us_senate crate scaffolded in the same commit — workspace glob)  - [x] expected (2026-07-05, leg B: parser-blind two-pass derivation per automation policy; conventions + ULID constants in fixtures/MANIFEST.json)  - [x] discover (2026-07-05, leg C: §2.1 agreement dance + §2.2 date-windowed DataTables POST, re-dance rule, senators-only v1; polite POST + cookie store added to pipeline PoliteClient)  - [x] fetch (2026-07-05, leg C: view GET → Bronze sha256; §2.5 TLS limitation documented in-code — see work item below)  - [x] parse (2026-07-05, leg C: scraper/html5ever, §3.7 integrity rejects, `--` sentinel, entity decoding, §6.2 scoring; LLM seam stub freezes paper/rejects as needs_llm_extraction)  - [x] normalize (2026-07-05, leg C: §3.2–§3.5 maps, amendment_number from title, supersedes NULL per §3.6, details schema snapshot at crates/pipeline/schemas/details/us_senate.transaction.json)  - [x] green (2026-07-05, leg C: conformance us_senate 4/4; us_house 5/5 + fixture_fake 1/1 unregressed)

## Follow-up work items (recorded, not gates for this goal)
- **Fetch TLS engine (§2.5):** live view GETs through plain `reqwest` are 403-bound
  (Akamai bot manager gates non-browser TLS fingerprints — probe matrix E13). The
  protocol is implemented faithfully; before live fetch runs, probe `wreq`-style
  browser-impersonation TLS from Rust, else fall back to a headless-browser sidecar
  (`chromiumoxide`). Record the flip SAF-first in docs/regimes/us_senate.md §2.5 with
  probe evidence; scope the `From:` identification header to senate.gov hosts
  (MANIFEST `fetch_client` capture addendum). No fingerprint-evasion escalation
  beyond the documented client.
- **Runner binding (us_house Task-9 pattern):** `stg_us_senate` DDL + RunnerBinding
  (filing identity from silver rows, `ptr_amendment_unlinked` review reasons on
  `details.amendment_number`, report_uuid high-water discovery threading).

## BLOCKED (human)
- ~~expected.*.json completion~~ SUPERSEDED 2026-07-05 by docs/decisions/automation-policy.md
  ("FIXTURE expected outputs … auto-resolved"): test-designer authors expected.silver.json /
  expected.gold.json independently (high-confidence extraction + second-model cross-check);
  records publish `unverified` and flow to the sampling-audit queue. Step 3 of the template
  above reads accordingly. No human gate remains on this goal.
- NOTE (fetch design, not a gate): eFD's bot manager 403s non-browser TLS fingerprints on
  view-page GETs — see docs/regimes/us_senate.md §2.5 before building `fetch`.
