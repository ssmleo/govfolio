# Govfolio.io — System Design

- **Date:** 2026-07-04
- **Status:** Approved (all 5 sections reviewed and approved by founder). **Amended same day: D7 stack → Rust data plane + TypeScript web (founder decision after stack discussion; see D7).**
- **Scope:** Worldwide politician financial-disclosure tracking; transparency tool + trading-signal product
- **Builder/operator:** Solo founder, AI-agent-driven development (goal-file / Ralph-loop execution)

---

## 1. Summary and positioning

Govfolio tracks politicians' financial disclosures worldwide and serves two audiences from one dataset:

- **Transparency** (journalists, researchers, citizens): free, complete, provenance-linked data.
- **Signals** (traders): paid immediacy — real-time alerts, real-time API, webhooks.

**Reconciliation of the two (decided up front, drives the architecture):**

> **Sell latency and convenience, never exclusivity.** Every datapoint eventually becomes free (~24h delay on the free tier). The paid good is *immediacy* and *machine access*. The narrative: "we can't stop the information asymmetry, so we expose it and level the playing field."

This resolves the apparent conflict the same way successful incumbents (Quiver Quantitative, Unusual Whales, Capitol Trades) do, and it materializes in the data model as the **two-stage publication state machine**: records publish instantly as `unverified` (speed for signal buyers) and are promoted to `verified` after QA (rigor for the transparency mission).

**Regulatory hedge (live risk as of 2026):** US bills advancing in Congress would ban *new* individual stock purchases by members while (a) allowing retention of holdings, (b) requiring 7-day public pre-sale notices, and (c) exempting crypto, commodities, and diversified funds. Consequences designed in:

- Data model is **asset-class agnostic** and **jurisdiction-aware**.
- `disclosure_regime` is a **first-class, versioned entity** — a legal change is a new regime row, not a schema migration.
- If a ban passes: transparency product survives on holdings/conflicts/violations; pre-sale notices become a *new* signal stream; activity migrates to exempted asset classes and other jurisdictions — all representable without redesign.

## 2. Decisions log (with rationale)

| # | Decision | Chosen | Rationale |
|---|---|---|---|
| D1 | Geographic scope | **Worldwide from day one** (schema + brand); tiered ingestion rollout (~8 jurisdictions at launch) | Building 100+ scrapers pre-launch is infeasible; a global **jurisdiction registry** with regime metadata ships day one and doubles as a "transparency scorecard" (mission content, SEO, differentiation). Expansion = adding an adapter, never a redesign. |
| D2 | Data sourcing | **Build from primary sources** | Full commercial resale rights (vendor licenses restrict redistribution — fatal for a signal/API business); filing-level provenance (credibility + defamation defense); no vendor covers worldwide anyway; the pipeline becomes the moat. |
| D3 | v1 product surface | **Website + alerts + paid API** | API-from-day-one forces API-first design (site is a client of `/v1`). Alerts set the latency SLO. Both are the revenue engines of the "sell latency" model. |
| D4 | Team | **Solo + AI agents** | Drives: ops minimalism (managed services, few moving parts) and agent legibility (monorepo, one language, executable done-criteria). |
| D5 | Architecture | **Modular monolith + queue-driven workers** (Approach A) | Volume reality: canonical data is low-millions rows/yr — Postgres territory for a decade. Microservices/lakehouse (Approach C) solves team-coordination problems a solo founder doesn't have at a $500–2000/mo cost floor. BaaS-first (Approach B) fights the product's pipeline-centric core (long-running OCR/LLM jobs vs edge-function limits). |
| D6 | Cloud | **GCP** (Cloud Run + Cloud SQL + GCS + Cloud Tasks), Cloudflare in front | Scale-to-zero containers fit bursty parsing + solo cost profile. AWS equivalents (Fargate/App Runner, RDS, S3, SQS) noted; nothing GCP-exotic is used. |
| D7 | Language *(amended)* | **Hybrid: Rust data plane + TypeScript web.** Rust: `core`, `pipeline`, `adapters`, `worker`, `api`. TS: `apps/web` only. | Boundary = workload boundary. Rust where correctness is existential and the compiler strengthens agent loops (exhaustive enums, no-null, `unwrap` banned), plus static binaries / ~ms cold starts on scale-to-zero, no-GC tails on the *paid* API, and a home for the v2 analytics engine. TS where ecosystem velocity dominates (Next.js SSR, reviewer UI). `serde`+`schemars` replaces Zod as the one-definition source: Rust types → JSON Schemas (`details` validation) → OpenAPI (`utoipa`) → generated TS client. API sits Rust-side so (a) domain types are defined exactly once, (b) the shared filter grammar has one implementation for `/records` *and* alert matching, (c) customers measure the paid API's tail latency. Honest cost: two toolchains — mitigated by generated-contract drift checks and per-side agent context. Perf caveat kept on record: the workload is I/O/data-bound; the biggest perf levers remain Postgres indexes, keyset pagination, and CDN discipline (explicit budgets in §10). Original TS-end-to-end rationale superseded by founder conviction + the above. |
| D8 | Homogeneity question (challenged & resolved) | **Stratified, not homogeneous** | Heterogeneity preserved in Bronze (raw docs) and Silver (per-regime staging tables). Gold unifies only the genuinely-common queryable core; regime specifics live in `details` JSONB **typed-by-contract** (versioned JSON Schema per (regime, record_type), validated at promotion). Semantics never laundered: API/UI always type-scope counts and analytics. |
| D9 | Coverage expansion + agent layer | **Coverage factory: phased source-exploration workflow driven by registry state; nine specified agent roles; Source Authority Files (SAF) with a write-back rule; geographic epochs US → Brazil → Europe → Asia → Oceania** | Depth of research is enforced by validation, not exhortation: each phase gates on a schema-validated, evidence-backed artifact (evidence-or-it-didn't-happen; `unknown` legal only with a tried-log; independent auditor pass — the data pipeline's `unverified→verified` pattern applied to research). Agent roles are specified like adapters (mission, tools/budget, output contract, anti-patterns, eval) and **calibrated against the hand-built `us_house` ground truth before any epoch opens**. SAFs are the living per-source canon all specialists must load; write-back-in-same-PR keeps knowledge compounding in the repo instead of evaporating in transcripts. Brazil-second is engineering-smart: first non-English regime hardens multilingual extraction and B3 instrument resolution before Europe's language fan-out. E1's UK/CA/AU/EU/FR/DE goals are regime-type **diversity seeds** (one per record_type path), not geography. From E2 onward no adapter goal is hand-written — the factory generates work from the registry. |

## 3. Architecture overview

```
                    ┌────────────────────────────────────────────────┐
                    │                 Cloudflare (DNS/CDN/WAF)       │
                    └───────────────┬────────────────────────────────┘
                                    │
        ┌───────────────┐   ┌──────┴───────┐    ┌──────────────────┐
        │  apps/web     │──▶│  apps/api    │◀───│  API customers   │
        │  (Next.js SSR │   │  (/v1,       │    │  (keys, quotas)  │
        │  + reviewer)  │   │  OpenAPI)    │    └──────────────────┘
        └───────────────┘   └──────┬───────┘
                                   │ reads
                          ┌────────┴────────┐
                          │  Cloud SQL (PG) │◀──────────────┐
                          │  Gold + Silver  │               │ writes
                          └────────┬────────┘               │
                                   │ outbox         ┌───────┴────────┐
                          ┌────────┴────────┐       │  apps/worker   │
                          │ alert dispatcher│       │  (stages via   │
                          │ email/webhooks  │       │  Cloud Tasks)  │
                          └─────────────────┘       └───────┬────────┘
                                                            │
                    ┌───────────────────────────────────────┴───────┐
                    │ GCS: Bronze raw documents (immutable, sha256) │
                    └───────────────────────────────────────────────┘
        Cloud Scheduler → per-jurisdiction discover cadences (Tier 1/2/3)
```

Three Cloud Run services (`api`, `web`, `worker`), all scale-to-zero. Postgres is the single source of truth for refined data; GCS holds unbounded raw growth; Cloud Tasks connects stages; the transactional outbox connects data to alerts.

**Language boundary (invariant):** `api` and `worker` are Rust; `web` is TypeScript. If it touches Bronze/Silver/Gold or defines domain semantics → Rust; if it renders pixels → TypeScript. The generated OpenAPI contract is the only door between the two; regeneration drift fails CI.

## 4. Data model

### 4.1 Principles (the six decisions)

1. **Four observation types, one table.** `disclosure_record` with `record_type ∈ {transaction, holding, interest, change_notification}`. The dominant queries (politician timeline, instrument activity, alert filters) cut across types → one index path, one API shape. Divergent ~20% lives in `details` JSONB.
2. **Value as an interval, always.** `value_low / value_high / currency`. US bands, French exact figures (`low = high`), UK open thresholds (`high = NULL`) — one representation, honest cross-regime math.
3. **Regime as versioned first-class entity.** Legal changes are new regime rows with `effective_from/to`; history stays attached to the rules it was filed under.
4. **Never update — supersede.** Immutable facts; amendments/corrections chain via `supersedes_record_id`. Three timestamps: `filed_date` (claimed), `published_at` (government released), `discovered_at` (we found it). Full bitemporal modeling deliberately rejected (YAGNI); this gets ~90% of it.
5. **Bronze / Silver / Gold.**
   - *Bronze:* every original document, immutable, SHA-256-addressed in GCS.
   - *Silver:* per-regime staging tables shaped like the source, tagged `extracted_by` + `extraction_confidence`.
   - *Gold:* canonical `disclosure_record` the API serves.
   Reprocessing replays Bronze→Gold when extraction improves; quality becomes *reprocessable*.
6. **Identity done properly.** `politician` (Wikidata-keyed person) ≠ `mandate` (tenure: body, role, party, dates). `instrument` securities master (ticker/ISIN/FIGI) + alias tables. **Raw as-filed text is always kept** beside any resolved ID.

### 4.2 Core DDL (canonical Gold layer)

```sql
-- ULIDs stored as text: time-sortable, URL-safe, no coordination.

create table jurisdiction (
  id          text primary key,
  name        text not null,
  iso_code    text,                      -- ISO 3166-1 alpha-2 where applicable
  level       text not null check (level in ('supranational','national','subnational')),
  parent_id   text references jurisdiction(id)
);

create table disclosure_regime (
  id                  text primary key,
  jurisdiction_id     text not null references jurisdiction(id),
  body                text not null,     -- 'US House', 'US Senate', 'Bundestag', ...
  regime_type         text not null check (regime_type in
                        ('transaction_report','periodic_declaration',
                         'change_notification','none')),
  value_precision     text not null check (value_precision in
                        ('exact','banded','categorical','none')),
  cadence             text,
  disclosure_lag_days int,
  source_url          text,
  details             jsonb not null default '{}',
  effective_from      date not null,
  effective_to        date,
  unique (jurisdiction_id, body, effective_from)
);

create table politician (
  id             text primary key,
  canonical_name text not null,
  wikidata_qid   text unique,
  details        jsonb not null default '{}'
);

create table politician_alias (
  politician_id text not null references politician(id),
  alias         text not null,
  lang          text,
  primary key (politician_id, alias)
);

create table mandate (
  id              text primary key,
  politician_id   text not null references politician(id),
  jurisdiction_id text not null references jurisdiction(id),
  body            text not null,
  role            text not null,
  party           text,
  district        text,
  start_date      date not null,
  end_date        date
);

create table instrument (
  id          text primary key,
  name        text not null,
  ticker      text,
  isin        text,
  figi        text,
  asset_class text not null,
  country     text,
  details     jsonb not null default '{}'
);

create table instrument_alias (
  instrument_id text not null references instrument(id),
  alias         text not null,
  source        text,
  primary key (instrument_id, alias)
);

create table raw_document (
  id           text primary key,
  storage_uri  text not null,
  sha256       text not null unique,     -- dedup + integrity + reprocess cache key
  mime_type    text not null,
  source_url   text,
  fetched_at   timestamptz not null,
  fetch_run_id text
);

create table filing (
  id                   text primary key,
  regime_id            text not null references disclosure_regime(id),
  politician_id        text not null references politician(id),
  raw_document_id      text not null references raw_document(id),
  external_id          text,             -- source-native id when the source has one
  filing_type          text not null,
  filed_date           date,
  published_at         timestamptz,      -- when the government made it public
  discovered_at        timestamptz not null,  -- when we found it (our latency, honestly)
  supersedes_filing_id text references filing(id),
  details              jsonb not null default '{}',
  unique (regime_id, external_id)
);

create table disclosure_record (
  id                    text primary key,
  filing_id             text not null references filing(id),
  politician_id         text not null references politician(id),  -- denorm (hot path)
  regime_id             text not null references disclosure_regime(id), -- denorm
  instrument_id         text references instrument(id),           -- nullable: never guess
  asset_description_raw text not null,                            -- as filed, always kept
  record_type           text not null check (record_type in
                          ('transaction','holding','interest','change_notification')),
  asset_class           text not null,   -- equity|bond|fund|option|crypto|commodity|
                                         -- real_estate|private|other
  side                  text check (side in ('buy','sell','exchange')),
  transaction_date      date,
  as_of_date            date,
  notified_date         date,
  event_date            date generated always as
                          (coalesce(transaction_date, notified_date, as_of_date)) stored,
  value_low             numeric(16,2),
  value_high            numeric(16,2),   -- NULL = open-ended (e.g. UK "> threshold")
  currency              char(3),
  owner                 text check (owner in
                          ('self','spouse','dependent','joint','unknown')),
  verification_state    text not null default 'unverified' check (verification_state in
                          ('unverified','verified','corrected','disputed')),
  extraction_confidence real,
  extracted_by          text not null,   -- parser id / model+prompt version
  fingerprint           text not null unique,  -- hash(filing_id, ordinal, content) → idempotency
  supersedes_record_id  text references disclosure_record(id),
  details               jsonb not null default '{}',  -- validated by (regime, type) JSON Schema
  created_at            timestamptz not null default now(),
  -- per-type integrity inside one table:
  check (record_type <> 'transaction' or (side is not null and transaction_date is not null)),
  check (record_type <> 'holding' or as_of_date is not null),
  check (value_low is null or value_high is null or value_high >= value_low)
);

create index dr_politician_time on disclosure_record (politician_id, event_date desc);
create index dr_instrument_time on disclosure_record (instrument_id, event_date desc)
  where instrument_id is not null;
create index dr_type_time       on disclosure_record (record_type, event_date desc);
create index dr_review          on disclosure_record (verification_state)
  where verification_state = 'unverified';
-- GIN on details added per-need, not by default.
```

Supporting tables (full DDL written during implementation, shapes fixed here):

- `review_task(id, target_kind, target_id, reason, priority_score, status, assignee, resolution, created_at, resolved_at)`
- Coverage state on `jurisdiction`: `epoch, coverage_phase, priority_score, claimed_by, claimed_at, blocked_reason` (drives the coverage factory, §5.8)
- `pipeline_run(id, adapter, stage, idempotency_key unique, status, stats jsonb, started_at, finished_at, error)`
- `outbox_event(id, kind, payload jsonb, created_at, dispatched_at)` — written in the same txn as Gold inserts
- `alert_rule(id, user_id, filter jsonb, channels jsonb, digest, active)` — filter grammar == `/records` query grammar
- `delivery(id, alert_rule_id, outbox_event_id, channel, dedup_key unique, status, attempts, last_error)`
- `user_account`, `api_key(hash)`, `usage_event`, plus Stripe-mirroring billing tables (standard SaaS furniture)
- Silver: one `stg_<regime>` table per adapter, source-shaped, plus `stg_meta` (run linkage)

### 4.3 The `details` contract system

- Each `(regime, record_type)` pair has a **versioned Rust type** in `crates/core/src/schemas/` deriving `serde::{Serialize,Deserialize}` + `schemars::JsonSchema`; emitted JSON Schemas are snapshot-committed so contract changes are visible in git diffs.
- Silver→Gold promotion **validates and rejects on mismatch** — JSONB is schema-on-write by contract, not a swamp.
- These schemas double as adapter conformance fixtures for agent loops.
- **Escape hatch:** if a `details` key becomes query-hot, promote it to a real (generated or backfilled) column without rewriting history.

### 4.4 Scale plan

- Volumes: US ≈ 10–15k transactions/yr; ~45k national legislators worldwide, mostly annual declarations → **low millions of Gold rows/yr**. Postgres, single primary, for years.
- Unbounded growth (raw docs) lives in GCS; Postgres stores pointers.
- Declarative yearly range partitioning of `disclosure_record`: **planned, deferred until ~10M rows.**
- Read scaling order: CDN/ETag caching → read replica → (much later) search/analytics offload (Typesense/DuckDB-on-Parquet) as *additions*, not rewrites.

## 5. Ingestion pipeline

### 5.1 Adapter contract (the agent task template)

```rust
#[async_trait]
pub trait JurisdictionAdapter: Send + Sync {
    fn regime(&self) -> RegimeRef;           // binds adapter → disclosure_regime row
    fn politeness(&self) -> PolitenessCfg;   // min interval, concurrency (default 1), UA contact

    async fn discover(&self, ctx: &RunCtx) -> Result<Vec<FilingRef>>;      // new/changed filings
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> Result<RawDocRef>;   // download → Bronze
    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> Result<Vec<StagingRow>>; // Bronze → Silver (+confidence)
    async fn normalize(&self, rows: &[StagingRow], ctx: &RunCtx) -> Result<Vec<GoldCandidate>>; // Silver → Gold
}
```

An adapter ships as: code + config + **golden fixtures** (real sample filings + expected Silver and Gold output) + the shared **conformance suite**. "Make jurisdiction X pass conformance" is a complete, self-verifying agent goal. Core code never changes when coverage grows.

### 5.2 Stages, queues, idempotency

- Cloud Scheduler triggers `discover` per adapter at tier cadence; each stage enqueues the next via Cloud Tasks: `discover → fetch → parse → normalize+resolve → publish`.
- Queues are at-least-once ⇒ **dedup is ours**: docs keyed by `sha256`, filings by `(regime_id, external_id)`, records by deterministic `fingerprint`; all writes `ON CONFLICT DO NOTHING`. Every stage is crash-safe and re-runnable. `pipeline_run` holds idempotency keys + audit stats.
- `publish` inserts Gold as `unverified`, writes `outbox_event` in the same transaction, and opens `review_task`s per the rules in §7.

### 5.3 Parsing strategy

1. **Deterministic first:** open-data feeds and HTML tables get coded parsers; digital PDFs get text-layer extraction.
2. **LLM second:** scanned/handwritten/layout-hostile docs go to LLM extraction, constrained to the target JSON Schema, confidence-scored.
3. **Cross-check on impact:** high-value or watchlist-politician rows get a cheap second-model verification before the fast path.
4. **Cache by SHA:** re-extraction happens only on parser/model version bump — pay per document *version*, once.

### 5.4 Entity resolution

- Politician: high precision (filings name their filer); rosters seeded from official member lists + Wikidata.
- Instrument waterfall: exact ticker/ISIN → alias table → OpenFIGI lookup → fuzzy match with threshold. **Below threshold: `instrument_id = NULL`, keep raw text, open review task.** A wrong link is worse than a missing one (signal quality *and* defamation exposure).

### 5.5 Cadence tiers & politeness

| Tier | Regimes | Discover cadence | Latency target |
|---|---|---|---|
| 1 | US House, US Senate (transaction reports) | 1–5 min in publication windows | discover→publish p50 < 10 min |
| 2 | Change-notification registers (UK, AU, CA) | hourly–daily | same day |
| 3 | Annual declarations (EU-P, FR, DE, JP, …) | daily check | bulk on publication |

Conditional GETs (ETag/Last-Modified), per-source rate limits, identified user-agent with contact address. Polite scraping is durable scraping.

### 5.6 Failure handling & reprocessing

- **Fail closed on drift:** adapter yields zero rows or layout shifts → freeze that adapter's publication, open review task. Garbage never reaches Gold.
- Dead-letter queue per stage; retries with backoff.
- `reprocess(adapter, date_range, extractor_version)` replays Bronze→Gold and emits a **diff report** (adds/changes/supersessions), human-gated for mass changes. Reprocessing supersedes; it never mutates.
- **Backfill = the same pipeline pointed at archives.** Launch with US history to the 2012 STOCK Act era: depth moat + base for future analytics.

### 5.7 Launch jurisdictions (v1 ingestion)

US House, US Senate (Tier 1); UK, Canada, Australia (Tier 2); EU Parliament, France (HATVP), Germany (Bundestag) (Tier 3). Registry seeded with **all** countries + regime metadata stubs; "research regime for country X" is itself a templated agent goal.

### 5.8 Coverage factory (source-exploration workflow)

A second pipeline that turns *jurisdictions into adapters*, mirroring the data pipeline's trust machinery. Phases (registry state machine on `jurisdiction.coverage_phase`): `stub → scouted → surveyed → sampled → specced → built → live | blocked:<reason>`, each gated by a validated artifact — `sources.yaml` (Scout), schema-validated `AUTHORITY.md` front-matter (Surveyor + Auditor pass), fixtures + manifest (Sampler, human glance), extraction plan + draft expected outputs (Spec-writer + Test-designer, human confirms), conformance-green adapter (Builder), continuous drift defense (Sentinel WATCH). Disciplines: every claim carries evidence (URL + archived snapshot); `unknown` is legal only with a tried-log; auditors never audit their own production; SAF write-back is part of every task's definition of done. Work selection: highest `priority_score` within the current epoch (`agents/EPOCHS.md`), leased via `claimed_by/claimed_at` so parallel loops don't collide. Full playbook with per-phase prompts: `agents/workflows/source-exploration.md`; role specs: `agents/roles/`. Registry columns added: `epoch, coverage_phase, priority_score, claimed_by, claimed_at, blocked_reason`.

## 6. API, alerts, product layer

### 6.1 API

- Rust domain types are the source of truth: `utoipa` emits `packages/contracts/openapi.json` from the axum handlers; the TypeScript web client is generated from it; regeneration drift fails CI (generated files are committed, never hand-edited). Everything under `/v1`.
- Resources mirror Gold ~1:1: `/politicians`, `/politicians/{id}/records`, `/records` (filters: jurisdiction, type, asset_class, instrument, politician, date range, value bounds, verification_state), `/instruments/{id}/records`, `/filings/{id}` (+ raw-doc link), `/jurisdictions`, `/regimes` (= the transparency scorecard endpoint), `/search`.
- Cursor pagination on ULIDs; ETags everywhere; consistent error envelope.
- The website consumes this same API. No private endpoints for core data.

### 6.2 Freemium boundary (the business model, mechanically)

| Capability | Free | Pro (traders) | Data/API |
|---|---|---|---|
| Website browse/search/profiles | full | full | full |
| Provenance links | ✓ | ✓ | ✓ |
| Freshness | ~24 h delayed | real-time | real-time |
| Alerts | — | instant email + webhook | webhooks |
| API | 60 req/day, delayed | small quota | full, metered |
| Bulk export | monthly snapshot | ✓ | ✓ + backfills |

The 24-hour delay is the only monetization lever. Everything becomes free; immediacy is the product.

### 6.3 Alerts

- Transactional outbox (event row committed with the record) → dispatcher worker → rule matching in Postgres (volumes: thousands of events/day; indexed match is microseconds; no streaming infra) → per-channel senders with retries, idempotent `dedup_key`s, DLQ. Digest mode per rule.
- **One filter grammar** shared by `/records`, the UI, and `alert_rule.filter` — learned once, tested once.
- Alert payloads carry `verification_state` + confidence. Honesty travels with the fast path.

### 6.4 Productization & growth

- Hashed API keys; coarse limits at Cloudflare; monthly quotas via `usage_event` → Stripe metered billing; HMAC-signed outbound webhooks. Postgres-backed counters at launch; Redis is a documented, unbuilt upgrade path.
- SEO is the free tier's growth engine: permanent, CDN-cached SSR URLs for every politician / instrument / filing / jurisdiction; sitemaps; Postgres FTS behind `/search` until it hurts.

## 7. Trust, quality, legal

### 7.1 Verification state machine

`unverified → verified`, branches `corrected`, `disputed`; supersession chains as history.

- **Auto-verify** (no human): deterministic parse + exact entity match + machine-readable regime.
- **Sampled spot-check:** confidence ~0.8–0.95 → published, randomly audited.
- **Mandatory review before promotion:** confidence <0.8, unresolved instrument, adapter drift, high-impact (value threshold, watchlist politician, anomaly), user report.

Rationale: solo operator ⇒ human review is the exception; attention concentrates where error cost is highest.

### 7.2 Review queue

`review_task` priority = impact × uncertainty; reviewer UI shows extracted fields beside the Bronze document; LLM pre-review note drafts the anomaly summary. Approve / edit / reject; edits create superseding `corrected` records; all actions audit-logged. Public "report an issue" feeds the same queue.

### 7.3 Trust surface (audit trail as UI)

Every record page: official-source link + our archived copy, verification badge + confidence, full supersession/correction history, regime methodology link. Public per-jurisdiction **coverage dashboard** (covered since, known gaps, freshness). Documented limits are what make a source citable.

### 7.4 Quality SLOs

- Extraction precision ≥ 99% on value/side/instrument/date (monthly random-sample audit).
- Freshness per tier (Tier 1 p50 < 10 min discover→publish).
- Mean-time-to-correction tracked and published.
- Golden-fixture regression suite is CI-blocking.

### 7.5 Legal posture

- **Not investment advice:** factual presentation, disclaimers, no personalized recommendations; our copy never says "copy this trade."
- **Defamation:** neutral as-filed language ("disclosed a purchase of…"), published corrections policy with response SLA, contact channel as right-of-reply; extra care for strict jurisdictions (UK).
- **GDPR/privacy:** public-role disclosures on legitimate/public-interest basis; **per-regime redaction pass** removes out-of-scope personal data (home addresses, dependents' names) *before* publication; DSAR runbook.
- **Licensing:** free snapshots CC BY (attribution = brand distribution); API under commercial terms.

## 8. Infrastructure

- **Services:** Cloud Run `api`, `web`, `worker` — scale-to-zero. AWS equivalents: App Runner/Fargate, SQS, RDS, S3.
- **Data:** Cloud SQL Postgres (small, PITR on), GCS (versioned buckets: bronze, exports), Cloud Tasks, Cloud Scheduler, Secret Manager.
- **Edge:** Cloudflare DNS/CDN/WAF + coarse rate limits.
- **IaC/CI:** Terraform in `infra/`; GitHub Actions: `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` (pedantic set; `unwrap_used`/`expect_used` denied outside tests) → `cargo test --workspace` → conformance fixtures → web lint/typecheck/test → contract drift check (`regen && git diff --exit-code`) → deploy on main. Rust builds cached with cargo-chef layers + sccache. No staging (tagged zero-traffic Cloud Run revisions + feature flags instead) — a second production is a luxury a solo op shouldn't babysit.
- **Security:** least-privilege service accounts, private DB, signed URLs for Bronze, budget alerts.
- **Cost:** < ~$150/mo idle-ish; dominant variable cost is LLM extraction, which scales with success and is SHA-cached.

## 9. Monorepo & agent execution

```
govfolio/
  crates/core        # domain types (serde+schemars), value math, fingerprint, sqlx migrations
  crates/pipeline    # adapter trait, conformance harness, stage runners, idempotency
  crates/adapters/<jurisdiction>/  # one crate each: adapter.rs, schemas/, fixtures/
  crates/api         # /v1 (axum + sqlx + utoipa → emits openapi.json)
  crates/worker      # queue consumers; backfill/reprocess binaries
  apps/web           # Next.js SSR site + reviewer UI (TypeScript, consumes generated client)
  packages/contracts # GENERATED: openapi.json + TS client (drift-checked, never hand-edited)
  infra/             # terraform
  docs/plans/        # this doc + implementation plan
  docs/regimes/      # per-country methodology (public page + agent context, dual use)
  agents/            # CLAUDE.md tree, goals/, task templates
```

Language boundary as stated in §3: domain/data → `crates/*` (Rust); pixels → `apps/web` (TS); `packages/contracts` is the generated door between them.

**Agent-execution artifacts:**

1. **Context files:** root `CLAUDE.md` (project map, conventions, definition of done, guardrails) + per-package `CLAUDE.md`. `docs/regimes/<country>.md` doubles as adapter-building context.
2. **Goal files:** `agents/goals/NNN-slug.md` — objective, scope in/out, context pointers, **acceptance criteria as executable commands**. Ralph-loop compatible: repo is memory; a checklist file is updated and committed each iteration; loop ends when the commands pass.
3. **Guardrail lanes:** agents work on branches; CI gates merges; human-only: DB migrations, `terraform apply`, public claim-making copy (pricing/legal/methodology).

4. **Role authority files:** `agents/roles/<role>.md` — nine specified agents (Scout, Surveyor, Auditor, Sampler, Spec-writer, Test-designer, Planner, Builder, Sentinel), each with mission, required context, tool/cost budget, output contract, anti-patterns, and an eval calibrated against the E1 `us_house` ground truth (goal 016). Epochs gate on green role evals.
5. **Source Authority Files:** `docs/regimes/<x>/AUTHORITY.md` — the living, evidence-backed canon per source; loaded by every specialist before any source-scoped task; **write-back in the same PR is part of definition of done** (knowledge compounds in the repo, not transcripts). Rollout order in `agents/EPOCHS.md` (US → Brazil → Europe → Asia → Oceania).

**Task taxonomy by loop-friendliness:** adapters (fixture-gated — ideal), API endpoints (contract-test-gated), pipeline stages (integration-fixture-gated), UI (Playwright-gated), infra (human-gated).

## 10. Testing strategy

- Unit: `crates/core` via `cargo test` (schemas, fingerprint, query grammar, value-interval math on `rust_decimal` — money is decimal end-to-end, never floats).
- Conformance: every adapter vs golden fixtures (Silver + Gold expected outputs); runner `cargo run -p pipeline --bin conformance -- <adapter>`.
- Contract: axum responses validated against the emitted OpenAPI (`jsonschema` crate); generated TS client must compile; contract regen drift blocks merge.
- Integration: `#[sqlx::test]` against docker-compose Postgres; full pipeline run over fixtures (Bronze→Gold→outbox→delivery), rerun-idempotency asserted.
- E2E smoke: Playwright over critical web paths (profile page, search, record provenance, signup→alert).
- All CI-blocking. A parser change that breaks a historical fixture cannot merge.

## 11. Risks & mitigations

| Risk | Mitigation |
|---|---|
| US trading ban passes | Regime versioning; pivot weight to holdings, 7-day pre-sale notices (new signal), exempt asset classes, non-US jurisdictions. Transparency product unaffected. |
| Source layout changes silently | Drift detection → fail closed + review task; golden fixtures in CI. |
| LLM extraction cost spike | Deterministic-first; SHA caching; batch endpoints. |
| Defamation claim | As-filed language, provenance, corrections SLA, right-of-reply. |
| GDPR complaint | Per-regime redaction pass; DSAR runbook; public-interest basis documented. |
| Solo bus factor | Boring managed infra, everything in Terraform + git, runbooks in docs/. |
| Polyglot boundary drift (Rust ↔ TS) | Contract artifacts are generated-and-committed; CI regenerates and fails on diff; hand-editing `packages/contracts` forbidden. |
| SEO dependence | Email list + API customers as owned channels. |

## 12. Out of scope for v1 (deliberate YAGNI)

Market/price data + performance analytics/backtests (v2 flagship, alongside anomaly scoring), ML "suspicious timing" detection, executive-branch and sub-national officials (v1 = national legislators), mobile apps, Elasticsearch/Typesense, Redis, multi-region, localized UI (data is global; UI is English), staging environment.

---
*Next step per process: implementation plan (agent-ready goal decomposition) via the writing-plans workflow.*
