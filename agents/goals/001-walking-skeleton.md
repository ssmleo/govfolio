# 001 — walking skeleton (M0–M1)

## Objective
Execute Tasks 1–11 of docs/plans/2026-07-04-govfolio-implementation.md exactly as written.

## Acceptance criteria
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
cargo run -p pipeline --bin conformance -- us_house
docker compose up -d && cargo test --workspace -- --ignored
```

## Checklist
- [x] T1 workspace  - [x] T2 CI  - [x] T3 migrations  - [x] T4 domain  - [x] T5 DDL  - [x] T6 fingerprint  - [x] T7 conformance  - [ ] T8 us_house  - [ ] T9 pipeline  - [ ] T10 /v1  - [ ] T11 promote

## BLOCKED (human)
- ~~fixture expected.*.json completion is human ground truth (plan Task 8)~~
  SUPERSEDED 2026-07-04 by docs/decisions/automation-policy.md: expected.*.json is
  auto-resolved (high-confidence extraction + second-model cross-check; publishes
  unverified → sampling-audit queue). T8 proceeds without a human stop.

## T8 progress (2026-07-04)
- [x] T8a regime doc: docs/regimes/us-house.md (commit 1c62dcd; spec-writer, evidence-pinned)
  - 4 fixture DocIDs verified + sha256-pinned: 20020055 (typical), 20019182 (multi-row),
    20033759 (amendment), 20034836 (SP/options)
  - KEY CORRECTION vs plan: PTR amendments = FilingType P w/ new DocID (A = annual FD);
    no deterministic supersession link → NULL + review_task ptr_amendment_unlinked
- [x] T8b fixture capture + independent expected.*.json (test-designer; NOT from parser output)
  - 4 cases under crates/adapters/us_house/fixtures/ (typical_single_row, multi_row_sp_vehicle,
    amendment_unlinked, sp_owner_options); all sha256 pins re-verified, zero drift (index zip
    byte-identical to E1). 12 rows transcribed from PDFs via two independent passes (text layer
    + visual render), then mechanically cross-checked silver→gold against §3 tables.
  - Conformance conventions the builder must hit are in fixtures/MANIFEST.json: fixed ULID id
    constants (PDF carries no ids; pool=None in conformance), confidence JSON literal
    0.9800000190734863 (f64 image of f32 0.98 under serde_json's raw cast), silver payload =
    §4 fields minus confidence (wrapper-level), details optional fields as explicit nulls.
  - Index slice + retrieval metadata archived: docs/regimes/us-house/evidence/
- [x] T8c adapter implementation to conformance ×4 green (rust-builder)
  - crates/adapters/us_house: details contract (schemars, snapshot at
    crates/pipeline/schemas/details/us_house.transaction.json), text-layer state
    machine (NUL-stripped small-caps labels, date-pair row anchor, band-wrap join,
    page-2 header skip, vehicle-owner inheritance), §6 confidence scoring, LLM seam
    stub (Extractor trait, fail-closed), conformance ULID constants per MANIFEST.
  - pipeline: PoliteClient::get_conditional (ETag/Last-Modified), capture_fixture
    bin, details-schema registry arm; serde_json float_roundtrip (expected-float
    parse must be exact — see conformance-diffing skill learning).
  - Evidence: cargo run -p pipeline --bin conformance -- us_house → 4/4 green;
    fixture_fake still 1/1; fmt/clippy -D warnings/test --workspace green.
- [ ] T8d adversarial cross-check pass (auditor; policy's second-model check)

## Environment note (2026-07-04)
Host has no Docker/admin: acceptance line `docker compose up -d` is satisfied by portable
PG 16.14 at localhost:5433 (same DATABASE_URL as .env.example) locally; CI runs the real
postgres:16 service. docker-compose.yml remains authoritative for other environments.
