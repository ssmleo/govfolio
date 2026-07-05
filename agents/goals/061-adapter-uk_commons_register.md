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
- [x] regime doc (2026-07-05, docs/regimes/uk_commons_register.md + evidence, spec leg)  - [x] fixtures (2026-07-05, 061b: 5 cases, 5/5 sha = doc pins)  - [x] expected (2026-07-05, 061b test-designer: two independent passes + mechanical check, zero divergences; publishes `unverified` per policy)  - [x] discover (2026-07-05, 061c: two date-windowed sweeps PublishedFrom/UpdatedFrom, Take=20 pagination w/ bounded totalResults-restart, version-qualified `{id}@{version}` FilingRefs)  - [x] fetch (2026-07-05, 061c: GET /Interests/{id} → Bronze sha256; politeness concurrency 1 / ≥2s / identified UA+contact)  - [x] parse (2026-07-05, 061c: pure serde_json, deny_unknown_fields drift gate, §3.8 rejects, §6.2 confidence)  - [x] normalize (2026-07-05, 061c: §3.4 R1–R4 value rules, §3.5 owner, §3.6 dates, §5 details contract snapshot-committed)  - [x] green (2026-07-05, 061c: conformance 5/5; us_house 5/5 / us_senate 4/4 / fixture_fake 1/1 no regression)

## Runner-binding / live-fetch follow-ups (061c, us_senate precedent)
- Persistent published/updated high-water marks for the §2.3 sweeps (adapter
  currently anchors the 30-day overlap window on "now").
- §3.8 check 4: re-emit the FilingRef under the observed version when
  `updatedDates` grew between discover and fetch (needs the runner's
  FilingRef→fetch threading; parse-side id threading is in).
- Version-qualified filing publish (`{id}@{version}`, `supersedes_filing_id`
  to `{id}@{version-1}`) + `uk_interest_update_unlinked` review routing at
  publish (§2.5/§3.7) — conformance doesn't publish.
- `From:` header on live requests (regime doc pairs it with the identified
  UA; the shared PoliteClient carries the contact in the UA string itself —
  extend PoliteClient or the runner's client when the runner binding lands).
- MNIS roster seeding for §2.4 politician resolution (exact id join;
  conformance uses the MANIFEST-pinned ULID table, pool mode emits unbound
  identity for the publish stage to bind — us_house Task-9 pattern).

## BLOCKED (human) — SUPERSEDED 2026-07-05 per docs/decisions/automation-policy.md
- ~~expected.*.json completion~~ → test-designer authors expecteds (high-confidence
  extraction + second-model cross-check); records publish `unverified` and flow to the
  sampling-audit queue. No human gate (same ruling as goals 001/060).
