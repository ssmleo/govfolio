# Govfolio.io — Consensus Extraction Design, Amendment 1 (goal 021 Phase 3)

- **Date:** 2026-07-07 (authored 2026-07-08)
- **Status:** Approved (founder, 2026-07-07; approval by issuance of goal 021 Phase 3,
  recorded in `agents/goals/021-llm-extraction.md` § Phase 3). Findings originate from two
  adversarially-verified reviews of the committed Phase-2 plan (30-agent
  correctness-per-dollar pass + 12-agent cross-lab model research pass; load-bearing claims
  primary-source-verified 2026-07-07).
- **Scope:** Amends `docs/plans/2026-07-07-consensus-extraction-design.md` (which stays
  authoritative for everything not amended here). Comparator policy pol1 → pol2
  (family-aware voting, ≥3-of-4 escalation acceptance, two-plane comparison, modal
  publication), strict tool schema (prompt p1 → p2), pixel/quality-triggered premium
  scrutiny, recall + template guards, audit/label/drift instrumentation, shadow eval
  harness, and a bake-off-gated cross-lab third vote.
- **Relation:** Original design body is NOT rewritten (amendment discipline); this document
  carries every change. Implementation: surgical edits to
  `docs/plans/2026-07-07-consensus-extraction.md` + addendum plan
  `docs/plans/2026-07-07-consensus-hardening.md` (Tasks 27+).

---

## 1. Amendments A1–A17 (approved findings, operationalized)

### A1 Occurrence-aware dispute identity (goal Tier-0 #1 + #14 merged)

`RowVerdict::Disputed` carries the aligned `RowKey` (incl. occurrence) and per-sample
UNdeduped candidates; premium rows get occurrence indexes via the same counter `align()`
uses; escalation acceptance is a strict majority ≥3-of-4 readers (samples + premium) with
TRUE vote multiplicity. Record the verified defect: the committed `field_resolution`
counts deduped values, so a 2v1 split counts as 1v1 and a premium siding with the minority
publishes the MINORITY value at 0.75 — stronger than the goal's "plurality would be a
no-op" wording.

### A2 Document-order emission contract (Tier-0 #2)

~40-token prompt addition: rows must be emitted strictly in printed top-to-bottom order,
continuing across pages, each printed row exactly once. Occurrence-index pairing and
ordinal derivation are load-bearing but only implicitly assumed today; making the contract
explicit closes cross-ordered-duplicate chimera publication and silent-dedup recall loss
at near-zero cost.

### A3 Row-count completeness gate (Tier-0 #3)

A projection-profile row estimate runs per page (CPU-only, ~$0); a mismatch against the
consensus row count in either direction opens a doc-level `row_count_mismatch` review_task
and caps every published row on that document to 0.79 — the system's only recall gate,
since three passes correlated on the same faint row otherwise publish "complete" at 0.90.

### A4 Template fingerprint guard (Tier-0 #4)

Revision classification runs before ANY coordinate-based pixel check: an unrecognized
template makes pixel checks emit no votes and no caps, and logs `template_unrecognized`
in stats instead. All ROI geometry today is calibrated from a single 2026 fixture; blind
application to 2012+ backfill forms would either flood review with false 0.79 caps or
silently void the checks. Per-revision coordinate sets are us_house regime knowledge
(AUTHORITY.md quirks), written in the task that measures them.

### A5 Few-shot worked example (goal #5)

The committed 9115811 expected transcription becomes a ≥1.1k-token few-shot example in the
cache-eligible static prefix, framed explicitly as "worked example of FORMAT, not this
document"; a fixture-literal leak check guards against verbatim copying, and 9115811 is
excluded from future calibration sampling (sha-keyed). Record the known consequence: the
single live smoke test targets 9115811, so once it appears in the prompt its value
assertions degrade to mechanics-only checks (schema validity, `stop_reason`); hard value
assertions move to a refill-template artifact once that arm exists (H42). Zero new live
tests are added to cover the gap in the interim.

### A6 Field-wise modal publication, Agreed branch only (goal #6)

On the Agreed branch, publish the per-field modal value across the 3 candidates in the
canonical plane rather than always taking `candidates[0]`; the published value is always
one of the models' own verbatim strings; a 3-way tie takes the first sample and records
the tie in stats. The escalation-resolved (Disputed) branch is explicitly OUT of this
change — Disputed carries per-sample undeduped candidates and reworking that vote
multiplicity is not in scope here.

### A7 Escalation-pass hardening (goal #7)

The premium pass never receives `thinking` config (Sonnet 5 omission is adaptive);
escalation `max_tokens` is sized so thinking never starves the tool call, with
`stop_reason == tool_use` asserted in the live smoke; `output_config.effort` is exposed as
an `extractor.toml` knob. This is a $0 change — the design's cost table already prices
adaptive thinking.

### A8 Confidence-lane-aware stratified audit (goal #8)

Sampling weights are keyed on `(extraction path, confidence)`; the LLM-0.90 lane — the only
lane no human reviews today, and the one the uniform sampler starves to ~10 rows/month — is
heavily oversampled. The draw stays deterministic-seeded and idempotent, with a nonzero
deterministic-lane floor preserved; the precision report applies inclusion-probability
weights so the SLO number isn't biased by the new weighting. The two ship together, not
staged.

### A9 Error labels + drift sentinel (goal #9)

Every review `Verdict::Edit` gets a mechanical field-diff (the 0.75/0.79 lanes are 100%
reviewed — free ground truth). An expand-only migration adds `discrepancy_fields jsonb` and
a closed `error_class` vocabulary to `sample_audit`, keyed by the `extracted_by` composite
tag. A weekly report-only worker bin compares agreement/escalation/hold/schema-invalid
rates against a trailing baseline (thresholds in `extractor.toml`; a breach opens a
review_task), and per-field premium-vs-majority agreement is logged from every escalation
as a free ongoing cross-model probe. Labels only — auto-demotion from lanes stays explicitly
out of scope.

### A10 Shadow consensus eval harness (goal #10)

Electronic PTRs (deterministic Silver is ground truth) get rasterized and run through the
full consensus path via Batch under an isolated tag `shadow@1` that never writes Gold,
producing per-field confusion matrices bucketed by consensus outcome — measuring
P(wrong | 3/3 agree), the design's admitted blind spot, before the 50k backfill. The run is
HARD-CAP gated. The refill arm (programmatically filling the blank paper template with
known values, producing checkbox ground truth at scale) rides the same harness. This
harness doubles as the §D bake-off rig — it is built once and reused.

### A11 Strict closed-vocabulary tool schema (goal #11)

Free-String DTO fields become schemars enums under a `strict: true` tool definition emitted
by a NEW consensus-only request builder (the frozen v1 seam is untouched). Band is emitted
as `band_column: enum A..J` plus `over_1m_spouse_dc: bool` — column K structurally cannot
contaminate the band; Rust maps letter → band string via the existing `tables::BANDS` index
order. The date `pattern` regex lives ONLY in local re-validation, since API-side strict
schemas strip `pattern`. A new `LlmConsensusRow` DTO lives in `us_house::consensus`; the v1
`LlmTransactionRow` / `tool_spec()` stay frozen. The model still never emits money — now not
even band strings, just letters, which also decorrelates shared transcription errors and
makes the pixel check index-to-index exact.

### A12 Alignment key drops voted fields (goal #12)

`key_fields` narrows to `[asset text, transaction_date]` only; the occurrence index handles
duplicates. With band/type in the key, the designed premium tiebreak is unreachable for
band disputes (key-splits mask them — 2v2 ties publish at 0.75) and every 1/3 band misread
spawns a phantom hold whose `Verdict::Edit` resolution inserts a duplicate Gold row.
Dropping them restores reachability of the premium tiebreak and kills the phantom holds.

### A13 Two-plane comparison (goal #13)

A canonical plane (NFKC/casefold/whitespace/dash normalization for asset text; date parsed
to ISO) is used for keys, agreement, and premium matching ONLY; the published value is
always the modal VERBATIM string among canonical-equals, with a deterministic tie-break —
invariant 2 holds by construction. Both planes are logged. Field lanes are bound by name
convention (`*_date_raw` ⇒ date lane; asset text ⇒ text lane) rather than a new
`ConsensusSpec` field, avoiding churn to its existing test literals. An unparseable date
canonicalizes to a sentinel that compares UNEQUAL — it routes to the dispute lane, never a
doc-level error.

### A14 Vote-margin-stratified escalation acceptance (goal #14)

Publish at 0.75 only when the winning value has ≥3 of the 4 readers (samples + premium); a
1/1/1 scatter with premium siding with one of them is 2-of-4 and HOLDs rather than
publishing. This is merged into A1's mechanism above; it is listed separately here only for
finding-number traceability back to goal §3.

### A15 Pixel-ambiguity-triggered premium on unanimous rows (goal #15)

A gray zone around `INK_THRESHOLD`, or two adjacent boxes both above the noise floor, fires
the single shared premium pass even on an unambiguous 3/3 agreement; premium concurrence
leaves 0.90 standing, premium dissent caps to 0.79 (the value is never rewritten — pixel
signal selects scrutiny, never truth). This amends design D8's trigger; the corresponding
policy row is in §2 below.

### A16 Quality-routed vote sets (goal #16)

Preprocessing by-products already computed for other purposes — residual skew, Otsu
between-class variance, noise count — feed a new `QualityMetrics` struct; thresholds live
in `extractor.toml` and are versioned into the composite tag as `q1`. Flagged documents run
3 samples plus the premium pass up front (reusing the same single premium slot, never a
second one), and 0.90 on a flagged document additionally requires premium concordance.
Thresholds are calibrated on committed PNGs, since pdfium output is not byte-stable across
builds.

### A17 Error-boundary fixture corpus + real regression gate (goal #17)

Every audit-confirmed published error becomes a committed fixture. The gate for
model/prompt/N bumps is a key-gated LIVE eval worker bin run over that corpus — a bin, never
a CI test, preserving the one-`#[ignore]`-live-test budget (conformance's primed-cache gate
is green by construction and cannot catch model regressions). Policy-only bumps instead
re-score stored `extraction_sample` payloads offline, with defined pass semantics and a
flake policy.

## 2. Confidence policy pol2 (supersedes §4's pol1 mapping; closed set unchanged)

The closed confidence SET {0.90, 0.75, 0.79} is unchanged — pol2 changes when each value
is reachable, not the values themselves (goal §5: this goal adds none to the set).

| Situation (row-level, critical fields; canonical compare, verbatim publish) | Outcome |
|---|---|
| 3/3 samples agree all critical fields, no flags | 0.90 publish |
| 3/3 agree, pixel-ambiguity / quality-flagged / high-impact floor → premium fires | premium concurs → 0.90 stands; dissents → 0.79 cap (value never rewritten; review lane) |
| Any critical-field disagreement (incl. 2×Haiku vs cross-lab — family-aware: never a 2/3 publish) → the ONE premium pass | winning value ≥3-of-4 readers, true multiplicity → 0.75; else HOLD |
| Premium novel value / tie stands | HOLD |
| Cross-lab vendor outage after bounded retries | doc errs → freeze + review_task; kill-switch `[cross_lab] enabled=false` = 3×Haiku under a DIFFERENT composite (mechanically honest) |

The one-premium-call invariant: exactly ONE `Option<SamplePass>` premium slot and ONE
transport send site; `premium_needed = quality_flagged || pixel_ambiguous || any_disputed
|| high_impact_floor` is computed once; an up-front prefetch fills the SAME slot the
dispute branch later reads; on the batch path, poll reuses a persisted escalation pass
before ever refiring. Note the §6.3 high-impact floor is thereby made real (the committed
plan computed it and discarded it) — consistent with the Phase-2 controller resolution
recorded in `.superpowers/sdd/progress.md`. Family-aware semantics are CODED in pol2 and
degenerate to same-family rules while the vote set is 3×Haiku (property-tested), so enabling
the cross-lab vote changes only the composite's models component.

Labeled assumption (non-blocking, ambiguity protocol): `vote_header` stays majority-of-3 —
header fields are protected downstream by roster resolution plus the stratified audit, so
they don't need the same family-aware treatment as row-level critical fields.

## 3. Versioning: pol2 / p2 / q1 in one cutover

A single consolidated bump — `versions.policy` pol1→pol2, `versions.prompt` p1→p2, NEW
`versions.quality` q1 — folds into the committed plan's Task-24 cutover; the composite
becomes `claude-haiku-4-5-20251001x3@t0.7+claude-sonnet-5+prompt@p2+pol2+q1`. Zero cache
rows, Gold rows, or lock pins exist under any consensus tag today, so E1 supersession stays
v3→v4 exactly once. The later cross-lab ACTIVATION is the second supersession (v4→v5,
hardening Task H46) — following the same mechanical trail (supersedes-sha + reason + date).

## 4. Cross-lab third vote (AD; bake-off-gated)

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

Adjudicated specifics:

- Candidates in the bake-off: Gemini 3.1 Flash-Lite (Vertex; ops-preferred — zero new
  vendor, HARD CAP stays single-cloud), gpt-5.4-mini (strongest decode-time schema
  guarantee), Qwen3-VL-235B (max lineage distance, open weights). Mistral Small 4 dropped:
  vision+schema pairing undocumented — weakest evidence.
- Numeric promotion gates (ALL must pass; corpus = shadow harness, ≥400 electronic-PTR
  docs / ≥1,500 rows, degraded stratum ≥150 refill-arm docs):
  1. per-critical-field accuracy ≥ Haiku-baseline −2pp overall AND ≥ −4pp on the degraded
     stratum;
  2. schema-invalid pass rate ≤ 1.0% after one retry;
  3. decorrelation (primary): P(candidate errs with the same wrong value | both Haiku
     samples err with that value) ≤ 0.40 per critical field, AND candidate–Haiku
     wrong-value overlap strictly below the Haiku–Haiku overlap on the same corpus;
  4. ops: batch images+schema proven; price ≤ 2× a Haiku sample pass; no announced
     shutdown inside the backfill horizon (re-verify the provider deprecations page at
     execution); ToS confirms no training on API inputs; per-vendor temperature convention
     verified (Gemini 3.x: 1.0 only, loader-enforced).
- Vendor client shape: a TRANSLATION-LAYER implementation of the existing `Transport`
  trait (Anthropic-shaped request in, vendor API out, Anthropic-shaped response back) in
  `crates/pipeline/src/extraction/vendors/`; `[cross_lab]` config carries vendor, model,
  per-vendor sampling and media constraints (e.g. gpt-5.4-mini pre-resize ≤1408px); config
  loader rejects Gemini temperature ≠ absent/1.0 and any router URL in production config.
- Cross-lab × batch topology (decided): 2×Haiku via Anthropic Message Batches + ONE direct
  cross-lab call per document at poll time (bounded concurrency, invariant 10) — one
  composite per document, uniform sync/batch semantics. Recorded trigger: move the
  cross-lab vote to the vendor's own batch API (Gemini batch −50% exists) if poll-time
  cost/scale warrants.
- Router (OpenRouter, `require_parameters:true`, ZDR) bake-off ONLY; production always
  direct APIs; never free tiers.

Family-aware voting is load-bearing: decorrelation literature (arXiv 2506.07962,
2605.29800, 2603.17111) shows cross-lab panels collapse to ~2 effective independent votes
and same-family models agree ~60% when both err. Therefore 2×Haiku agreement plus cross-lab
dissent on a critical field is a DISPUTE (escalate), never a 2/3 publish; cross-lab
concurrence keeps 0.90 (the ceiling is unchanged — the gain is fewer wrong 0.90s, not higher
confidence).

## 5. Disposition of the Phase-2 amendment candidates

From the goal file's "Phase 2 amendment candidates" section: lever 2 (vote decorrelation)
is SUPERSEDED by AD (the cross-lab third vote is the adjudicated form of it); levers 1
(adaptive N) and 3 (per-pass crop) remain UNADOPTED candidates — not approved by Phase 3,
still awaiting founder adjudication if ever raised again.

## 6. HALT items (automation-policy: halt files a goal)

HARD CAP values (carried from Phase 2; blocks shadow-harness spend H41b, bake-off H45,
activation H46; `require_budget()` fail-closed) · non-Google vendor ToS (conditional;
founder legal lane; filed at the H45→H46 boundary if the winner is not the Vertex path).
