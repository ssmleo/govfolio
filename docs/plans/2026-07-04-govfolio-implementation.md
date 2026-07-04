# Govfolio v1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
> Work on a branch. Human-only lanes: applying DB migrations to prod, `terraform apply`, public claim-making copy.
> Read `/CLAUDE.md` and the relevant `agents/goals/NNN-*.md` before each task. Repo is memory: update the goal's checklist and commit every iteration.

**Goal:** Ship govfolio v1 — worldwide politician-disclosure tracking with a free transparency layer and paid real-time alerts + API — per `docs/plans/2026-07-04-govfolio-design.md`.

**Architecture:** Modular monolith (Cloud Run `api`/`web`/`worker`, scale-to-zero) + queue-driven ingestion (Bronze GCS → Silver staging → Gold Postgres), two-stage publication (`unverified → verified`), transactional-outbox alerts, OpenAPI-first `/v1` consumed by the website itself.

**Tech Stack:** TypeScript end-to-end (pnpm workspaces, strict TS), Fastify + OpenAPI, Next.js, Postgres (SQL-first migrations + kysely-codegen types), Zod (types + JSON Schemas from one source), Vitest, Playwright, GCP (Cloud Run, Cloud SQL, GCS, Cloud Tasks, Scheduler), Cloudflare, Terraform, GitHub Actions, Stripe.

---

## Milestone map

| M | Name | Detail lives in |
|---|---|---|
| M0 | Repo bootstrap (workspace, CI, local Postgres, migration runner) | **This doc — Tasks 1–3** |
| M1 | Walking skeleton (core schemas → DDL → fingerprint → adapter contract + conformance → `us_house` adapter → local pipeline run → minimal `/v1/records`) | **This doc — Tasks 4–11** |
| M2 | Cloud substrate (Terraform: SQL, GCS, Cloud Run ×3, Tasks, Scheduler; deploy skeleton) | `agents/goals/020` |
| M3 | Alerts (outbox dispatcher, email + HMAC webhooks, alert-rule CRUD on the shared filter grammar) | `agents/goals/030` |
| M4 | Website (SSR politician/record/jurisdiction pages, search, sitemap) + reviewer UI | `agents/goals/040–041` |
| M5 | Productization (auth, API keys, quotas/usage → Stripe, free-tier 24h delay) | `agents/goals/050` |
| M6 | Coverage wave 1 (`us_senate`, `uk`, `canada`, `australia`) + worldwide regime registry seed + coverage dashboard | `agents/goals/060–065` |
| M7 | Trust hardening (review queue UI, sampling audits, corrections log, per-regime redaction, drift detection) | `agents/goals/070` |
| M8 | US historical backfill (→2012) + launch checklist | `agents/goals/080` |

Rule: when a goal file is too big for one loop-session, the loop's first action is to expand it into `docs/plans/<date>-<goal>.md` **using this same task format**, then execute that.

---

## M0 — Repo bootstrap

### Task 1: pnpm workspace + strict TS + Vitest

**Files:**
- Create: `package.json`, `pnpm-workspace.yaml`, `tsconfig.base.json`, `.gitignore`, `.nvmrc`
- Create: `packages/core/package.json`, `packages/core/tsconfig.json`
- Create: `packages/core/src/index.ts`
- Test: `packages/core/src/index.test.ts`

**Step 1: Write the failing test**

```ts
// packages/core/src/index.test.ts
import { describe, expect, it } from "vitest";
import { hello } from "./index";

describe("workspace smoke", () => {
  it("compiles and runs", () => {
    expect(hello()).toBe("govfolio");
  });
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm i && pnpm -r test`
Expected: FAIL — `hello` is not exported / module not found.

**Step 3: Write minimal implementation**

```ts
// packages/core/src/index.ts
export const hello = (): string => "govfolio";
```

`tsconfig.base.json` must set: `"strict": true, "noUncheckedIndexedAccess": true, "exactOptionalPropertyTypes": true, "module": "NodeNext"`. No `any` anywhere in the repo — CI greps for it (Task 2).

**Step 4: Run test to verify it passes**

Run: `pnpm -r test` → Expected: 1 passed.
Run: `pnpm -r typecheck` (script = `tsc --noEmit`) → Expected: clean.

**Step 5: Commit**

```bash
git add -A && git commit -m "chore: pnpm workspace, strict TS, vitest smoke"
```

### Task 2: CI gate

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `scripts/no-any.sh` (`! grep -rn ": any" --include="*.ts" packages apps || (echo "no any allowed" && exit 1)`)

Steps: write workflow running `pnpm i --frozen-lockfile`, `pnpm -r lint`, `pnpm -r typecheck`, `pnpm -r test`, `scripts/no-any.sh`; push branch; verify the workflow runs green; commit. (Add eslint+prettier configs in this task; keep rules default+strict.)

### Task 3: Local Postgres + SQL-first migration runner

**Rationale:** the design doc's DDL is authoritative; migrations are plain `.sql` files so the schema in git is exactly the schema in prod. Types are *generated* from the live DB (kysely-codegen), never hand-written — they can't drift.

**Files:**
- Create: `docker-compose.yml` (postgres:16, port 5433, healthcheck)
- Create: `packages/core/migrations/0000_init.sql` (empty marker table `schema_migrations`)
- Create: `packages/core/src/db/migrate.ts` (~30 lines: read `migrations/*.sql` in order, skip applied, apply in a transaction, record filename)
- Test: `packages/core/src/db/migrate.test.ts` (integration, tagged `@db`)

**Step 1: failing test** — spins nothing itself; requires `docker compose up -d`; asserts `migrate()` applies `0000` once and is idempotent on second run (row count in `schema_migrations` unchanged).
**Step 2:** `docker compose up -d && pnpm --filter core test:db` → FAIL (migrate.ts missing).
**Step 3:** implement runner with `pg` client; `DATABASE_URL` from env, default `postgres://postgres:postgres@localhost:5433/govfolio`.
**Step 4:** test passes; running twice is a no-op.
**Step 5:** `git commit -m "feat(core): sql-first migration runner + local pg"`

---

## M1 — Walking skeleton

### Task 4: Domain primitives (ULID, value interval, enums, GoldCandidate)

**Files:**
- Create: `packages/core/src/ids.ts`, `packages/core/src/domain/value.ts`, `packages/core/src/domain/enums.ts`, `packages/core/src/domain/gold.ts`
- Test: `packages/core/src/domain/value.test.ts`, `gold.test.ts`

**Step 1: failing tests (complete):**

```ts
// value.test.ts
import { describe, expect, it } from "vitest";
import { ValueInterval, midpoint } from "./value";

describe("ValueInterval", () => {
  it("accepts exact values as low==high", () => {
    expect(ValueInterval.parse({ low: "5000.00", high: "5000.00", currency: "EUR" }).low)
      .toBe("5000.00");
  });
  it("accepts open-ended thresholds (high null)", () => {
    const v = ValueInterval.parse({ low: "70000.00", high: null, currency: "GBP" });
    expect(v.high).toBeNull();
  });
  it("rejects high < low", () => {
    expect(() => ValueInterval.parse({ low: "10.00", high: "5.00", currency: "USD" }))
      .toThrow();
  });
  it("midpoint of a US band", () => {
    expect(midpoint({ low: "1001.00", high: "15000.00", currency: "USD" })).toBe("8000.50");
  });
});
```

```ts
// gold.test.ts — the cross-regime pair from the design doc, verbatim as fixtures
import { GoldCandidate } from "./gold";
it("accepts a US PTR transaction", () => {
  GoldCandidate.parse({
    recordType: "transaction", side: "buy", transactionDate: "2026-03-02",
    assetDescriptionRaw: "NVIDIA Corporation - Common Stock", assetClass: "equity",
    value: { low: "1001.00", high: "15000.00", currency: "USD" }, owner: "spouse",
    details: { ptr_row: 3 },
  });
});
it("accepts a UK categorical interest and rejects a transaction without side", () => {
  GoldCandidate.parse({
    recordType: "interest", notifiedDate: "2026-04-10",
    assetDescriptionRaw: "Shareholding in X Ltd (Category 7(i))", assetClass: "equity",
    value: { low: "70000.00", high: null, currency: "GBP" }, owner: "self",
    details: { category: "7(i)" },
  });
  expect(() => GoldCandidate.parse({ recordType: "transaction",
    assetDescriptionRaw: "x", assetClass: "equity", owner: "self", details: {} }))
    .toThrow();
});
```

**Step 2:** run → FAIL. **Step 3:** implement with Zod: money as **decimal strings** (never floats — rationale: numeric fidelity end-to-end to `numeric(16,2)`), `superRefine` for per-type requirements mirroring the SQL CHECKs (one rule, two enforcers). Export `zodToJsonSchema(GoldCandidate)` from `packages/core/src/schemas/`. **Step 4:** pass. **Step 5:** commit `feat(core): domain primitives + GoldCandidate contract`.

### Task 5: Migration 0001 — the design DDL

**Files:**
- Create: `packages/core/migrations/0001_core.sql` — **copy the DDL verbatim from `docs/plans/2026-07-04-govfolio-design.md §4.2`**, plus `outbox_event`, `pipeline_run`, `review_task` shapes from §4.2's supporting list.
- Test: extend `migrate.test.ts`: after migrate, `insert` the two GoldCandidate examples (via raw SQL) and assert the CHECK constraints reject a transaction missing `side` and a `value_high < value_low`.
- Create: `packages/core/src/db/types.ts` via `pnpm --filter core db:codegen` (kysely-codegen against local DB) — committed, regenerated in CI to detect drift.

Steps: failing test → apply → verify CHECKs fire (expected SQLSTATE 23514) → codegen → commit `feat(core): canonical gold DDL + generated db types`.

### Task 6: Deterministic record fingerprint

**Files:** `packages/core/src/domain/fingerprint.ts` + test.

Failing test asserts: same `(filingId, ordinal, canonicalized content)` → same 64-hex sha256; key-order and whitespace changes in `details` do **not** change it; changing `value.low` does. Implement with stable-stringify (sorted keys) + `crypto`. Commit `feat(core): idempotency fingerprint`.

### Task 7: Adapter contract + conformance harness

**Files:**
- Create: `packages/pipeline/src/adapter.ts` (the `JurisdictionAdapter` interface from design §5.1, plus `RunCtx` carrying: bronze store, db, clock, http with politeness wrapper)
- Create: `packages/pipeline/src/conformance.ts`
- Create: `packages/adapters/_fixture_fake/` (fake adapter reading local fixture files — exists to test the harness itself)
- Test: `packages/pipeline/src/conformance.test.ts`

**Harness spec (complete behavior):** given `packages/adapters/<x>/fixtures/<case>/{input.*, expected.silver.json, expected.gold.json}` — run `parse` on input, deep-compare to `expected.silver.json`; run `normalize`, validate every candidate against `GoldCandidate` **and** the `(regime, recordType)` JSON Schema for `details`, deep-compare to `expected.gold.json`. Any mismatch prints a unified diff. Exposed as `pnpm conformance --filter adapters/<x>`.

TDD it against `_fixture_fake` (one passing case, one deliberately-broken case asserting the diff output). Commit `feat(pipeline): adapter contract + conformance harness`.

### Task 8: First real adapter — `us_house`

**Why House first:** the Clerk publishes a machine-readable annual index (XML/ZIP) of PTR filings → deterministic `discover`; documents are PDFs with text layers → exercises Bronze + PDF parse + LLM-fallback paths. (Senate eFD needs a session dance; it's goal 060.)

**Files:**
- Create: `docs/regimes/us-house.md` (methodology: source URLs, cadence, bands table, quirks — written FIRST; it is the adapter's context)
- Create: `packages/adapters/us_house/{adapter.ts, config.ts, schemas/transaction.details.ts, fixtures/…}`
- Create: `tools/capture-fixture.ts` (fetch one real filing → write `input.pdf` + skeleton expected files for human completion)
- Test: conformance fixtures ×3 minimum (typical PTR, an amendment, a multi-row filing)

Steps: write `us-house.md` → capture 3 fixtures → hand-complete `expected.*.json` (**human step — you are the ground truth once per fixture**) → failing conformance → implement `discover` (index XML), `fetch` (store Bronze via `sha256`), `parse` (pdf text-layer via `pdfjs-dist`; if extraction confidence < threshold, route to LLM extractor stub interface — real LLM wiring is goal 021), `normalize` (band strings → `ValueInterval`, owner codes, `details` per schema) → green ×3 → commit `feat(adapters): us_house PTR adapter (conformance ×3)`.

### Task 9: Local pipeline runner (in-process queue)

**Files:** `packages/pipeline/src/{run.ts, stages/*.ts}`, `apps/worker/src/local.ts`; integration test `packages/pipeline/src/e2e.local.test.ts` (`@db`).

Failing test: run full pipeline over `us_house` fixtures against dockerized PG → asserts Bronze rows, Silver rows, Gold rows `verification_state='unverified'`, one `outbox_event` per record, `pipeline_run` rows with idempotency keys; **running twice inserts nothing new**. Implement stages calling the adapter + `ON CONFLICT DO NOTHING` writes. Commit `feat(pipeline): end-to-end local run, idempotent`.

### Task 10: Minimal `/v1` (records + politician timeline)

**Files:** `packages/contracts/openapi.yaml` (only `/v1/records`, `/v1/politicians/{id}/records`, error envelope, cursor params), `apps/api/src/{server.ts, routes/records.ts}`, contract test `apps/api/src/contract.test.ts`.

Failing contract test: boots Fastify against test DB seeded by Task 9, fetches both endpoints, validates responses **against the OpenAPI schema** (ajv), checks ULID cursor pagination (page 2 starts after page 1's last id) and `verification_state` present on every record. Implement thin handlers over kysely. Commit `feat(api): /v1 records + timeline, contract-tested`.

### Task 11: Two-stage publication smoke

**Files:** `packages/pipeline/src/promote.ts` + test.

Failing test: resolve a `review_task` → record flips to `verified`; an "edit" resolution inserts a superseding record (`corrected`, `supersedes_record_id` set) and never UPDATEs the original row's facts. This locks the supersede-never-update invariant behind a test before any UI exists. Commit `feat(pipeline): verification promotion + supersession`.

**M1 exit criteria (all must be green):** `pnpm -r lint && pnpm -r typecheck && pnpm -r test && pnpm conformance && pnpm test:db` — and a human can run `pnpm skeleton:demo` to see real House records served at `localhost:8080/v1/records`.

---

## M2+ — executed via goal files

Every remaining unit of work is an `agents/goals/NNN-*.md` file (objective, scope in/out, context pointers, **acceptance criteria as commands**). The loop protocol is `agents/LOOP.md`; the ordered queue is `agents/goals/000-INDEX.md`. Adapter goals (060+) are the repeatable template: *write `docs/regimes/<x>.md` → capture fixtures → human completes expected outputs → conformance green.*
