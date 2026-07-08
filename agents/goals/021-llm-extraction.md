# 021 — LLM extraction fallback

> Phase 2 (consensus expansion) appended 2026-07-07 — see `## Phase 2` below. The v1 record
> under `## Objective`…`## BLOCKED` is history; do not edit it.

## Objective
Implement the extractor interface stubbed in plan Task 8: schema-constrained LLM extraction for low-confidence/scanned PDFs, sha-cached, confidence-scored, second-model cross-check for high-impact rows.

## Context (read first)
- design §5.3, §4.3 · crates/pipeline/src/adapter.rs

## Acceptance criteria
```bash
cargo test -p pipeline extraction
cargo run -p pipeline --bin conformance -- us_house   # scanned fixture case goes green
```

## Checklist
- [x] extractor iface impl  - [x] cache by sha+version  - [x] confidence  - [x] cross-check  - [x] scanned fixture
  - second leg DONE 2026-07-05 (rust-builder): `pipeline::extraction`
    (`AnthropicExtractor` machinery: reqwest+rustls Messages client, no SDK, forced
    tool use with the silver-row schema as `input_schema` + local jsonschema
    re-validation, retries w/ exponential backoff, key never logged —
    `HttpTransport` Debug redacts; models env-overridable, defaults
    `claude-haiku-4-5-20251001` primary / `claude-sonnet-5` cross-check);
    cache key `(document_sha256, extractor_tag, model_id)` in two tiers (committed
    in-case `extraction.cache.json` + pg `extraction_cache`, migration 0004
    expand-only, migrate pin 4→5); conformance cache PRIMED MECHANICALLY from
    expected.silver.json via `prime_from_expected_silver` (provenance records
    ground truth 77740d8, equality enforced by
    `cargo test -p pipeline extraction_cache_entry…`); confidence 0.9 f32 stamped,
    <0.9 or schema-invalid cache entries fail closed; cross-check per SAF §6.3
    (bands ≥ $500,001 — NOTE: dispatch prompt said $1M, SAF is authoritative —
    + watchlist stub `WATCHLIST_POLITICIANS: &[] `), field-level compare, mismatch =
    `CrossCheckMismatch` + `llm_crosscheck_mismatch` review_task + freeze; live
    test `#[ignore = "needs ANTHROPIC_API_KEY"]` (skips loudly without key);
    scanned fixture moved into `fixtures/`, staged MANIFEST entry + conformance
    ids applied, `conformance -- us_house` 5/5 GREEN OFFLINE, e2e 4→5 filings
    (13 gold/outbox rows; paper filer resolves via new prefix-less canonical
    alias seeding in `seed_roster`); clerk-stamp date parser (`2026 MAY -6` →
    2026-05-06) for signed/filed dates. Live-mode note: on a cache miss the DocID
    is threaded from `raw_document.source_url` (§2.3 URL shape) since the paper
    form prints no Filing ID; a live extraction without pool/indexed URL fails
    closed.
  - E1 lock SUPERSEDED to v2 (2026-07-05, per this goal's founder-approved
    BLOCKED staging): `E1.lock.json` now `version: 2`,
    `supersedes: b4238f01962cde59bdf459ec7d2d84949cc02d428fc5160667af3d178c4c1c1d`
    (sha256 of the v1 lock file's LF bytes, verified against `git show HEAD:`),
    reason + date recorded in the lock; v2 re-pins the two amended files
    (fixtures/MANIFEST.json, docs/regimes/us-house.md — staged write-backs
    applied: open question resolved, §7 row 5, E13, paper-anatomy quirks) and
    adds pins for the scanned case (input/silver/gold + ground-truth-derived
    extraction.cache.json); all other v1 pins byte-identical.
    `pipeline::evals::reference` verifies the supersession trail (v2+ without
    supersedes/reason/date fails closed); rust-builder scorer marker 4/4→5/5.
  - first leg DONE 2026-07-05 (test-designer): fixture captured at
    `crates/adapters/us_house/fixtures-llm/scanned_paper_ptr/` (DocID 9115811, sha
    `2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c`, text layer proven
    absent) with independent visual-transcription expecteds; capture record + paper-form
    conventions + flagged uncertainties in `crates/adapters/us_house/fixtures-llm/MANIFEST.json`.
    Parked in `fixtures-llm/` because conformance `run_cases` + `e2e_local.rs` (asserts 4 dirs)
    + `factory.rs` (cases<->dirs bijection) auto-discover `fixtures/` — red-CI guard.

## BLOCKED — E1 lock supersede needed before second leg lands (2026-07-05, test-designer)
Context: `docs/regimes/us-house/reference/E1.lock.json` sha-pins `fixtures/MANIFEST.json`
and `docs/regimes/us-house.md`; supersede is founder-gated per
`docs/decisions/role-eval-thresholds.md` (and test-designer is a SCORED role — must not
amend its own reference corpus). The first-leg SAF write-backs are therefore RECORDED but
NOT applied to the pinned files. Whoever supersedes the lock (v2 bump + note) applies:
1. `docs/regimes/us-house.md`: resolve open question "Do paper PTRs have any text layer?"
   (answer: NO — pdftotext emits 1 byte, a lone form-feed, on E13/9115811); add evidence row
   E13 (9115811.pdf sha above, retrieval log `evidence/f312caf4….retrieval.json`); add §7 row 5
   (scanned paper PTR) + quirks entry for paper-form anatomy (no Filing ID, NAME lacks `Hon.`,
   no signature block — clerk received stamp only, no cap-gains column, no [XX] codes,
   checkbox vocabulary) — full text staged in `fixtures-llm/MANIFEST.json`.
2. `fixtures/MANIFEST.json`: move the scanned_paper_ptr entry from `fixtures-llm/MANIFEST.json`
   into `cases`, add `Diana Harshbarger|TN01 -> 0HSEMBR0000000000000000005` and
   `9115811 -> 0HSEFNG0000000000009115811` to conformance_ids.
Builder second leg (same PR as, or after, the lock supersede): implement the seam, prime the
sha-keyed cache from expected.silver.json (case is LLM-path: NO text parse), add the ULID
mapping to normalize.rs, move the case dir into `fixtures/`, bump e2e's expected case count
4 -> 5, tick the scanned-fixture box when `conformance -- us_house` prints 5/5 green.
Options considered: (a) supersede lock autonomously — rejected (founder-gated + scored role
amending own reference); (b) skip write-back recording — rejected (SAF discipline);
(c) record here + fixtures-llm manifest, defer pinned-file edits to the lock supersede — CHOSEN.

## Phase 2 — Consensus extraction expansion (founder-approved 2026-07-07)

**Approval record.** The founder approved the consensus-extraction architecture on 2026-07-07
and issued the Phase-2 goal text as a planning-session prompt (brainstorming gate SATISFIED;
the committed design doc is that gate's terminal artifact). This section is the durable
approval record cited by the design doc, the implementation plan, and the E1 lock v4
supersession reason.

**Why.** v1 stamps a constant 0.9 `extraction_confidence` (the one non-mechanical trust input
in the pipeline; design §7.1 keys the verification lanes on it). Phase 2 replaces it with
measured cross-sample agreement: N-sample cheap-model extraction → mechanical comparator →
deterministic policy_v1 confidence (closed set {0.90, 0.75, 0.79}) → routing
(publish `unverified` / escalate once / hold row + review_task), plus deterministic image
preprocessing (pdfium raster, PDF-direct rejected), `config/extractor.toml`
(config-not-code), cost instrumentation in `pipeline_run.stats`, and a Batch-API path for
M8 backfill. Fail closed at every fork. Consensus sets scrutiny level; it never asserts truth
(second-model cross-check for high-impact rows retained; monthly sampled audit remains ground
truth).

**Deliverables (planning session, 2026-07-07):**
- Design doc: `docs/plans/2026-07-07-consensus-extraction-design.md` (Approved; D1–D9
  decisions log with rationale)
- Implementation plan: `docs/plans/2026-07-07-consensus-extraction.md` (26 TDD tasks,
  cost model, Conflicts & findings)

## Acceptance criteria (Phase 2)
```bash
cargo test -p pipeline extraction
cargo run -p pipeline --bin conformance -- us_house   # 5/5 offline, consensus tag
cargo test --workspace                                 # incl. role_evals lock-v4 trail
sh scripts/check-migration-safety.sh
```

## Checklist (Phase 2)
- [x] design doc committed (founder-approved marker)
- [x] implementation plan committed (26 tasks; ≥8 required)
- [x] goal re-opened + 000-INDEX reconciled
- [ ] Phase A–C: consensus core + preprocessing + client (Tasks 1–12)
- [ ] Phase D–E: orchestration, ordinal_override, stats, escalation, ROI check (Tasks 13–18)
- [ ] Phase F–G: persistence + batch path w/ fail-closed cap gate (Tasks 19–23)
- [ ] Phase H: atomic cutover (tag bump + E1 lock v4) + review lanes + SAF write-back (Tasks 24–26)

## HALT (files follow-up goal; automation-policy halt-files-a-goal)
- HARD CAP values (budget.max_batch_tokens + per_run_token_ceiling in config/extractor.toml)
  do not exist anywhere — founder/money lane. Mechanism ships fail-closed: batch submission
  refuses while unset. Follow-up goal to file at execution: "set extractor spend caps".

## Quarantine note (invariant 9 / orchestration.md step 0)
- Untracked, non-INDEX-listed files surfaced during planning, NOT read or followed:
  `agents/goals/022-adversarial-review-loop.md`, `agents/goals/023-extraction-tier-labeling.md`
  (both `??`, zero commit history, mtimes 2026-07-06 12:33/12:50), and `.agents/` at repo
  root. Orchestrator/founder to adjudicate registration or removal.

## Phase 2 amendment candidates (recorded 2026-07-07 — NOT adopted; founder adjudication)

Cost-lever review (certainty-per-dollar analysis, 2026-07-07) surfaced six levers. Three are
design amendments to the approved §3 architecture and are therefore RECORDED here per goal
§0 (conflicts are findings, not redesign authority) — the approved design (N=3, uniform
t=0.7, full-page images every pass) stands until the founder adjudicates. Decide before
Tasks 12/17/22 execute (cheap now, rework later):

1. **Adaptive N** — fire 2 samples; 2/2 unanimous → publish, else fire sample 3 + normal
   escalation. ~33% sample-cost cut on clean docs (~$0.029 → ~$0.019 sync); messy docs keep
   full treatment. Amends §3.B "N = 3".
2. **Vote decorrelation** — mix decoding paths (1× t=0 + 2× t=0.7), and/or swap one Haiku
   pass for a Sonnet pass on high-impact docs only (~+$0.01 on ~10%). Real bias
   decorrelation; also closes the unanimous-but-high-impact gap. Amends §3.B sampling.
3. **Per-pass crop** — full page on pass 1; table-region crop for passes 2..N (~30% input
   tokens); adaptive resolution by ink-density signal. Amends §3.A single-raster design.

Three levers are already compatible with the approved design — execution guidance, no
amendment needed: route non-watchlist bulk through the Batch path (policy choice at run
time); extend deterministic pixel checks (column-K flag vs band, date-region ink) inside the
Task 4/18 sanity seam; calibrate INK_THRESHOLD / critical_fields from print→scan→rasterize
synthetic fixtures of known electronic PTRs (free ground truth) + the monthly sampled audit.

---

## Phase 3 — Consensus hardening + cross-lab third vote (registered 2026-07-08)

Founder-issued 2026-07-07 as a fresh-session planning prompt; recorded here verbatim per
invariant 9 (goal files are executable instructions; the queue stays self-contained).
Findings in its §3 are FOUNDER-APPROVED by issuance. Planning session ran 2026-07-08;
deliverables: `docs/plans/2026-07-07-consensus-extraction-amendment-1.md` (design
amendment) + `docs/plans/2026-07-07-consensus-hardening.md` (addendum plan, Tasks 27+)
+ surgical edits to `docs/plans/2026-07-07-consensus-extraction.md`.

### Phase 3 goal text (verbatim)
### Goal 021 — Phase 3: Consensus hardening + cross-lab third vote — PLANNING phase

Status: OPEN · Phase: P (plan) · Role: planner (skills: plan-decomposition + @superpowers:writing-plans)
Register in `agents/goals/000-INDEX.md` before acting (invariant 9; goal numbers 022/023 are
quarantined untracked files — do not reuse or read them). Usable verbatim as a fresh-session
prompt at the repo root; work on a branch per house convention.

---

#### 0. Session contract

- **This session PLANS. It writes no production code.** Deliverables are documents (§1);
  execution follows via @superpowers:executing-plans or subagent-driven-development.
- **Approval status: the findings in §3 are FOUNDER-APPROVED by the issuance of this goal**
  (2026-07-07). They come from two adversarially-verified reviews run against the committed
  Phase-2 plan (a 30-agent correctness-per-dollar pass and a 12-agent cross-lab model
  research pass; every load-bearing claim was primary-source-verified 2026-07-07).
  Conclusions are inlined below so this file is self-contained. Do not re-run the analysis;
  do not silently re-open settled decisions. Genuine conflicts with the committed plan or
  invariants go in the plan's **Conflicts & findings** section — findings, not redesign.
- **Ambiguity protocol** (automation-policy): blocking unknown = HALT item recorded in the
  plan (files a follow-up goal); non-blocking unknown = labeled assumption with rationale.
- Every planner decision carries a one-line *why*.

#### 1. Objective & deliverables

Amend the committed Phase-2 consensus-extraction work (design doc
`docs/plans/2026-07-07-consensus-extraction-design.md`, plan
`docs/plans/2026-07-07-consensus-extraction.md`, 26 tasks, execution IN FLIGHT) with the
approved findings of §3. Produce:

1. **Design amendment** — `docs/plans/<session-date>-consensus-extraction-amendment-1.md`
   in house design-doc voice, marked *Approved (founder, 2026-07-07, recorded in this goal)*:
   the §3 content restated, including the family-aware voting change to policy semantics and
   the cross-lab third-vote mechanism. Commit FIRST. Never rewrite the original design doc's
   history — this is an amendment document; add a one-line amendment pointer at the top of
   the original.
2. **Plan amendment** — either an addendum plan `docs/plans/<session-date>-consensus-
   hardening.md` (new `### Task N` sections) plus surgical edits to not-yet-executed tasks
   of the committed plan, or a single consolidated amendment plan — planner decides (D1)
   with rationale. Same writing-plans discipline: failing test first, exact paths, complete
   pivotal-test code, exact commands + expected output, commit per task.
3. Goal-file + 000-INDEX reconciliation (this phase registered; checklist added).

**Execution-frontier constraint:** Phase-2 execution has begun (Task 1 consensus DTOs landed
or in flight; check `git log` + `crates/pipeline/src/extraction/consensus.rs`). Amendments to
ALREADY-LANDED code are code-amendment tasks (with a single consolidated `policy_version`
bump, D2); amendments to un-executed tasks are plan edits. Never retro-edit a landed task's
plan section — mark it superseded and point to the amendment task.

#### 2. Context to load (in order)

1. `/CLAUDE.md` — invariants 1–10
2. `docs/plans/2026-07-07-consensus-extraction-design.md` — the architecture being amended
3. `docs/plans/2026-07-07-consensus-extraction.md` — esp. Global Constraints, Tasks 1–4, 6,
   9, 12–13, 16–18, 20–24 (the amendment targets), Conflicts & findings
4. `agents/goals/021-llm-extraction.md` — Phase 2 section + amendment-candidates section
5. `docs/regimes/us-house.md` §3, §6, §7 · `docs/regimes/us_house/AUTHORITY.md`
6. `docs/decisions/automation-policy.md` — HALT semantics, billing HARD CAP
7. `agents/LOOP.md` · `agents/goals/000-INDEX.md`

#### 3. Approved findings (plan against these; rationale inline)

##### Tier 0 — defects in the committed plan (bug-fix class; fix before or at the affected task)

1. **Occurrence-aware premium matching.** `resolve_disputed`/`premium_row_at` hardcode
   occurrence 0 and `RowVerdict::Disputed` carries no RowKey — duplicate-lot rows (same
   asset/date, real PTR pattern) silently cross-match the premium's first occurrence and can
   PUBLISH swapped owner/notification-date at 0.75. Fix: thread the aligned RowKey (incl.
   occurrence) through `RowVerdict::Disputed` into escalation matching; premium rows get
   occurrence indexes via the same counter `align()` uses. Touches Tasks 3/4/17 (+23 reuse).
2. **Document-order emission contract.** Add to the static prompt: rows strictly in printed
   top-to-bottom order, continuing across pages; emit each printed row exactly once, in
   printed order (phrasing chosen to avoid inducing phantom duplicates on band-wrap page
   breaks). Why: occurrence-index pairing and ordinal derivation are load-bearing but only
   implicitly assumed; cross-ordered duplicates can publish chimera owner/date rows; silent
   dedup drops real transactions. ~40 tokens, Task 18's prompt string.
3. **Row-count completeness gate.** Horizontal-rule projection profile (Task 6 primitive
   family) estimates populated table rows per page; mismatch vs consensus row count (either
   direction) → doc-level review_task `row_count_mismatch` + 0.79 cap (doc-level — the row
   DTO carries no page attribution). Why: consensus is structurally blind to correlated
   omission — three passes skipping the same faint row publishes "complete" at 0.90; this is
   the only recall gate in the system. CPU-only, ~$0.
4. **Form-revision template fingerprint guard.** Before ANY coordinate-based pixel check:
   classify the printed template by ink-fingerprinting static template regions against
   per-revision fingerprints measured from fixtures; unknown template → pixel checks emit
   no votes and no caps, log `template_unrecognized` in stats. Why: all ROI geometry is
   calibrated from a single 2026 fixture; 2012+ backfill forms differ — blind application
   floods review with false 0.79 caps or silently voids the checks. Per-revision coordinate
   sets are regime knowledge (us_house adapter).

##### Tier 1 — adopt-compatible, ~$0 (approved outright)

5. **Few-shot worked example from the committed 9115811 fixture** in the cache-eligible
   static prefix (expected transcription already exists, parser-blind). Targets the worst
   class: correlated 3/3-agree misconventions (hallucinated SP on blank owner, date-column
   swap, clerk-stamp misparse) that ship at 0.90 — the class temperature diversity cannot
   touch. Conditions: example ≥ ~1.1k tokens (pushes 1-page docs over Haiku's 4096-token
   cache minimum → roughly cost-neutral); "worked example of FORMAT, not this document"
   framing; fixture-literal leak check via the sanity seam; exclude 9115811 from future
   calibration sampling; land before mass backfill (prompt_version bump supersedes).
6. **Field-wise modal publication — Agreed branch ONLY.** `score()` publishes
   `candidates[0]`; a 1-of-3 misread of any unvoted secondary field (filing_status_raw —
   drives amendment supersession — cap_gains, vehicle fields) ships at 0.90. Publish the
   per-field modal (canonical-plane) value across the 3 candidates; 3-way tie → first
   sample, recorded in stats. The escalation-resolved branch is OUT (Disputed carries
   deduped candidates — vote multiplicity destroyed; do not rework that contract here).
7. **Escalation-pass hardening.** Codify: never send `thinking` config to the premium pass
   (Sonnet 5 omit = adaptive); size escalation max_tokens so thinking never starves the tool
   call (assert `stop_reason == tool_use` in the live smoke); expose `output_config.effort`
   as an extractor.toml knob. ~$0 (design cost table already prices adaptive thinking).
8. **Confidence-lane-aware stratified audit.** The 0.90 LLM lane is the only lane no human
   reviews and the uniform sampler gives it ~10 rows/month — statistically void. Add config
   sampling weights keyed (extraction path, confidence); oversample LLM-0.90 heavily; keep
   deterministic-seeded idempotent draw + a nonzero deterministic-lane floor; precision
   report MUST apply inclusion-probability weights (ship together or the SLO number biases).
9. **Zero-friction error labels + drift sentinel.** (a) Mechanical field-diff on every
   `Verdict::Edit` resolution (0.75/0.79 lanes are 100% reviewed = free ground truth);
   (b) expand-only migration adding `discrepancy_fields jsonb` + `error_class` closed
   vocabularies to sample_audit; key both by the `extracted_by` composite tag. (c)
   Report-only worker bin sweeping weekly agreement/escalation/hold/schema-invalid rates
   vs trailing baseline (thresholds in extractor.toml); breach → review_task; log per-field
   premium-vs-majority agreement from every escalation (free ongoing cross-model probe).
   Labels only — auto-demotion from lanes is explicitly out of scope.
10. **Shadow consensus eval harness.** Manual worker bin: electronic PTRs (deterministic
    Silver = ground truth) → rasterize → full consensus path via Batch under isolated tag
    `shadow@1` → per-field confusion matrices bucketed by consensus outcome. Measures
    P(wrong | 3/3 agree) — the design's admitted blind spot — before the 50k backfill.
    One-time ~$100 pilot / ~$1k full sweep, behind the HARD-CAP fail-closed gate. Plus the
    refill arm: programmatically fill the blank paper template with known values for
    checkbox ground truth at scale. **This harness doubles as the §D bake-off rig.**

##### Tier 2 — approved design amendments (this goal's issuance = the adjudication)

11. **Strict closed-vocabulary tool schema; band as column letter.** Replace free-String
    DTO fields with schemars enums; `strict: true` tool definition; band emitted as
    `band_column: enum A..J` + `over_1m_spouse_dc: bool` (column K structurally cannot
    contaminate the band); Rust maps letter→band string via `tables::BANDS`. Date `pattern`
    regex lives ONLY in local re-validation (API-side strict schemas strip `pattern`).
    Why: alignment keys become byte-identical across passes (format noise currently breaks
    alignment and can cross-pair rows into false agreement); band becomes a positional task
    (decorrelates shared transcription errors); pixel check becomes index-to-index exact.
12. **Drop voted fields (band, type) from the alignment key** — key on
    transaction_date + normalized asset text only (occurrence index handles duplicates).
    Why (verified): with band/type in key_fields the designed premium tiebreak is
    UNREACHABLE for band disputes (key-splits mask them — 2v2 ties publish at 0.75) and
    every 1/3 band misread spawns a phantom hold whose `Verdict::Edit` resolution inserts a
    duplicate Gold row. Restores §3.4/§4 escalation semantics; two-line spec change +
    policy bump.
13. **Two-plane comparison: canonical compare, verbatim publish.** Per-field-class
    canonicalization (NFKC/casefold/whitespace/dash for asset text; date parse to ISO with
    fail-closed fallback) used ONLY in the comparison plane (keys, agreement, premium
    matching); the published value is always one of the model's own verbatim strings (modal
    verbatim among canonical-equals; deterministic tie-break). Kills false disagreements
    and a deterministic false-hold (premium's cosmetic variance fails exact lookup).
    Invariant 2 intact by construction; log both planes into extraction_sample.
14. **Vote-margin-stratified escalation acceptance.** Publish at 0.75 only when the winning
    value has ≥3 of the 4 readers (samples + premium). 1/1/1-scatter + premium match =
    2-of-4 → HOLD, not publish. (Wording matters: "strict majority ≥3 of 4"; "plurality"
    is what the code already does and would be a no-op.) Strictly fail-closed, $0.
15. **Pixel-ambiguity-triggered premium vote on unanimous rows.** Ink density in a
    calibrated gray zone around INK_THRESHOLD, or two adjacent boxes above noise floor →
    the (single, shared) premium pass fires even on 3/3 agreement; premium concurs → 0.90
    stands; dissents → 0.79 / hold. Pixel signal selects scrutiny, never a value.
    ~+$0.001–0.004/doc. Amends D8's trigger and adds a policy row.
16. **Quality-routed vote sets.** Free preprocess by-products (residual skew, Otsu
    between-class variance, noise count) → `QualityMetrics`; thresholds in extractor.toml;
    flagged docs run 3 Haiku + premium up front, and 0.90 additionally requires premium
    concordance on flagged docs. ~+$0.003–0.005/doc sync, halved in batch. Thresholds
    versioned in the composite tag; calibrate on committed PNGs (pdfium not byte-stable).
17. **Error-boundary fixture corpus with a REAL regression gate.** Every audit-confirmed
    published error becomes a committed fixture; the gate for model/prompt/N bumps is a
    key-gated LIVE eval bin over that corpus (outside CI — conformance's primed-cache gate
    is green by construction and can't catch model regressions); policy-only bumps re-score
    stored extraction_sample payloads offline. Defined pass semantics + flake policy.

##### D — Cross-lab third vote (approved mechanism; model selection is bake-off-gated)

**Decision: one of the three sample votes SHOULD come from a different lab**, contingent on
bake-off evidence, with **family-aware voting**. Verified shortlist (all prices/facts
primary-sourced 2026-07-07; re-verify at implementation — check the provider deprecations
page before committing to any model):

| Candidate | Access | $/pass sync / batch | Notes |
|---|---|---|---|
| Gemini 3.1 Flash-Lite | **Vertex AI** (IAM/ADC, existing GCP billing — zero new vendor) | $0.0026 / $0.0013 | responseSchema incl. in 50% batch; temperature MUST stay 1.0 (Gemini 3 warns <1.0); earliest shutdown 2027-05-07 → schedule requalification; Lite OCR on degraded scans unproven |
| gpt-5.4-mini | OpenAI direct (new vendor: key+billing+ToS) | $0.0093 / $0.0046 | Decode-time strict-schema guarantee (best); pre-resize pages ≤1408px (mini's 1,536-patch budget downscales 1568px ~9%); temperature needs `reasoning_effort:"none"` smoke test; batch prefers hosted GCS URLs |
| Qwen3-VL-235B | DeepInfra (new vendor) | $0.0018 / $0.0014 (batch only −20%) | Open weights = no deprecation; max lineage distance; strict-schema adherence + images-in-batch unverified |
| Mistral Small 4 | Mistral direct (new vendor, EU) | $0.0013 / $0.0006 | Direct temp-0.7 mapping; vision+schema pairing undocumented |

- **Family-aware voting (policy change, load-bearing):** decorrelation literature
  (arXiv 2506.07962, 2605.29800, 2603.17111) shows cross-lab panels collapse to ~2
  effective independent votes and same-family models agree ~60% when both err. Therefore:
  2×Haiku agreement + cross-lab dissent on a critical field = DISPUTE (escalate), never a
  2/3 publish. Cross-lab concurrence keeps 0.90 (ceiling unchanged; the gain is fewer wrong
  0.90s, not higher confidence).
- **Bake-off** rides the §10 shadow-eval harness: per-field accuracy on degraded scans,
  schema-violation reject rate, and MEASURED error correlation vs the Haiku votes — select
  on decorrelation as much as accuracy. Router (OpenRouter, `require_parameters:true`, ZDR
  routing) is acceptable for the bake-off ONLY; production is always direct APIs (no batch
  discount via routers, 5.5% fee, no SLA).
- **Ops preference order:** Vertex path first (no new vendor, HARD CAP stays single-cloud);
  any non-Google winner adds a vendor ToS review = **founder legal lane (HALT item)**.
  Never a free tier anywhere (free tiers train on inputs). Per-vendor sampling conventions
  live in extractor.toml (temperature is not portable across labs).

#### 4. Decision points — planner resolves-with-rationale or HALTs

| # | Decision |
|---|---|
| D1 | Amendment vehicle: addendum plan + surgical edits vs one consolidated amendment plan; how superseded committed-task sections are marked |
| D2 | policy_version strategy: single pol2 bump covering ALL comparator changes (11–14, family-aware) vs staged bumps; cache/supersession + E1-lock consequences (another lock supersession is expected — same mechanical trail as v4) |
| D3 | Sequencing vs the execution frontier: which Tier-0/2 items merge into not-yet-executed tasks vs become post-cutover amendment tasks; nothing may leave conformance red between commits |
| D4 | Bake-off scope: which 2–3 shortlist models; harness = shadow-eval bin extension; success thresholds for promotion (decorrelation + accuracy + adherence) |
| D5 | Family-aware policy table: exact vote-counting semantics with a foreign vote present (incl. interaction with 14's ≥3-of-4 rule and 15/16's premium triggers — one premium call per doc invariant must survive) |
| D6 | Vendor client shape: second Transport-style client behind the existing seam; config schema for per-vendor sampling/media params |
| D7 | Which findings write back to us-house.md §6 vs AUTHORITY.md quirks (SAF discipline, same PR) |

#### 5. Hard constraints

All ten CLAUDE.md invariants. Plus, carried from Phase 2 and extended: closed confidence
set (values may be ADDED only by policy_version bump with founder-recorded rationale — this
goal adds none); LLM rows never auto-verify; deterministic checks cap/route, never rewrite
a field; CI offline + deterministic, exactly one key-gated live smoke (the live regression
eval bin of §17 and the bake-off bin are manual, key-gated, never CI); batch behind the
fail-closed HARD-CAP gate (values still unset — HALT stands); verbatim publish plane
(invariant 2) under §13; model never emits money (§11 strengthens this — letters, not
amounts); no router in production; no free-tier keys; no model with a scheduled shutdown
inside the backfill horizon.

#### 6. Acceptance (this planning session)

```bash
test -f docs/plans/*-consensus-extraction-amendment-1.md
test -f docs/plans/*-consensus-hardening.md            # or consolidated per D1
grep -c '^### Task' docs/plans/*-consensus-hardening.md # expect ≥ 10
git diff --name-only <base>..HEAD                       # only docs/ + agents/ paths
```

- [ ] Design amendment committed first, founder-approval marker, family-aware policy table present
- [ ] Every task: failing test first, red/green commands + expected output, commit msg
- [ ] D1–D7 resolved-with-rationale or HALT
- [ ] Tier 0 items scheduled ahead of (or into) the tasks they defend
- [ ] Bake-off gate defined with numeric promotion thresholds
- [ ] HALT items carried: HARD CAP values; non-Google vendor ToS (conditional)
- [ ] Conflicts & findings section present; goal + 000-INDEX reconciled; handoff offered

#### 7. Anti-patterns (auditor rejects on sight)

Flat vote counting across model families · publishing 2-same-family-votes over a cross-lab
dissent · router in the production path · free-tier API usage · committing to a model with a
published shutdown inside the backfill window · temperature <1.0 sent to Gemini 3.x ·
`pattern`/unsupported keywords in API-side strict schemas · skipping the bake-off gate ·
retro-editing landed code without a policy_version bump · a second premium call per document ·
values invented for the HARD CAP.

### Checklist (Phase 3)

Planning session (2026-07-08):
- [x] Phase 3 registered in 000-INDEX + this goal file (this commit)
- [x] design amendment committed FIRST (amendment-1 doc + one-line pointer in the original design doc) (done 2026-07-08, commit 7819a19)
- [x] hardening addendum plan (Tasks 27+) + surgical changeset to the committed plan — ONE atomic commit (done 2026-07-08, commit 35af0b3)
- [x] planning close-out: this checklist reconciled + JOURNAL line (done 2026-07-08, this commit)

Execution (merged order — committed-plan tasks interleaved with hardening addendum tasks;
"H" numbers live in docs/plans/2026-07-07-consensus-hardening.md):
- [ ] committed Tasks 1–17 with amended contract shapes (Phase-2 executor, in flight)
- [ ] H28 canonical-plane comparison → H29 occurrence+multiplicity ≥3-of-4 → H30/H30b family-aware pol2 + modal publication
- [ ] committed Task 18 (amended: strict schema, band letters, few-shot, key_fields)
- [ ] H31–H35b quality routing · premium trigger disjunction · escalation params · row-count gate · template fingerprint guard
- [ ] committed Tasks 19–23 (amended)
- [ ] committed Task 24 cutover (composite `…+prompt@p2+pol2+q1`, E1 v3→v4 once) + Task 25 (amended) + Task 26
- [ ] H36–H37 batch parity (POST-cutover: reuses Task 24's Silver mapping + validated() gate; precondition of the first real batch run)
- [ ] H38–H43 audit weights · labels migration 0012 · drift sentinel · shadow harness · refill arm + live-smoke re-point · error-boundary corpus
- [ ] H44 cross-lab transport (config DISABLED) · H45 bake-off (numeric gates) · H46 activation (BLOCKED: gates + HARD CAP + conditional vendor ToS; E1 v4→v5) · H47 SAF write-backs + close-out

### HALT (Phase 3; automation-policy halt-files-a-goal)

- **HARD CAP values** — carried from Phase 2 (see §HALT above): blocks H41b shadow-harness
  spend, H45 bake-off spend, H46 activation. Mechanism fail-closed (`require_budget()`).
- **Non-Google vendor ToS** (conditional) — if the H45 bake-off winner is not the Vertex
  path, H46 is blocked until a founder legal-lane goal (to be filed at the H45→H46
  boundary) closes. Never file it preemptively; never invent HARD CAP values.
