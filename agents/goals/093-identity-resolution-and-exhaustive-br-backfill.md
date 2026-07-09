# 093 — cross-time/cross-body politician-identity resolution + exhaustive br historical backfill

## Objective
Fix the shared `resolve_politician` mechanism so two different real people sharing
`(alias, district, body)` — same-pass or cross-time — no longer silently collapse onto one
`politician` row (JULIO CESAR DOS SANTOS + CARLOS ALBERTO DE SOUZA, both confirmed live in
`br`), then (Phase 2, after Phase 1 is green) walk `br`'s remaining historical years as
exhaustively as the real source data allows.

## Scope
In:
- **Phase 1** (foundational, every regime, not just `br`): survey every live/near-live
  regime for a durable per-filer id in raw source data; add a universal nullable
  `politician.external_identifier` column + id-aware/year-window disambiguation to
  `pipeline::stages::roster::resolve_hits`; wire `br`'s CPF/voter-title into it; zero
  behavior change for regimes without an id (`us_house`/`us_senate`); retroactively
  re-split CARLOS ALBERTO DE SOUZA using the same template as the already-fixed JULIO
  CESAR case; full workspace green (fmt/clippy/test, every adapter's conformance).
- **Phase 2** (only after Phase 1 green): every `br` general-election year still
  unwritten, working backward from 2010, probing full CKAN/alternate resource lists
  before ruling a year out, building whatever schema variants the real record requires.

Out (this goal): wiring a live `RunnerBinding` for `uk_commons_register`/`canada_ciec`/
`eu_fr_de_annual`/`australia_register` (their durable ids — MNIS `member.id`, `clientId`,
`mep_id`/`mdb_id`/`id_origine` — are surveyed and already flow into Silver/`details`, but
none of these four regimes calls `roster.rs` in production yet; that's separate,
per-regime follow-up work). Any code change unrelated to identity resolution or the br
backfill.

## Context (read first)
- `docs/decisions/politician-identity-resolution-design.md` — the Phase 1 design (survey
  table, schema, `resolve_hits` algorithm, threshold justification, CARLOS ALBERTO
  decision), written and executed this session.
- `docs/decisions/br-identity-collision-remediation.md` — the original JULIO CESAR plan;
  Phase 1's design and the CARLOS ALBERTO fix bin both mirror it directly.
- `docs/regimes/br/AUTHORITY.md` Quirks log — CPF/voter-title masking history, 2006/2010
  schema-fork finding, 2002-and-earlier no-bem_candidato-data finding (Phase 2 boundary).
- `agents/goals/092-br-historical-backfill-extension.md` — the prior pass that found the
  CARLOS ALBERTO collision and left it flagged, not fixed.
- `crates/pipeline/src/stages/roster.rs` — the fixed mechanism.
- `crates/worker/src/bin/fix-br-carlos-alberto-souza-sp.rs` — the executed one-off fix.

## Acceptance criteria (all must pass)
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
cargo run -p pipeline --bin conformance -- br   # and every other adapter, all green
DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test --workspace -- --ignored
cargo run -p worker --bin check-br-identity-collisions   # PASS: zero
```

## Checklist
- [x] Phase 1 — survey 5 regimes (us_house/us_senate, uk_commons_register, canada_ciec,
      australia_register, eu_fr_de_annual) for durable filer ids; wrote
      `politician-identity-resolution-design.md`
- [x] Phase 1 — migration `0013_politician_external_identifier.sql` (expand-only)
- [x] Phase 1 — `resolve_hits`/`resolve_politician`/`seed_roster` id-aware + year-window
      disambiguation (`crates/pipeline/src/stages/roster.rs`); `FilingIdentity`/
      `RosterMember` gain `external_identifier`; `br` populates from CPF (sentinel `-4`
      aware) falling back to voter-title; `us_house`/`us_senate` unaffected (proven by
      unchanged `roster_historical.rs`)
- [x] Phase 1 — new regression suite `crates/pipeline/tests/roster_identity.rs` (JULIO
      CESAR same-pass shape, CARLOS ALBERTO cross-pass shape, legacy-row backward
      compat, implausible-gap fail-closed, plausible-gap-with-no-id honest limitation)
- [x] Phase 1 — full workspace green: fmt/clippy/test, all 8 adapters' conformance,
      full `--ignored` DB-gated suite (incidentally fixed 2 pre-existing unrelated
      breakages blocking the gate: `crates/core/tests/migrate.rs`'s stale migration-count
      assertion, off by the already-landed 0012 migration; left an in-progress, unrelated
      goal-081 WIP file — `crates/worker/src/migrate_local_to_prod.rs` + its bin/test —
      untouched/unfixed beyond what was needed to unblock the shared build, since it
      belongs to different in-flight work)
- [x] Phase 1 — CARLOS ALBERTO DE SOUZA retroactively re-split: dry-run reviewed, executed
      (`fix-br-carlos-alberto-souza-sp.rs --execute`), independently re-verified via
      `check-br-identity-collisions` (PASS, zero), idempotent re-run confirmed (no-op).
      New politician `01KX3P9PVZK386AQPPMDD622QT` (CPF `09867774809`, 2022 filing, 3
      records); old politician `01KWXA32E7PMQ6D7CBEZJWCA9F` keeps CPF `29168317972`
      (2014 filing, 8 records) untouched.
- [x] Phase 2 — 2010: built the 2006/2010 `bem_candidato` legacy-schema variant
      (`#[serde(alias=...)]` in `crates/adapters/br/src/parse.rs`, no version dispatch
      needed, proven against all 81,050 real 2010 rows); real write 6245 filings
      published / 26678 new Gold rows / 42 failed closed; idempotency re-run confirmed.
      check-br-identity-collisions surfaced 3 NEW collisions — traced to a real Phase 1
      gap (pre-existing politicians all had `external_identifier = NULL`, so a new year
      against the existing roster always hit the weak fallback) — HALTed back into
      Phase 1 per goal instruction: built `backfill-br-external-identifiers.rs`
      (retroactively populated 16,399 safe politicians) + generalized the JULIO
      CESAR/CARLOS ALBERTO pattern into `fix-br-cpf-collision.rs`; all 3 fixed,
      independently re-verified PASS. Also confirmed 1994/1998/2002's FULL CKAN resource
      lists exhaustively (3 resources each, no asset-shaped resource under any name).
- [ ] Phase 2 — 2006: run the now-working legacy schema + id-aware defenses
- [ ] Phase 2 — years before 2006: exhaustive resource-list probing per invariant 12
      (bem_candidato-equivalent data confirmed absent 1994-2002; further back TBD)

## BLOCKED (human)
(empty)
