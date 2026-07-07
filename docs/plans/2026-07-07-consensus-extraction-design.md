# Govfolio.io — Consensus Extraction Design (goal 021 v2)

- **Date:** 2026-07-07
- **Status:** Approved (founder, 2026-07-07; approval recorded in `agents/goals/021-llm-extraction.md` § Phase 2). Produced as the committed terminal artifact of the goal-021-v2 brainstorming gate — the design dialogue itself concluded 2026-07-07 and is not re-run here.
- **Scope:** LLM extraction of scanned/paper disclosure documents — us_house first, platform strategy for every regime that hits the LLM seam. Extraction only: instrument resolution is out of scope and unchanged (invariant 3 governs it).
- **Relation:** Implements design §5.3 (parsing strategy) and §7.1 (verification lanes) of `docs/plans/2026-07-04-govfolio-design.md`; supersedes the goal-021-v1 fixed-confidence wrapper (`AnthropicExtractor` stamping a constant 0.9). Implementation plan: `docs/plans/2026-07-07-consensus-extraction.md`.

---

## 1. Problem and why

The verification state machine (§7.1) routes every record by `extraction_confidence` — it decides what publishes instantly, what gets sampled, and what a human must see. For the deterministic text path, confidence is mechanically derived from parse evidence. For the LLM path, v1 stamps a constant `0.9`, and the obvious alternative — asking the model how confident it is — is uncalibrated: research on safety-critical extraction consistently shows a majority of errors ship with self-reported confidence ≥ 0.8.

**Decision: consensus agreement is measured, never asserted.** Run N independent extraction samples, align their rows mechanically, and let the *measured agreement* — not the model's opinion, not a constant — set the confidence. This is the same mechanical-guardrail shape the automation policy prefers everywhere else, and it replaces the one non-mechanical trust input in the pipeline.

## 2. Decisions log (with rationale)

| # | Decision | Chosen | Rationale |
|---|---|---|---|
| D1 | Preprocessing library | **Pure-Rust `image` + `imageproc`; `pdfium-render` for rasterization** | No system deps → small deploy image, fast Cloud Run cold start, light CI. Host admin constraints were lifted (founder, 2026-07-07), so OpenCV is *possible* — rejected on deploy-image weight and cold-start, not host limits. Escalation trigger recorded: a fixture-proven fidelity gap upgrades to OpenCV and records why in the regime doc (mirrors the `pdf-extract` → `pdfium-render` pattern). pdfium dylib is runtime-discovered (bblanchon/pdfium-binaries, pinned version + checksum); an absent library is a typed error and the document fails closed. pdfium output is not byte-stable across builds — tests assert structure (page count, dimensions, non-blank), never raster SHAs; byte-determinism is proven on committed PNGs for the pure-image stages. |
| D2 | Raster scale | **`preprocess.max_edge = 1568` in `config/extractor.toml`** | 1568 px is the current useful long-edge cap for the sample-tier model (source URL + retrieval date recorded beside the value; re-verified at implementation time — provider caps are model-version-dependent). Margin-crop runs *before* resize so effective DPI on content is higher than the raw page scale. |
| D3 | API client shape | **Extend `crates/pipeline/src/extraction/` — no new crate; stay on forced tool use** | Single consumer; the `Transport` mock seam and retry machinery already live there; a crate split is churn without a second user. Forced tool use with local `jsonschema` re-validation is already built and equivalent in output shape to structured outputs — migrating is orthogonal churn, recorded as an available simplification, not taken. reqwest + rustls stands (no official Rust SDK — verified 2026-07-07). |
| D4 | Batch orchestration | **Polling worker bins, manual/local invocation; no terraform/scheduler in this goal** | Polling has no inbound auth surface and the Batch API's 24 h SLA fits backfill cadence. Cloud Scheduler wiring is deferred to the M8 goal (trigger: first real batch backfill run); scheduler stubs are created PAUSED per the deploy runbook regardless. All batch tasks sequence AFTER the sync path is green. |
| D5 | Per-pass raw retention | **Postgres `extraction_sample` table (per-pass payload + usage, PK `(sha256, consensus_tag, pass_idx)`); consensus result rides the existing `extraction_cache` under a composite tag** | The reviewer UI reads the database through the API — a GCS blob would add a second read path and IAM surface for small JSONB payloads. Held-row review tasks link competing payloads by `(sha256, consensus_tag)`; no jsonb column is added to `review_task` (API routes read that table). The samples double as the calibration dataset for future threshold tuning (retained, not acted on — see §7). |
| D6 | Sanity-check placement | **Cross-field temporal rule + top-band outlier → comparator code under `policy_version`; enum validity stays in the details contract** | JSON Schema (draft 2020-12 via schemars) cannot compare two date fields, so `notified_date ≥ transaction_date` cannot live in the contract. Split: contract = shape (machine-diffable via the committed schema snapshot), policy code = semantics needing cross-field comparison or thresholds. Deliberate bonus: the `us_house.transaction` schema snapshot stays byte-identical, so the E1-pinned artifact is not churned. |
| D7 | Confidence constants | **policy_v1 closed set {0.90, 0.75, 0.79}, committed and versioned** | Full mapping in §4. `AGREED` is exactly `0.9f32` so every pinned confidence literal in the E1 fixture corpus stays byte-identical through the cutover — the fixture diff reduces to extractor-tag strings. No path may emit ≥ 0.95: the deterministic auto-verify lane is unreachable from the LLM path by construction. |
| D8 | Escalation pass | **One fresh full-page premium extraction (default `claude-sonnet-5`), same schema prompt, temperature omitted** | A disagreement-focused prompt that shows the competing values would anchor the tiebreaker; a fresh independent pass is unbiased and reuses the same prompt asset. The comparator consults the premium output only on disputed fields. Exactly one premium call per document regardless of disputed-row count. The premium tier rejects non-default sampling params (400) — temperature is sent only to the sample tier, and only when configured. |
| D9 | `Extractor` trait | **Per-adapter trait signature frozen; consensus internals live in `pipeline::extraction`; ordinal reservation via `GoldCandidate.ordinal_override: Option<u32>`** | The v1 seam (`async fn extract(&self, doc, ctx) -> Result<Vec<StagingRow>>`) is implemented by four adapters — changing it is cross-adapter churn with no information gain, since held rows and audit data are pipeline-internal concerns. Publish uses `ordinal_override.unwrap_or(candidates_index)`: `None` everywhere existing preserves every published fingerprint; the consensus path sets the document ordinal so a held row's later resolution never shifts its siblings' fingerprints (invariant 4). |

## 3. Architecture

The seam is unchanged in scope: in the us_house adapter, `parse()` routes per document — text-layer success → the existing deterministic path, untouched; otherwise (scanned/paper PTR, ~10% of current P filings and most backfill volume) → the `Extractor` trait, now implemented by `ConsensusExtractor`.

### 3.1 Deterministic preprocessing (Rust)

`pdfium-render` rasterizes pages (text-layerless scans make rasterization mandatory; PDF-direct upload to the API is **rejected** — it surrenders deskew/contrast/scale control, and controlling those is the point of this stage). Pipeline per page: grayscale → deskew (projection-profile angle sweep) → adaptive/Otsu binarization → margin crop → resize longest edge to `preprocess.max_edge`. Geometric/optical normalization shrinks the variance of the stochastic stage and caps token spend; a few degrees of skew measurably craters extraction accuracy. Any preprocessing failure (including pdfium absent) freezes the document and opens a review_task (§5.6 semantics, unchanged).

### 3.2 ConsensusExtractor (N samples)

N = 3 independent samples (config), temperature ≈ 0.7 on the cheap sample tier. Temperature > 0 is the point: consensus over deliberately diverse sampling paths is the signal; temperature 0 yields three copies of one path and measures nothing. Model IDs, prices, and caps live in `config/extractor.toml`, never in code. The prompt is schema-constrained to a compact Silver-shaped DTO — short keys, the amount band as a closed enum over the printed PTR bands (ten bands A–J; paper column K is a separate over-$1M checkbox flag, **not** an eleventh band), owner/type as closed enums, `asset_description_raw` VERBATIM (invariant 2). Rust `normalize` owns band → `ValueInterval` decimal math (invariant 7); **the model never emits money values.** Each pass is locally re-validated against the JSON Schema; a schema-invalid pass is an error, not a vote (fail closed). Requests are structured so the shared prefix (system + schema + images) is cache-eligible: pass 1 fires alone, passes 2..N fire after it starts — noting that single-page documents sit under the sample tier's minimum cacheable prefix, so the cost model assumes no cache hits.

### 3.3 Comparator + mechanical confidence

Rows align across samples by **content key** (transaction date + normalized asset text + type + band) plus an occurrence index for duplicate keys — never by position: passes reorder rows, and positional diffing manufactures false disagreement and destabilizes fingerprints. Field-level agreement is scored over aligned rows; critical fields are band, type/side, dates, owner, asset text. A committed, versioned policy (§4) maps agreement → `extraction_confidence`. Fingerprint ordinals derive from consensus document order **including held rows** (ordinal reserved via `ordinal_override`), so a later-resolved row does not shift its siblings' fingerprints.

### 3.4 Routing and escalation (fail closed at every fork)

- Full agreement on all critical fields → publish `unverified` at the policy ceiling (0.90).
- Disagreement on any critical field → **one** premium escalation pass (D8). Premium + at least one sample agree with no remaining tie → publish at 0.75.
- Still ambiguous → **hold that row**: no GoldCandidate is emitted — publishing one of two diverging bands is a guess, and the spirit of invariant 3 forbids it. A review_task (`consensus_row_hold`) opens; the competing payloads are addressable in `extraction_sample`. Consensus rows of the same filing still publish — row-level granularity keeps the fast path fast without laundering uncertainty. A resolved hold re-enters through the existing review `Verdict::Edit` machinery carrying its reserved ordinal.
- Checkbox-shaped conflicts: a deterministic ROI ink-density check against template coordinates measured once from fixtures. The pixel primitive is regime-agnostic (pipeline); the form geometry, thresholds, and token vocabulary are regime knowledge and live in the adapter, entering through the sanity-check seam. A checkbox/model conflict **caps confidence to 0.79 (mandatory review) — it never rewrites the extracted value.** Azure Document Intelligence selection-marks are the documented, unbuilt upgrade path (a second cloud vendor for a fixed paper form fails ops-minimalism, design D4/D6); trigger: the ROI check demonstrably insufficient on fixtures.
- Programmatic sanity checks run before consensus math: `notified_date ≥ transaction_date`, enum validity (contract), top-band outlier → mandatory review regardless of agreement (§7.1 high-impact rule).
- Zero publishable rows, or preprocessing failure → adapter freeze + review_task (§5.6, unchanged).

### 3.5 Cost paths and cache

- **Tier-1 live scanned filings: 3 parallel-ish synchronous calls.** The Batch API's 24 h SLA violates the discover→publish p50 < 10 min SLO — batch is never used on the live path.
- **M8 backfill: Batch API** at the 50% discount; submissions sized against the per-action token ceiling and the billing HARD CAP (over cap = halt, per automation-policy). Cap *values* do not exist yet (founder-deferred to funding) — the submit bin fails closed while they are unset. `expired` batch items are retryable, never holds.
- SHA cache keyed `(document_sha256, consensus_tag, composite_model_id)` — a document version is paid for exactly once; a cache hit short-circuits before any API call (§5.3.4). The composite encodes sample model + N + temperature, escalation model, prompt_version, and policy_version; `extracted_by` carries the same composite (§4.1 provenance). Bumping any component re-extracts and **supersedes** prior rows through the normal reprocess machinery — never mutates.
- `pipeline_run.stats` (parse-stage run row) gains token counts, cost estimate (rust_decimal from the config pricing table — no float money), pass count, and agreement metrics per run: mechanical cap enforcement plus the dataset for future threshold calibration against the monthly audit.
- Retry/backoff with jitter on 408/429/5xx; semaphore-bounded concurrency; API keys via `ANTHROPIC_API_KEY` env locally / Secret Manager in cloud — never in repo.

### 3.6 Independence honesty

Same-model samples share bias: temperature decorrelates token paths, not model priors — a model that misreads a faded checkbox can misread it identically three times. Therefore the **second-model cross-check for high-impact rows stays** (≥ $500,001 bands, watchlist filers, §5.3.3) as bias decorrelation, and the monthly sampled audit remains the only ground truth — the world verifies the model. Consensus sets *scrutiny level*; it never asserts truth.

### 3.7 Testing strategy (offline, deterministic CI)

Conformance never calls a live model: the scanned fixture passes offline through the committed, mechanically-primed extraction cache, exactly as in v1 but under the new tag. Comparator and routing are unit-tested on synthetic disagreement matrices through a scripted mock transport. An e2e over the scanned fixture asserts Silver rows, confidence values, a review_task on a planted disagreement, and idempotent rerun (second run inserts nothing). Exactly **one** live smoke test exists — the v1 `#[ignore = "needs ANTHROPIC_API_KEY"]` test repurposed to drive the full consensus path; never a sibling.

## 4. Confidence policy (policy_v1) and §7.1 lane mapping

| Outcome | `extraction_confidence` | §7.1 lane | Action |
|---|---|---|---|
| 3/3 agreement on all critical fields, sanity green | **0.90** (exactly `0.9f32`) | Sampled spot-check (~0.8–0.95) | publish `unverified` |
| Escalation resolves (premium + ≥1 sample agree, no tie) | **0.75** | Mandatory review (<0.8) | publish `unverified` + `consensus_mandatory_review` task |
| Sanity fail / top-band outlier / ROI-checkbox conflict | **0.79** (forced cap) | Mandatory review | publish + review_task; value never rewritten |
| Still ambiguous after escalation | — | — | hold row: no candidate, ordinal reserved, `consensus_row_hold` task, competing payloads retained |
| Zero publishable rows / preprocessing failure | — | — | freeze document + review_task (§5.6) |

High-impact rows (≥ $500,001 bands, watchlist) additionally keep the second-model cross-check regardless of agreement. The closed set {0.90, 0.75, 0.79} is asserted in tests; the acceptance gate checks set membership, not a threshold — a tampered 0.85 fails closed.

## 5. Cost model (directional; authoritative numbers live in `config/extractor.toml` with source + date)

Per scanned PTR (2 pages, no cache assumed): 3 samples × (~3k image + ~1.5k prompt tokens in / ~1k out) ≈ 13.5k in / 3k out → ≈ $0.03 sync on the sample tier; escalation adds ≈ $0.04–0.05 on ~10–20% of documents; batch halves everything → ≈ $0.015–0.03 per document. M8 ballpark: 50k scanned documents ≈ $750–1,500 via batch. Actuals land in `pipeline_run.stats.extraction` per run.

## 6. Failure modes

| Fork | Behavior |
|---|---|
| pdfium missing / raster fails | typed error → freeze doc + review_task |
| A sample pass schema-invalid | extraction errs (a bad vote is not a vote) |
| Transport 408/429/5xx after bounded retries | error propagates → freeze doc path |
| All rows held | zero publishable rows → freeze doc + review_task |
| Budget keys unset | batch submit refuses before any API call |
| Cache poisoned / confidence outside policy set | `validated()` set-membership check fails closed |

## 7. Deferred (each with its recorded trigger)

- **Test-Time Augmentation ensembles + Needleman–Wunsch character voting** — they reduce escalations, not errors; fail-closed already catches what they resolve. Trigger: held-row review volume materially hurts.
- **Azure DI selection-marks** — trigger: ROI ink-density check demonstrably insufficient on fixtures.
- **Non-us_house regimes** — us_senate paper GIFs et al. adopt the same platform pieces per their own goals.
- **Watchlist population** — `WATCHLIST_POLITICIANS` stays a stub until the watchlist product surface exists.
- **Automated threshold re-tuning** — the `extraction_sample` dataset is retained for it; nothing is built.
- **Batch scheduler/terraform** — M8 goal; trigger: first real batch backfill run.
