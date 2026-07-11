# Govfolio v1 Implementation Plan (Rust data plane + TS web)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
**Required skills:** skill:executing-plans
> Work on a branch. Human-only lanes: applying DB migrations to prod, `terraform apply`, public claim-making copy.
> Read `/CLAUDE.md` and the relevant `agents/goals/NNN-*.md` before each task. Repo is memory: update the goal's checklist and commit every iteration.

**Goal:** Ship govfolio v1 — worldwide politician-disclosure tracking with a free transparency layer and paid real-time alerts + API — per `docs/plans/2026-07-04-govfolio-design.md` (as amended: D7 hybrid stack).

**Architecture:** Rust data plane (`crates/*`: pipeline, adapters, worker, axum `/v1`) + TypeScript presentation edge (`apps/web`, Next.js). Bronze GCS → Silver staging → Gold Postgres; two-stage publication (`unverified → verified`); transactional-outbox alerts. The generated OpenAPI contract is the only door between the languages.

**Tech Stack:** Rust stable (tokio, axum, sqlx raw SQL, serde + schemars, utoipa, reqwest, rust_decimal, sha2, jsonschema, pdf-extract → pdfium-render if fidelity demands), TypeScript (Next.js, generated client via openapi-typescript), Postgres 16, Vitest/Playwright (web), GCP + Cloudflare + Terraform, GitHub Actions (cargo-chef + sccache), Stripe.

---

## Milestone map

| M | Name | Detail lives in |
|---|---|---|
| M0 | Repo bootstrap (cargo workspace, lint gates, CI, local Postgres, sqlx migrator) | **This doc — Tasks 1–3** |
| M1 | Walking skeleton (domain → DDL → fingerprint → adapter trait + conformance → `us_house` → local pipeline → `/v1/records`) | **This doc — Tasks 4–11** |
| M2 | Cloud substrate (Terraform; deploy skeleton) | `agents/goals/020` |
| M3 | Alerts (outbox dispatcher, email + HMAC webhooks, rules CRUD on shared grammar) | `agents/goals/030` |
| M4 | Website (pnpm bootstrap + generated client; SSR pages; sitemap) + reviewer UI | `agents/goals/040–041` |
| M5 | Productization (auth, keys, quotas → Stripe, 24h free-tier delay) | `agents/goals/050` |
| M6 | Coverage wave 1 + worldwide registry seed + coverage dashboard | `agents/goals/060–065` |
| M7 | Trust hardening (audits, corrections log, redaction, drift detection) | `agents/goals/070` |
| M8 | US backfill (→2012) + launch checklist | `agents/goals/080` |

Rule: a goal too big for one loop-session gets expanded into `docs/plans/<date>-<slug>.md` in this same task format first.

---

## M0 — Repo bootstrap

### Task 1: Cargo workspace + lint regime + smoke test

**Files:**
- Create: `Cargo.toml` (workspace), `rust-toolchain.toml` (pin stable), `rustfmt.toml`, `.gitignore`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Step 1: Write the failing test** — in `crates/core/src/lib.rs`:

```rust
pub fn hello() -> &'static str { "govfolio" }

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_smoke() { assert_eq!(super::hello(), "govfolio"); }
}
```
Start with the test referencing a not-yet-written `hello` to see red first, then add the fn.

**Step 2:** `cargo test -p core` → FAIL (unresolved `hello`).
**Step 3:** add the fn (above). Workspace `Cargo.toml` sets the lint law once:

```toml
[workspace]
members = ["crates/*", "crates/adapters/*"]
resolver = "2"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
pedantic = { level = "warn", priority = -1 }
```
(Tests opt out with `#[allow(clippy::unwrap_used)]` at module level — panics belong in tests only.)

**Step 4:** `cargo test -p core` → 1 passed; `cargo clippy --all-targets -- -D warnings` → clean; `cargo fmt --check` → clean.
**Step 5:** `git add -A && git commit -m "chore: cargo workspace, lint law (no unwrap), smoke test"`

### Task 2: CI gate

**Files:** `.github/workflows/ci.yml`

Jobs: (a) rust — fmt check, clippy `-D warnings`, `cargo test --workspace`, cached with `Swatinem/rust-cache` (cargo-chef in the deploy image later); (b) db — services: postgres:16, runs `cargo test --workspace -- --ignored` for `#[sqlx::test]` suites with `DATABASE_URL` set; (c) placeholder web job (activates in goal 040). Push branch → verify green → commit `chore: ci gates`.

### Task 3: Local Postgres + sqlx migrator

**Rationale:** design DDL stays authoritative as plain `.sql`; `sqlx::migrate!` gives ordering + checksums + embedding for free — boring and standard.

**Files:**
- Create: `docker-compose.yml` (postgres:16 on 5433, healthcheck), `.env.example` (`DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio`)
- Create: `crates/core/migrations/0000_init.sql` (`select 1;` marker)
- Create: `crates/core/src/db.rs` (`pub async fn migrate(pool) -> …` wrapping `sqlx::migrate!("./migrations")`), `crates/core/src/bin/migrate.rs` (CLI)
- Test: `crates/core/tests/migrate.rs`

**Step 1: failing test:**

```rust
#[sqlx::test(migrations = false)]
async fn migrator_is_idempotent(pool: sqlx::PgPool) {
    core::db::migrate(&pool).await.unwrap();
    core::db::migrate(&pool).await.unwrap(); // second run: no-op, no error
    let n: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}
```
**Step 2:** `docker compose up -d && cargo test -p core --test migrate` → FAIL (`db` module missing).
**Step 3:** implement `db.rs` + bin.
**Step 4:** test passes.
**Step 5:** `git commit -m "feat(core): sqlx sql-first migrator + local pg"`

---

## M1 — Walking skeleton

### Task 4: Domain primitives (ULID, ValueInterval, enums, GoldCandidate)

**Files:**
- Create: `crates/core/src/{ids.rs, domain/value.rs, domain/enums.rs, domain/gold.rs, schemas/mod.rs}`
- Test: inline `#[cfg(test)]` + `crates/core/tests/schema_snapshot.rs`

**Step 1: failing tests (pivotal ones, complete):**

```rust
// domain/value.rs tests — money is rust_decimal, serialized as strings. Never floats.
#[test]
fn exact_value_is_low_eq_high() {
    let v = ValueInterval::new(dec!(5000.00), Some(dec!(5000.00)), Currency::EUR).unwrap();
    assert_eq!(v.low(), v.high().unwrap());
}
#[test]
fn open_ended_threshold_high_is_none() {
    assert!(ValueInterval::new(dec!(70000.00), None, Currency::GBP).is_ok());
}
#[test]
fn rejects_high_below_low() {
    assert!(ValueInterval::new(dec!(10.00), Some(dec!(5.00)), Currency::USD).is_err());
}
#[test]
fn midpoint_of_us_band() {
    let v = ValueInterval::new(dec!(1001.00), Some(dec!(15000.00)), Currency::USD).unwrap();
    assert_eq!(v.midpoint().unwrap(), dec!(8000.50));
}
```

```rust
// domain/gold.rs tests — the cross-regime pair from the design doc, verbatim.
#[test]
fn accepts_us_ptr_transaction_and_uk_interest_rejects_sideless_transaction() {
    us_ptr_fixture().validate().unwrap();          // transaction: buy, 2026-03-02, 1001–15000 USD, spouse
    uk_interest_fixture().validate().unwrap();     // interest: notified 2026-04-10, 70000–open GBP
    let mut bad = us_ptr_fixture(); bad.side = None;
    assert!(matches!(bad.validate(), Err(DomainError::TypeRequires { .. })));
}
```

**Step 2:** `cargo test -p core` → FAIL. **Step 3:** implement: enums with `#[serde(rename_all = "snake_case")]`, `GoldCandidate` deriving `Serialize/Deserialize/JsonSchema`, `validate()` mirroring the SQL CHECKs (one rule, two enforcers — Rust and Postgres). Snapshot test writes `schemars::schema_for!(GoldCandidate)` to `crates/core/schemas/gold_candidate.json` and fails on diff (contract changes must be visible in git).
**Step 4:** green. **Step 5:** `git commit -m "feat(core): domain primitives + GoldCandidate contract (schema snapshot)"`

### Task 5: Migration 0001 — the design DDL

**Files:** `crates/core/migrations/0001_core.sql` — **copy verbatim from design §4.2** + `outbox_event`, `pipeline_run`, `review_task` shapes; test `crates/core/tests/ddl.rs`.

Failing `#[sqlx::test]`: insert both GoldCandidate examples via raw SQL; assert a transaction missing `side` and a `value_high < value_low` are rejected with SQLSTATE **23514**; then `cargo sqlx prepare --workspace` (offline query metadata, committed). Commit `feat(core): canonical gold DDL + sqlx offline metadata`.

### Task 6: Deterministic fingerprint

**Files:** `crates/core/src/domain/fingerprint.rs` + tests.

Failing tests assert: same `(filing_id, ordinal, canonical content)` → same 64-hex sha256; JSON key order / whitespace changes do **not** alter it; changing `value.low` does. Implement: canonicalize via `serde_json::Value` → recursive BTreeMap sort → `sha2`. Commit `feat(core): idempotency fingerprint`.

### Task 7: Adapter trait + conformance harness

**Files:**
- Create: `crates/pipeline/src/{adapter.rs, conformance.rs, bin/conformance.rs}` (`RunCtx` carries bronze store, pool, clock, politeness-wrapped `reqwest` client)
- Create: `crates/adapters/fixture_fake/` (reads local fixtures — exists to test the harness)
- Test: `crates/pipeline/tests/conformance.rs`

**Harness spec:** for each `crates/adapters/<x>/fixtures/<case>/{input.*, expected.silver.json, expected.gold.json}` — run `parse`, deep-compare Silver; run `normalize`, `validate()` every candidate **and** check `details` against that (regime, record_type) JSON Schema (`jsonschema` crate), deep-compare Gold; mismatches print a unified diff (`similar` crate). TDD against `fixture_fake` with one passing and one deliberately-broken case asserting diff output. Runner: `cargo run -p pipeline --bin conformance -- <adapter>`. Commit `feat(pipeline): adapter trait + conformance harness`.

### Task 8: First real adapter — `us_house`

**Why House first:** the Clerk publishes a machine-readable annual index (XML/ZIP) of PTR filings → deterministic `discover`; PDFs with text layers exercise Bronze + parse + the LLM-fallback seam. (Senate eFD's session dance is goal 060.)

**Files:** `docs/regimes/us-house.md` (written FIRST — it is the adapter's context), `crates/adapters/us_house/{src/adapter.rs, src/details.rs, fixtures/…}`, `crates/pipeline/src/bin/capture_fixture.rs`.

Steps: regime doc → capture ≥3 real fixtures (typical PTR, amendment, multi-row) → **HUMAN completes `expected.*.json` (you are ground truth, once per fixture)** → failing conformance → implement `discover` (index XML), `fetch` (Bronze by sha256), `parse` (`pdf-extract` text layer; below-threshold confidence routes to the `Extractor` trait stub — real LLM wiring is goal 021; if text-layer fidelity fails on fixtures, upgrade to `pdfium-render` and record why in the regime doc), `normalize` (band strings → `ValueInterval`, owner codes, `details` per schema) → conformance green ×3 → commit `feat(adapters): us_house PTR adapter (conformance x3)`.

### Task 9: Local pipeline runner (in-process)

**Files:** `crates/pipeline/src/{run.rs, stages/*.rs}`, `crates/worker/src/bin/local.rs`; test `crates/pipeline/tests/e2e_local.rs`.

Failing `#[sqlx::test]`: full run over `us_house` fixtures → Bronze rows exist, Silver rows exist, Gold rows have `verification_state='unverified'`, one `outbox_event` per record (same txn), `pipeline_run` rows carry idempotency keys — and **a second run inserts nothing** (`ON CONFLICT DO NOTHING` everywhere). Commit `feat(pipeline): end-to-end local run, idempotent`.

### Task 10: Minimal `/v1` (axum + utoipa)

**Files:** `crates/api/src/{main.rs, routes/records.rs, routes/politicians.rs}`, `crates/api/src/bin/openapi.rs` (emits `packages/contracts/openapi.json`); test `crates/api/tests/contract.rs`.

Failing contract test: boot axum on a test pool seeded by Task 9; GET `/v1/records` and `/v1/politicians/{id}/records`; validate response bodies against the emitted OpenAPI schema (`jsonschema`); assert ULID cursor pagination (page 2 begins after page 1's last id) and `verification_state` present on every record. Implement thin sqlx handlers, `#[utoipa::path]` annotations. CI drift step: `cargo run -p api --bin openapi && git diff --exit-code packages/contracts/`. Commit `feat(api): /v1 records + timeline, contract-tested + drift-gated`.

### Task 11: Two-stage publication smoke

**Files:** `crates/pipeline/src/promote.rs` + test.

Failing test: resolving a `review_task` flips the record to `verified`; an "edit" resolution inserts a superseding record (`corrected`, `supersedes_record_id` set) and the original row's facts are **never UPDATEd** — the supersede-never-update invariant locked behind a test before any UI exists. Commit `feat(pipeline): verification promotion + supersession`.

**M1 exit criteria (all green):**
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
cargo run -p pipeline --bin conformance -- us_house
docker compose up -d && cargo test --workspace -- --ignored   # sqlx integration suites
cargo run -p api   # human sees real House records at localhost:8080/v1/records
```

---

## M2+ — executed via goal files

Every remaining unit of work is an `agents/goals/NNN-*.md` file (objective, scope, context pointers, **acceptance criteria as commands**). Loop protocol: `agents/LOOP.md`; queue: `agents/goals/000-INDEX.md`. Adapter goals (060+) are the E1 repeatable template: *regime doc → fixtures → human expected outputs → conformance green.* From Epoch 2 (Brazil) onward, adapter work is GENERATED by the coverage factory (goal 015) from registry state — see agents/workflows/source-exploration.md and agents/EPOCHS.md; no hand-written adapter goals. Web work (040+) bootstraps pnpm + the generated TS client (`openapi-typescript` against `packages/contracts/openapi.json`).
