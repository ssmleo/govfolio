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
- [x] T1 workspace  - [x] T2 CI  - [x] T3 migrations  - [x] T4 domain  - [x] T5 DDL  - [x] T6 fingerprint  - [x] T7 conformance  - [ ] T8 us_house  - [x] T9 pipeline  - [ ] T10 /v1  - [ ] T11 promote

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
- [x] T8d adversarial cross-check pass (auditor; policy's second-model check) — PASS
  - Ground truth re-derived 12/12 rows across all 4 fixtures via an independent third
    extraction path (visual PDF render, distinct from pdftotext and pdf-extract):
    zero substantive mismatches in silver or gold (asset text, bands, dates, owner,
    side, sub-lines, confidence arithmetic all confirmed).
  - Integrity: fixtures touched by exactly one commit (8a6c9a6), which predates the
    adapter (5681073) — expected.*.json provably not regenerated from parser output;
    all 4 PDFs re-hash to the §7 pins.
  - Invariant sweep clean (2, 3, 6, 7, 8, 10); acceptance re-run independently:
    conformance 4/4, fmt/clippy/test green, ignored sqlx suites green on PG 5433.
  - Adjudication: details type placement ruled adapter-local (regime doc §5 updated);
    design §4.3's core/src/schemas wording superseded by §5.1 + §9 for regime types.
  - Pelosi UBER `D :` "strike price of $50": verbatim-faithful — the PDF itself
    repeats the INTC description on the UBER row (filer copy-paste, not transcription).
  - Non-blocking notes: regime doc §3.6 says "46-code legend" but buckets list 48
    codes (E2 snapshot not committed, so count unverifiable); E2/E3/E9/E10 evidence
    snapshots never landed under docs/regimes/us-house/evidence/ despite the §8
    same-PR note (only the index slice did) — hygiene gap for a future pass, pins
    remain sha256-re-verifiable.

## T9 progress (2026-07-04)
- [x] T9 local pipeline runner, end-to-end idempotent (rust-builder)
  - pipeline: `run.rs` (Runner + RunnerBinding seam; §5.1 adapter trait untouched) +
    `stages/{pipeline_run,ingest,seed,roster,publish}.rs`; migration 0002
    (stg_us_house + stg_meta, expand-only, guardrail green); worker `local` bin
    (offline, fixtures + evidence-slice roster, no network).
  - T8c seam closed: us_house normalize pool=Some emits unbound (nil-ULID) identity;
    publish binds roster-resolved politician + (regime_id, external_id)-deduped filing,
    computes fingerprint via core::fingerprint, writes Gold unverified + outbox_event
    SAME TXN + §3.7 amendment tasks; unresolved filer = review_task, no Gold row.
  - Evidence: e2e_local 3/3 green on PG (second run inserts nothing across all 15
    tables; forced publish replay inserts nothing; publish rollback atomic).
    Local bin run 1: 4 published / 12 gold / 12 outbox / 1 review task;
    run 2: 4 replayed / 0 inserted. Conformance still 4/4; fmt/clippy/test green.

## Environment note (2026-07-04)
Host has no Docker/admin: acceptance line `docker compose up -d` is satisfied by portable
PG 16.14 at localhost:5433 (same DATABASE_URL as .env.example) locally; CI runs the real
postgres:16 service. docker-compose.yml remains authoritative for other environments.
