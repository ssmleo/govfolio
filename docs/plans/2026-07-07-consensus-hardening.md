# Consensus Hardening Addendum Plan (goal 021 Phase 3)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
**Required skills:** skill:subagent-driven-development
> Work on the existing branch (`goal/021-consensus`). Read `/CLAUDE.md` and `agents/goals/021-llm-extraction.md` (§ Phase 3) before each task. Repo is memory: update the goal's Phase-3 checklist and commit every task.
> This is an ADDENDUM to `docs/plans/2026-07-07-consensus-extraction.md` (26 tasks, amended in place — its Tasks 3/9/10/13/17/18/19/20/22/23/24/25 carry **AMENDED** banners and are executed AS EDITED). Task numbering continues that plan's space as H27–H47. Interleaving is normative: follow the **Merged Execution Order** below, never plain numeric order.

**Goal:** Harden the committed Phase-2 consensus extraction with the founder-approved goal-021-Phase-3 findings (design amendment `docs/plans/2026-07-07-consensus-extraction-amendment-1.md`, A1–A17 + AD): occurrence-aware ≥3-of-4 escalation on true vote multiplicity, two-plane comparison, family-aware pol2, modal publication, quality/pixel/high-impact-triggered premium scrutiny under a single-call guard, recall (row-count) + template-revision guards, batch parity, lane-aware audit + error labels + drift sentinel, shadow eval harness + refill arm + error-boundary corpus, and a bake-off-gated cross-lab third vote.

**Architecture:** Everything rides the committed plan's seams: comparator evolutions in `crates/pipeline/src/extraction/consensus.rs` (the route-3→4→17 evolution idiom, continued), preprocess by-products in `preprocess.rs`, config in `config/extractor.toml` (+`config.rs`, all new tables `#[serde(default)]`), adapter geometry in `crates/adapters/us_house/src/consensus.rs`, persistence via migration 0011/0012, worker bins beside the existing report-only bins, and a translation-layer `Transport` impl under `crates/pipeline/src/extraction/vendors/`. One consolidated version cutover (prompt p2 + policy pol2 + quality q1) inside the amended Task 24; cross-lab activation is a second, gated cutover (H46, E1 v4→v5).

**Tech Stack:** unchanged from the committed plan (Rust stable; tokio, reqwest+rustls, sqlx, serde_json `float_roundtrip`, schemars, jsonschema, rust_decimal, image/imageproc, pdfium-render, toml). NEW at H46 only: Vertex AI `generateContent` via the existing reqwest stack under ADC — no new SDK, no router.

## Global Constraints

Everything in the committed plan's Global Constraints (as amended 2026-07-08) binds every H-task, plus:

- **pol2 semantics** (amendment-1 §2) — the closed confidence SET {0.90 exact `0.9f32`, 0.75, 0.79} is UNCHANGED; pol2 changes reachability only. Set-membership asserts, never thresholds. LLM rows never auto-verify.
- **One premium call per document, ever:** a single `Option<SamplePass>` slot, a single `transport.send` site; `premium_needed = doc_quality_flagged || pixel_ambiguous || any_disputed || high_impact_floor` computed once (H32's 2⁴ property test is the invariant's gate). Batch reuses a persisted escalation pass before ever refiring (H37).
- **Deterministic checks cap or route; they never rewrite a field** (invariant 2; pixel/template/row-count guards inherit the ROI doctrine).
- **CI offline + deterministic; exactly ONE key-gated `#[ignore]` live test repo-wide** (the amended Task 25 smoke, re-pointed at a refill artifact in H42). The eval surfaces here (H41b shadow, H43b live eval, H45 bake-off) are worker BINS — manual, key-gated, never CI, never wrapped in `#[ignore]` tests.
- **HARD CAP fail-closed:** `require_budget()` gates every spend surface (H41b, H45, H46); values are the founder's — NEVER invented. HALT stands.
- **Family-aware anti-patterns (auditor rejects on sight):** flat vote counting across families · publishing 2-same-family votes over a cross-lab dissent · router in production · free-tier keys · a model with a published shutdown inside the backfill horizon · temperature <1.0 to Gemini 3.x · `pattern`/unsupported keywords in API-side strict schemas · skipping the bake-off gate · a second premium call · retro-editing landed code without a policy_version bump.
- Every task independently green: `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test --workspace` · `cargo run -p pipeline --bin conformance -- us_house` at each commit. db-gated lanes: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio` + `#[ignore = "needs postgres"]`.
- E1-pinned artifacts change ONLY inside the amended Task 24's atomic v4 supersession or H46's atomic v5 supersession (same mechanical trail: supersedes-sha + reason + date).
- Docs discipline: AUTHORITY.md touches are append-only quirks + `cargo run -p pipeline --bin validate-survey -- us_house` in the same task (SAF same-PR).
- Pre-cutover comparator changes (H28–H30b) are inert in production: the adapter's tier-3 live
  path routes through `ConsensusExtractor` only from Task 24's cutover (the committed plan wires
  it there) — until then the consensus code has no production caller.
- Where a task Consumes code landed by an earlier task, the signatures quoted here are the
  contract; the landed file is ground truth — verify before wiring (two controllers share this
  branch).

## Merged Execution Order (normative)

| Phase | Execute | Notes |
|---|---|---|
| M-A | Committed 1–17 (Tasks 3/9/10/13/17 AS AMENDED) | Phase-2 executor lane; H27 runs FIRST if any doubt about what landed when |
| M-B | H27 → H28 → H29 → H30 → H30b | comparator hardening; H29 evolves Task 17, so M-B starts only after committed 17 |
| M-C | Committed 18 (AS AMENDED) | strict schema (A11) / few-shot (A5) / key_fields (A12) / document-order prompt contract (A2) |
| M-D | H31 → H32 → H33 → H34 → H35a → H35b | quality/pixel/recall/template; M-D lands code + offline coverage — the live adapter path exercises it only from Task 24's cutover (Task 24 wires tier-3 through ConsensusExtractor) |
| M-E | Committed 19–23 (AS AMENDED) | persistence + batch machinery |
| M-F | Committed 24 (AS AMENDED, after H32) → 25 (AS AMENDED) → 26 | the ONE pol2/p2/q1 cutover, E1 v3→v4 |
| M-G | H36 → H37 | batch parity — deliberately POST-cutover: H36 reuses Task 24's Silver mapping (`to_staging_rows` widened to a shared `silver_rows`) and asserts against Task 24's `validated()` gate + `us_house_ptr/consensus@1` tag. Safe because batch is inert until HARD CAP values exist; H36+H37 are PRECONDITIONS of the first real batch run |
| M-H | H38 → H39 → H40 → H41a → H41b → H42 → H43a → H43b | instrumentation + eval (post-cutover) |
| M-I | H44 → H45 | cross-lab prep (disabled) + bake-off |
| M-J | H46 (GATED) → H47 | activation (E1 v4→v5) + SAF close-out |

Nothing may leave the conformance gate red between commits, in any lane.

### Task H27: Frontier re-check + amendment reclassification

Docs-procedure task — no code. Purpose: before H28+ build on "post-changeset" shapes, mechanically
verify which committed-plan tasks the surgical changeset touched are still un-executed, and file the
reclassification for any that raced ahead. Rationale: goal `021-llm-extraction.md` §"Phase 3 goal
text (verbatim)" §1 "Execution-frontier constraint" — "Amendments to ALREADY-LANDED code are
code-amendment tasks... Never retro-edit a landed task's plan section — mark it superseded and point
to the amendment task."

**Files:**
- Test/Modify: none (repo-inspection only)
- Create: none

**Interfaces:**
- Consumes: `git log`, `git diff --stat`, `git status --short` against `crates/pipeline/src/extraction/`
  and `docs/plans/2026-07-07-consensus-extraction.md`; `agents/goals/021-llm-extraction.md`'s
  "## Checklist (Phase 2)" and "### Checklist (Phase 3)" sections.
- Produces: a pass/fail frontier report (this task's own commit-message body, since there is no code
  diff) plus, if THE RULE (Step 3) fires for any target, a filed follow-up note in the goal's Phase 3
  checklist for the orchestrator to pick up (not authored here — filing is out of scope for this
  docs-only task; H27 only detects and records, per the goal's halt-files-a-goal convention).

- [ ] **Step 1: Read current state**

  Run, in order:
  ```bash
  git log --oneline -30
  git diff --stat HEAD -- crates/pipeline/src/extraction
  git status --short -- docs/plans/2026-07-07-consensus-extraction.md crates/pipeline/src/extraction agents/goals/021-llm-extraction.md
  ```
  Then read `agents/goals/021-llm-extraction.md`'s `## Checklist (Phase 2)` and `### Checklist
  (Phase 3)` sections directly (no command — open the file).

  Expected (observed at this task's authoring time, 2026-07-08 — RE-RUN at execution time; this is a
  worked example of the read, not a frozen assertion):
  - `git log --oneline -30`'s newest entries include the design-amendment-1 commit
    (`docs(plans): consensus extraction design amendment 1 — pol2, strict schema, cross-lab third
    vote (goal 021 Phase 3)`) and the Phase-3 registration commit
    (`chore(agents): register goal 021 Phase 3 — consensus hardening + cross-lab third vote`); no
    commit yet for any hardening-plan `H`-numbered task or for committed-plan Tasks 2–25.
  - `git diff --stat HEAD -- crates/pipeline/src/extraction` shows only `mod.rs` (the
    `pub mod consensus;` registration line, 1 insertion) as an uncommitted change.
    `git status --short` additionally shows `crates/pipeline/src/extraction/consensus.rs` as `??`
    (untracked) and `docs/plans/2026-07-07-consensus-extraction.md` as `M` (the surgical changeset,
    uncommitted) — i.e. committed-plan Task 1 exists only as UNCOMMITTED working-tree content; no
    Task-1 commit has landed either.
  - `## Checklist (Phase 2)`: "Phase A–C" / "Phase D–E" / "Phase F–G" / "Phase H" boxes all
    unchecked (Tasks 1–26 in flight). `### Checklist (Phase 3)`: the design-amendment box is
    checked; the "hardening addendum plan... — ONE atomic commit" box is UNCHECKED (this cluster's
    own deliverable, not yet landed).

- [ ] **Step 2: Verify each surgical edit target is still un-executed**

  The surgical changeset (`docs/plans/2026-07-07-consensus-extraction.md`'s working-tree diff)
  touches committed-plan Tasks 3, 9, 10, 13, 17, 18, 19, 20, 22, 23, 24, 25 (per amendment-1.md
  §"Relation": "surgical edits to `docs/plans/2026-07-07-consensus-extraction.md`"). For each,
  verify no commit has landed the corresponding CODE (a plan-doc edit landing is expected and fine;
  a commit IMPLEMENTING that task's code before the edit is what voids it):
  ```bash
  git log --oneline --all -i -E --grep='goal 021.*task (3|9|10|13|17|18|19|20|22|23|24|25)\)'
  ```
  Expected: no output (zero matches) — none of the twelve targets has a landed implementation commit.
  If this ever returns a match for task N, THE RULE (Step 3) fires for that N; note which commit SHA
  landed the code and whether it predates or postdates the corresponding plan-doc edit (`git log -1
  --format=%cI -- docs/plans/2026-07-07-consensus-extraction.md` for the doc edit's own commit date,
  once it is committed, compared against the code commit's date).

- [ ] **Step 3: Apply THE RULE**

  THE RULE: any surgical edit whose target task ALREADY EXECUTED (landed a code commit) BEFORE the
  edit itself landed is VOID for that task — its content reclassifies to the corresponding H-task as
  a code-amendment, never a retro-edit of the landed task's own plan section (goal §1
  "Execution-frontier constraint"). The mapping for this goal's twelve targets:
  - **Task 3 edit → H29 absorbs** the `RowVerdict::Disputed` shape change (`key: RowKey` +
    undeduped candidates) as a code evolution of Task 3's landed enum, not a plan retro-edit.
  - **Task 9/10 edits → H33/H34-adjacent** config evolution (escalation params / row-count gate
    config additions land as evolutions of Task 9's `ExtractorConfig` and Task 10's
    `build_image_request`/`SamplingParams`, in whichever of H33/H34 owns the touched surface).
  - **Task 13 edit → H32 absorbs** the shared MockTransport switch, `test_cfg` 12-field shape, and
    Disputed interface quote change as a code amendment via H32's ConsensusSpec/test-helper sweep
    (H32 already grep-and-patches every landed ConsensusSpec/test fixture), plus a one-line
    mock-switch follow-up recorded in the goal checklist — if Task 13 already executed before this
    edit landed.
  - **Task 17 edit → H29 absorbs** the key-threaded `resolve_disputed` arity change and the
    occurrence-aware `premium_row_at` helper as a code evolution — H29 already evolves exactly
    these helpers and independently re-verifies Task 17's landed state at its own start — if Task
    17 already executed before this edit landed.
  - **Task 18 edit → its own code-amendment task to be filed.** No H-task in this cluster (H27–H30b)
    or its known neighbors currently owns a Task-18-specific catch-up; if Task 18 (ROI checkbox
    cross-check, us_house) has already executed when this rule fires, file a new goal-021-Phase-3
    checklist line for a dedicated amendment task before continuing (automation-policy
    halt-files-a-goal — this is a HALT, not a guess at scope).
  - **Task 24 edit → pre-cutover verification.** Task 24 is the atomic cutover (composite tag bump,
    E1 lock supersession) — if it has already executed, the surgical edit's content becomes a
    verification checklist item run immediately before any FURTHER supersession, never a rewrite of
    the already-superseded lock.
  - **A2 (document-order prompt contract) → committed Task 18 AS AMENDED** (the prompt's
    document-order paragraph) — same Task-18 target as A5/A11/A12 above, listed separately here
    so the trace is greppable by amendment letter, not just by task number.
  - **Task 19 edit → H37 absorbs** the `extraction_doc_signal` signal-table addition as a
    follow-on migration if migration 0011 already ran unedited (a new migration file gains the
    table rather than 0011 itself, since 0011 is already applied).
  - **Task 20 edit → H36 absorbs** the Silver-mapping persistence fix (`to_staging_rows` →
    `silver_rows`, `persist_consensus_run` wiring) as a code amendment to the landed
    `persist_published`/`persist_consensus_run`, never a retro-edit of Task 20's own plan section.
  - **Task 22/23 edits → H36/H37 absorb** their respective touched surfaces as code amendments:
    22's `consensus-batch-submit` preprocessing/signal-persist changes → H37; 23's
    `resolve_document`/`ingest_batch_results` Silver-shape changes → H36.
  - **Task 25 edit → H42 absorbs** the live-smoke re-shape (re-pointing the sole
    `#[ignore = "needs ANTHROPIC_API_KEY"]` test from the 9115811 bronze fixture to the refill
    artifact) as a code amendment to Task 25's landed test body.

  As of Step 2's result at authoring time (all twelve un-executed), THE RULE does not fire for any
  target — every surgical edit stands as an ordinary plan amendment to a not-yet-executed task. This
  step's job is to re-run the check at whatever time H27 actually executes and apply the mapping
  above to any target the check flags.

- [ ] **Step 4: Verify `consensus.rs` still matches the committed Task-1 API**

  ```bash
  grep -n 'pub mod policy\|POLICY_VERSION\|pub struct RowKey\|pub fn row_key\|fn key_fields_content' crates/pipeline/src/extraction/consensus.rs
  ```
  Expected (five matches, Task-1 shape, `pol1` not yet bumped — H30 owns the `pol2` bump):
  ```
  18:pub mod policy {
  31:    pub const POLICY_VERSION: &str = "pol1";
  72:pub struct RowKey(String, usize);
  80:pub fn row_key(row: &Value, key_fields: &[String], occurrence: usize) -> RowKey {
  88:fn key_fields_content(row: &Value, key_fields: &[String]) -> String {
  ```
  If `align`/`score`/`route`/`ConsensusExtractor`/etc. also appear in this file already, the frontier
  has moved past Task 1 — re-run Step 2's per-task grep (extended to Tasks 2, 4, 5–17 as needed) to
  find exactly how far, and confirm H28's own prerequisite (committed Task 13's `summarize_agreement`
  must exist — see H28's intro note on the corrected execution slot) before relying on it.

- [ ] **Step 5: Record the result — no commit**

  This is a checklist-only task: its "commit" step is ticking this task's line in
  `agents/goals/021-llm-extraction.md`'s `### Checklist (Phase 3)` → Execution block from `[ ]` to
  `[x]` once Steps 1–4 have been run and (if THE RULE fired) the follow-up filed per Step 3. Do
  **not** run `git commit` for this task in isolation — the checklist tick rides in the same commit
  as whichever code task executes next in the merged order (or the addendum's own landing commit),
  consistent with this task carrying no code diff of its own.

  Per-task green gate (still required — confirms the repo was left exactly as found):
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace &&
  cargo run -p pipeline --bin conformance -- us_house`

---

### Task H28: Canonical-plane comparison (finding 13 / amendment-1 §A13)

Amendment-1 §A13: "A canonical plane (NFKC/casefold/whitespace/dash normalization for asset text;
date parsed to ISO) is used for keys, agreement, and premium matching ONLY; the published value is
always the modal VERBATIM string among canonical-equals... Field lanes are bound by name convention
(`*_date_raw` ⇒ date lane; asset text ⇒ text lane)... An unparseable date canonicalizes to a
sentinel that compares UNEQUAL — it routes to the dispute lane, never a doc-level error." Implements
`canonical_field`; EVOLVES Task 1's `key_fields_content` to join canonical outputs; EVOLVES Task 3's
`disagreeing_fields` to compare canonically; EVOLVES committed Task 13's `summarize_agreement` to add
two-plane counters. Slot note (corrected from the original cluster brief against the verified
execution-frontier record in `agents/goals/021-llm-extraction.md`'s Phase 3 "Execution (merged
order)" checklist): this task executes AFTER committed Tasks 1–17 land (the merged-order checklist
places the whole "committed Tasks 1–17" block before "H28... → H29 → H30/H30b"), not before Task 13
— `summarize_agreement` (Task 13) must already exist for this task's counters evolution to compile.

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` — add `canonical_field` (new); evolve
  `key_fields_content` (Task 1), `disagreeing_fields` (Task 3), and `summarize_agreement`
  (committed Task 13) bodies; update `summarize_agreement`'s ONE call site inside
  `ConsensusExtractor::extract` (committed Task 13/17)
- Modify: `crates/pipeline/Cargo.toml` — add the `unicode-normalization` dependency
- Test: `crates/pipeline/src/extraction/consensus.rs` (new inline `#[cfg(test)] mod canonical_tests`)

**Interfaces:**
- Consumes: committed Task 1's `fn key_fields_content(row: &Value, key_fields: &[String]) -> String`
  (private); committed Task 3's `fn disagreeing_fields(candidates: &[Value], critical_fields:
  &[String]) -> Vec<String>` (private); committed Task 13's `fn summarize_agreement(verdicts:
  &[RowVerdict]) -> serde_json::Value` (private) and its one call site
  `let agreement = summarize_agreement(&verdicts);` inside `ConsensusExtractor::extract`; committed
  Task 2's `pub struct AlignedRows { pub rows: Vec<AlignedRow> }`, `pub struct AlignedRow { pub
  ordinal0: u32, pub key: RowKey, pub candidates: Vec<Value>, pub presence: PresenceClass }`;
  committed Task 1's `pub struct ConsensusSpec { pub tool: DocumentToolSpec, pub rows_pointer:
  String, pub key_fields: Vec<String>, pub critical_fields: Vec<String> }`.
- Produces (shared contract `hardening-shared-contract.md` §"H28 — canonical plane", exact shape):
  `pub(crate) fn canonical_field(pointer: &str, value: &Value) -> String`; evolved
  `fn key_fields_content(row: &Value, key_fields: &[String]) -> String` (unchanged signature, new
  body); evolved `fn disagreeing_fields(candidates: &[Value], critical_fields: &[String]) ->
  Vec<String>` (unchanged signature, new body) plus a new private helper
  `fn canonical_pointer_value(field: &str, row: &Value) -> String`; evolved `fn summarize_agreement(
  verdicts: &[RowVerdict], aligned: &AlignedRows, spec: &ConsensusSpec) -> serde_json::Value` (TWO
  new parameters — the evolution's cost, same discipline Task 4/17 used for `route`) whose returned
  `Value` gains `"canonical_merges": <u32>` and `"verbatim_variants": <u32>`.

- [ ] **Step 1: Write the failing tests**

  Append to `crates/pipeline/src/extraction/consensus.rs`, in a new module placed anywhere after the
  `disagreeing_fields`/`summarize_agreement` functions exist (order among test modules does not
  matter):
  ```rust
  #[cfg(test)]
  #[allow(clippy::unwrap_used, clippy::float_cmp)]
  mod canonical_tests {
      use serde_json::json;

      use super::*;

      fn dummy_tool() -> DocumentToolSpec {
          DocumentToolSpec {
              tool_name: "record_ptr_transactions".to_owned(),
              tool_description: "test".to_owned(),
              input_schema: json!({}),
              prompt: "test".to_owned(),
          }
      }

      fn spec() -> ConsensusSpec {
          ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
              critical_fields: vec!["/amount_raw".to_owned()],
          }
      }

      fn row(asset: &str, date: &str, amount: &str) -> Value {
          json!({ "asset_raw": asset, "transaction_date_raw": date, "amount_raw": amount })
      }

      fn sample(rows: Vec<Value>) -> Value {
          json!({ "rows": rows })
      }

      #[test]
      fn double_space_and_case_variance_align_and_agree() {
          let s = spec();
          // Before this task: different key_fields_content strings -> two
          // separate row groups -> each Minority presence -> both Disputed.
          let a = row("Apple  Inc Common Stock", "4/17/2026", "$15,001 - $50,000");
          let b = row("apple inc common stock", "4/17/2026", "$15,001 - $50,000");
          let samples = vec![sample(vec![a.clone()]), sample(vec![b.clone()]), sample(vec![a.clone()])];
          let aligned = align(&samples, &s).unwrap();
          assert_eq!(
              aligned.rows.len(),
              1,
              "cosmetic case/whitespace variance must canonicalize to ONE row, not mis-key"
          );
          let verdicts = score(&aligned, &s);
          assert!(
              matches!(verdicts[0], RowVerdict::Agreed { .. }),
              "canonical-plane agreement on amount_raw (the only critical field) must publish Agreed"
          );
      }

      #[test]
      fn zero_padded_and_unpadded_date_variants_align_and_agree() {
          let s = spec();
          let a = row("Apple Inc Common Stock", "4/17/2026", "$15,001 - $50,000");
          let b = row("Apple Inc Common Stock", "04/17/2026", "$15,001 - $50,000");
          let samples = vec![sample(vec![a.clone()]), sample(vec![b.clone()]), sample(vec![a.clone()])];
          let aligned = align(&samples, &s).unwrap();
          assert_eq!(aligned.rows.len(), 1, "M/D/YYYY and MM/DD/YYYY must canonicalize to the same key");
          let verdicts = score(&aligned, &s);
          assert!(matches!(verdicts[0], RowVerdict::Agreed { .. }));
      }

      #[test]
      fn an_unparseable_date_disputes_against_a_parsed_one_never_a_doc_error() {
          let s = spec();
          let unparsed = row("Apple Inc Common Stock", "APR 17", "$15,001 - $50,000");
          let parsed = row("Apple Inc Common Stock", "4/17/2026", "$15,001 - $50,000");
          let samples =
              vec![sample(vec![unparsed.clone()]), sample(vec![parsed.clone()]), sample(vec![parsed.clone()])];
          // align() must NOT error — the sentinel is a comparison value, never a parse failure.
          let aligned = align(&samples, &s).unwrap();
          // The sentinel-tagged row and the parsed-date row never share a key.
          assert_eq!(aligned.rows.len(), 2);
          let verdicts = score(&aligned, &s);
          assert!(
              verdicts.iter().all(|v| matches!(v, RowVerdict::Disputed { .. })),
              "neither group reaches InAll presence (3/3) — both hold, never a silent doc error"
          );
      }

      #[test]
      fn published_row_carries_a_verbatim_input_never_the_canonical_string() {
          let s = spec();
          // candidates[0] (sample 0, doc order) is deliberately the
          // case/whitespace-noisy variant so this assertion is meaningful.
          let a = row("Apple  Inc Common Stock", "4/17/2026", "$15,001 - $50,000");
          let b = row("apple inc common stock", "04/17/2026", "$15,001 - $50,000");
          let samples = vec![sample(vec![a.clone()]), sample(vec![b.clone()]), sample(vec![a.clone()])];
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          match &verdicts[0] {
              RowVerdict::Agreed { row, .. } => {
                  let asset = row["asset_raw"].as_str().unwrap();
                  assert!(
                      asset == a["asset_raw"].as_str().unwrap() || asset == b["asset_raw"].as_str().unwrap(),
                      "published asset_raw {asset:?} must be one of the model's own verbatim \
                       strings (invariant 2) — never a canonical_field replacement"
                  );
                  assert_ne!(
                      asset,
                      canonical_field("/asset_raw", &json!(asset)),
                      "sanity: this fixture's verbatim strings are not already canonical, so \
                       equality here would mean canonicalization leaked into the published value"
                  );
              }
              other => panic!("expected Agreed, got {other:?}"),
          }
      }

      #[test]
      fn summarize_agreement_counts_canonical_merges_and_verbatim_variants() {
          let s = spec();
          // Row 0: verbatim variance on the critical field that canonicalizes
          // to agreement (double space only) -> counts toward BOTH counters.
          let a0 = row("Apple Inc Common Stock", "4/17/2026", "$15,001  - $50,000");
          let b0 = row("Apple Inc Common Stock", "4/17/2026", "$15,001 - $50,000");
          // Row 1: verbatim-identical everywhere -> neither counter.
          let a1 = row("Tesla Inc Common Stock", "4/18/2026", "$1,001 - $15,000");
          let samples = vec![
              sample(vec![a0.clone(), a1.clone()]),
              sample(vec![b0.clone(), a1.clone()]),
              sample(vec![a0.clone(), a1.clone()]),
          ];
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          let agreement = summarize_agreement(&verdicts, &aligned, &s);
          assert_eq!(agreement["verbatim_variants"], json!(1u32));
          assert_eq!(agreement["canonical_merges"], json!(1u32));
      }
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: FAIL to compile — `error[E0425]: cannot find function 'canonical_field' in this scope`
  (does not exist yet); separately, `summarize_agreement(&verdicts, &aligned, &s)` fails with
  `error[E0061]: this function takes 1 argument but 3 arguments were supplied` against committed
  Task 13's signature.

- [ ] **Step 3: Write minimal implementation**

  Add the `unicode-normalization` dependency (NFKC only — casefold uses `str::to_lowercase`, already
  std, matching the existing convention in `crates/adapters/canada_ciec/src/tables.rs` and
  `crates/adapters/eu_fr_de_annual/src/eu.rs`):
  ```bash
  cargo add unicode-normalization -p pipeline
  ```

  Insert `canonical_field` and its two lane helpers into `crates/pipeline/src/extraction/consensus.rs`
  (anywhere before `key_fields_content`, since it now calls this):
  ```rust
  use unicode_normalization::UnicodeNormalization;

  /// Canonicalizes one field value for the COMPARISON plane only (keys,
  /// agreement, premium matching) — the published value is NEVER this
  /// function's output (invariant 2; amendment-1 §A13). Lane selection by
  /// name convention: a row-relative pointer ending in `_date_raw` uses the
  /// date lane; everything else uses the text lane. Non-string JSON values
  /// fall back to their `Display` form rather than panicking (fail closed —
  /// never a doc-level error).
  #[must_use]
  pub(crate) fn canonical_field(pointer: &str, value: &Value) -> String {
      let verbatim = value.as_str().map_or_else(|| value.to_string(), ToOwned::to_owned);
      if pointer.ends_with("_date_raw") {
          canonical_date(&verbatim)
      } else {
          canonical_text(&verbatim)
      }
  }

  /// Date lane: `M/D/YYYY`, `MM/DD/YYYY`, and `YYYY-MM-DD` all canonicalize
  /// to `yyyy-mm-dd`; anything else canonicalizes to a sentinel that compares
  /// UNEQUAL to every parsed date AND to every other unparseable verbatim
  /// string — it lands in the dispute lane, never a parse `Err`.
  fn canonical_date(verbatim: &str) -> String {
      parse_date_lane(verbatim).unwrap_or_else(|| format!("\u{1e}unparsed\u{1f}{verbatim}"))
  }

  fn parse_date_lane(raw: &str) -> Option<String> {
      for format in ["%m/%d/%Y", "%Y-%m-%d"] {
          if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, format) {
              return Some(date.format("%Y-%m-%d").to_string());
          }
      }
      None
  }

  /// Text lane: Unicode NFKC normalize -> full Unicode case fold (via
  /// `to_lowercase`, the same convention already used elsewhere in this
  /// workspace for comparison-only casefolding — see
  /// `crates/adapters/canada_ciec/src/tables.rs`) -> collapse every
  /// whitespace run to one ASCII space, trim -> map Unicode dash variants
  /// (U+2010..U+2015 hyphen/dash block, U+2212 minus sign) to ASCII `-`.
  fn canonical_text(raw: &str) -> String {
      let nfkc: String = raw.nfkc().collect();
      let folded = nfkc.to_lowercase();
      let mut collapsed = String::with_capacity(folded.len());
      let mut last_was_space = false;
      for ch in folded.chars() {
          let mapped = map_dash(ch);
          if mapped.is_whitespace() {
              if !last_was_space {
                  collapsed.push(' ');
              }
              last_was_space = true;
          } else {
              collapsed.push(mapped);
              last_was_space = false;
          }
      }
      collapsed.trim().to_owned()
  }

  fn map_dash(ch: char) -> char {
      match ch {
          '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
          other => other,
      }
  }
  ```

  Replace `key_fields_content`'s body:
  ```rust
  fn key_fields_content(row: &Value, key_fields: &[String]) -> String {
      key_fields
          .iter()
          .map(|pointer| {
              row.pointer(pointer)
                  .map_or_else(|| "null".to_owned(), ToString::to_string)
          })
          .collect::<Vec<_>>()
          .join("\u{1f}")
  }
  ```
  with:
  ```rust
  fn key_fields_content(row: &Value, key_fields: &[String]) -> String {
      key_fields
          .iter()
          .map(|pointer| {
              row.pointer(pointer)
                  .map_or_else(|| "null".to_owned(), |value| canonical_field(pointer, value))
          })
          .collect::<Vec<_>>()
          .join("\u{1f}")
  }
  ```

  Replace `disagreeing_fields`'s body:
  ```rust
  fn disagreeing_fields(candidates: &[Value], critical_fields: &[String]) -> Vec<String> {
      critical_fields
          .iter()
          .filter(|pointer| {
              let field = pointer.as_str();
              let mut values = candidates.iter().map(|row| row.pointer(field));
              let Some(first) = values.next() else {
                  return false;
              };
              values.any(|value| value != first)
          })
          .cloned()
          .collect()
  }
  ```
  with:
  ```rust
  fn disagreeing_fields(candidates: &[Value], critical_fields: &[String]) -> Vec<String> {
      critical_fields
          .iter()
          .filter(|pointer| {
              let field = pointer.as_str();
              let mut values = candidates.iter().map(|row| canonical_pointer_value(field, row));
              let Some(first) = values.next() else {
                  return false;
              };
              values.any(|value| value != first)
          })
          .cloned()
          .collect()
  }

  /// A row's value at `field` in the comparison plane: `canonical_field`'s
  /// output, or the literal `"null"` when absent — the SAME absent-field
  /// convention `key_fields_content` uses (never panics, never errors).
  fn canonical_pointer_value(field: &str, row: &Value) -> String {
      row.pointer(field).map_or_else(|| "null".to_owned(), |value| canonical_field(field, value))
  }
  ```

  Replace `summarize_agreement`'s body:
  ```rust
  fn summarize_agreement(verdicts: &[RowVerdict]) -> serde_json::Value {
      let agreed = verdicts.iter().filter(|v| matches!(v, RowVerdict::Agreed { .. })).count();
      let disputed = verdicts.len() - agreed;
      serde_json::json!({ "total_rows": verdicts.len(), "agreed": agreed, "disputed": disputed })
  }
  ```
  with:
  ```rust
  fn summarize_agreement(
      verdicts: &[RowVerdict],
      aligned: &AlignedRows,
      spec: &ConsensusSpec,
  ) -> serde_json::Value {
      let agreed = verdicts.iter().filter(|v| matches!(v, RowVerdict::Agreed { .. })).count();
      let disputed = verdicts.len() - agreed;

      // Two-plane audit counters (amendment-1 §A13): how many rows carried
      // verbatim (raw-string) disagreement on a critical field, and how many
      // of those were rescued into agreement purely by canonical comparison.
      let mut canonical_merges = 0u32;
      let mut verbatim_variants = 0u32;
      for row in &aligned.rows {
          let has_verbatim_variance = spec.critical_fields.iter().any(|field| {
              let mut raw = row.candidates.iter().map(|candidate| candidate.pointer(field));
              let Some(first) = raw.next() else {
                  return false;
              };
              raw.any(|value| value != first)
          });
          if !has_verbatim_variance {
              continue;
          }
          verbatim_variants += 1;
          if disagreeing_fields(&row.candidates, &spec.critical_fields).is_empty() {
              canonical_merges += 1;
          }
      }

      serde_json::json!({
          "total_rows": verdicts.len(),
          "agreed": agreed,
          "disputed": disputed,
          "canonical_merges": canonical_merges,
          "verbatim_variants": verbatim_variants,
      })
  }
  ```

  Update `summarize_agreement`'s ONE call site inside `ConsensusExtractor::extract` — replace:
  ```rust
          let agreement = summarize_agreement(&verdicts);
  ```
  with:
  ```rust
          let agreement = summarize_agreement(&verdicts, &aligned, spec);
  ```

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: PASS — all 5 new `canonical_tests` tests green, plus every prior `tests`/`align_tests`/
  `score_tests`/`sanity_tests` test still green (unchanged behavior for verbatim-identical fixtures).
  Then: `cargo test -p pipeline --test consensus_extraction` (Task 13/17's e2e suite recompiles
  against the new `summarize_agreement` call site) PASS — all six tests unchanged.
  Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
  ```bash
  git add crates/pipeline/src/extraction/consensus.rs crates/pipeline/Cargo.toml Cargo.lock
  git commit -m "$(cat <<'EOF'
  feat(pipeline): canonical-plane comparison for consensus keys/agreement (goal 021 Phase 3, Task H28)

  canonical_field (NFKC/casefold/whitespace/dash for text, ISO parse with a
  compares-unequal sentinel for dates, amendment-1 A13) now backs
  key_fields_content and disagreeing_fields — cosmetic transcription noise no
  longer mis-keys or disputes a row. Published values stay verbatim
  (invariant 2); summarize_agreement gains canonical_merges/verbatim_variants
  two-plane audit counters.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
  EOF
  )"
  ```

  Per-task green gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H29: Occurrence-aware, multiplicity-true ≥3-of-4 escalation (findings 1 + 14 / amendment-1 §A1, §A14)

**This task deliberately exceeds the ~1h guideline** — the ≥3-of-4 occurrence-aware rewrite
touches `field_resolution`'s core counting logic AND every one of Task 13's disagreement-matrix
fixtures together; splitting would leave the vote-counting invariant inconsistent (deduped in
some call sites, multiplicity-true in others) between commits. Do not split it.

Amendment-1 §A1 (already partially landed by the surgical changeset — see the note below): "premium
rows get occurrence indexes via the same counter `align()` uses; escalation acceptance is a strict
majority ≥3-of-4 readers (samples + premium) with TRUE vote multiplicity. Record the verified defect:
the committed `field_resolution` counts deduped values, so a 2v1 split counts as 1v1 and a premium
siding with the minority publishes the MINORITY value at 0.75." §A14: "Publish at 0.75 only when the
winning value has ≥3 of the 4 readers... a 1/1/1 scatter with premium siding with one of them is
2-of-4 and HOLDs rather than publishing."

**Verified frontier note (do not skip — this changes this task's actual diff):** the surgical
changeset already landed in `docs/plans/2026-07-07-consensus-extraction.md`'s working tree carries an
"A1 edit" comment on committed Task 17's `field_resolution` and `premium_row_at`:
`premium_row_at` is ALREADY occurrence-aware (it builds the same per-content occurrence counter
`align()` uses, then an exact `RowKey` match) and candidates are ALREADY undeduped (per
`RowVerdict::Disputed`'s changeset-edited shape). What remains — and is this task's entire diff — is
`field_resolution`'s ACCEPTANCE RULE: committed/changeset code still accepts on bare plurality (a
clear top vote-getter with no exact tie for first place, no absolute floor) and counts raw `Value`
equality, not the canonical plane. This task (a) replaces the plurality rule with a strict
`>= 3`-of-4-readers floor, (b) moves vote counting into the canonical plane (H28), and (c) makes the
published value the modal VERBATIM string within the winning canonical group (never a bare copy of
premium's own — possibly cosmetically distinct — string). `resolve_disputed`'s public arity
(`key: &RowKey` added) and `premium_row_at`'s body are UNCHANGED by this task — do not re-edit them.
Slot note: executes after committed Task 17 lands, before committed Tasks 19/23 (goal
`021-llm-extraction.md` "Execution (merged order)" checklist).

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` — replace the `FieldResolution` enum and
  `field_resolution` function bodies; update `resolve_disputed`'s match arms; add `group_vote`/
  `modal_verbatim` helpers (also consumed by H30b's per-field modal publication)
- Test: `crates/pipeline/src/extraction/consensus.rs` (new inline `#[cfg(test)] mod
  escalation_hardening_tests`)

**Interfaces:**
- Consumes: committed Task 17's `pub fn resolve_disputed(ordinal0: u32, key: &RowKey, candidates:
  &[Value], disputed_fields: &[String], spec: &ConsensusSpec, premium: &Value) ->
  Option<PublishedRow>` (arity unchanged by this task — the changeset already added `key`); committed
  Task 17's `fn premium_row_at(premium: &Value, disputed_key: &RowKey, spec: &ConsensusSpec) ->
  Option<Value>` (unchanged, consumed as-is — occurrence-aware already); H28's `pub(crate) fn
  canonical_field(pointer: &str, value: &Value) -> String`; committed Task 1's `pub fn row_key(row:
  &Value, key_fields: &[String], occurrence: usize) -> RowKey` and `pub mod policy { pub const
  CONF_ESCALATED: f32 = 0.75; }`.
- Produces (shared contract §"H29 — escalation acceptance"): a 2-variant `enum FieldResolution {
  Resolved(Value), Held }` (replacing the committed 3-variant `Resolved`/`Novel`/`Tied`); evolved
  `fn field_resolution(field: &str, candidates: &[Value], premium_row: &Value) -> FieldResolution`
  (unchanged signature, new body: canonical-plane grouping, strict `>= 3`-of-4 floor, modal-verbatim
  emission); `fn group_vote(groups: &mut Vec<(String, Vec<Value>)>, field: &str, row: &Value)` and
  `fn modal_verbatim(verbatims: &[Value]) -> Value` (private helpers, first-seen tie-break, reused
  verbatim by H30b).

- [ ] **Step 1: Write the failing tests**

  Append to `crates/pipeline/src/extraction/consensus.rs`:
  ```rust
  #[cfg(test)]
  #[allow(clippy::unwrap_used, clippy::float_cmp)]
  mod escalation_hardening_tests {
      use serde_json::json;

      use super::*;

      fn dummy_tool() -> DocumentToolSpec {
          DocumentToolSpec {
              tool_name: "record_ptr_transactions".to_owned(),
              tool_description: "test".to_owned(),
              input_schema: json!({}),
              prompt: "test".to_owned(),
          }
      }

      #[test]
      fn premium_match_is_occurrence_aware_never_cross_matched_to_occurrence_zero() {
          // Regression lock: premium_row_at's occurrence-awareness landed with
          // the surgical changeset (A1), not this task — this pins that
          // behavior against this task's REWRITTEN field_resolution/
          // resolve_disputed so a future refactor cannot silently regress it.
          let key_fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
          let disputed_fields = vec!["/owner_code_raw".to_owned()];
          let spec = ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: key_fields.clone(),
              critical_fields: vec!["/owner_code_raw".to_owned()],
          };
          let lot = |owner: &str| {
              json!({
                  "asset_raw": "Apple Inc Common Stock",
                  "transaction_date_raw": "4/17/2026",
                  "owner_code_raw": owner,
              })
          };
          // Occurrence-1 disputed row (a second, distinct real transaction at
          // the SAME asset/date — the duplicate-lot pattern): 2 samples say
          // "SP", 1 says "JT".
          let candidates = vec![lot("SP"), lot("JT"), lot("SP")];
          let key = row_key(&candidates[0], &key_fields, 1);

          // Premium's own two rows for this SAME asset/date, in premium's own
          // emission order: occurrence 0 = "JT" (the WRONG pairing a
          // hardcoded-occurrence-0 lookup would grab), occurrence 1 = "SP"
          // (the CORRECT pairing for THIS disputed row).
          let premium = json!({ "rows": [lot("JT"), lot("SP")] });

          let resolved = resolve_disputed(1, &key, &candidates, &disputed_fields, &spec, &premium)
              .expect("occurrence-1 lot resolves: 2 sample \"SP\" + premium's occurrence-1 \"SP\" = 3-of-4");
          assert_eq!(resolved.row["owner_code_raw"], json!("SP"));
          assert_eq!(resolved.confidence, policy::CONF_ESCALATED);
      }

      #[test]
      fn premium_siding_with_the_minority_holds_never_publishes_it() {
          // Regression lock for the historical committed-plan defect (amendment-1
          // §A1: deduped candidates made a 2v1 split count as 1v1, so a
          // minority-siding premium vote won outright). Undeduped counting
          // (already landed) makes this an unbroken 2-2 tie today; this test
          // pins it against THIS task's rewritten threshold logic too.
          let key_fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
          let disputed_fields = vec!["/amount_raw".to_owned()];
          let spec = ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: key_fields.clone(),
              critical_fields: vec!["/amount_raw".to_owned()],
          };
          let row_with = |amount: &str| {
              json!({
                  "asset_raw": "Apple Inc Common Stock",
                  "transaction_date_raw": "4/17/2026",
                  "amount_raw": amount,
              })
          };
          let candidates =
              vec![row_with("$15,001 - $50,000"), row_with("$1,001 - $15,000"), row_with("$15,001 - $50,000")];
          let key = row_key(&candidates[0], &key_fields, 0);
          let premium = json!({ "rows": [row_with("$1,001 - $15,000")] });

          let resolved = resolve_disputed(0, &key, &candidates, &disputed_fields, &spec, &premium);
          assert!(
              resolved.is_none(),
              "a 2-2 vote (majority sample pair vs minority-sample+premium) must HOLD, never publish"
          );
      }

      #[test]
      fn a_one_one_one_scatter_with_premium_matching_one_holds_at_two_of_four() {
          let key_fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
          let disputed_fields = vec!["/amount_raw".to_owned()];
          let spec = ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: key_fields.clone(),
              critical_fields: vec!["/amount_raw".to_owned()],
          };
          let row_with = |amount: &str| {
              json!({
                  "asset_raw": "Apple Inc Common Stock",
                  "transaction_date_raw": "4/17/2026",
                  "amount_raw": amount,
              })
          };
          let candidates = vec![
              row_with("$15,001 - $50,000"),
              row_with("$1,001 - $15,000"),
              row_with("$50,001 - $100,000"),
          ];
          let key = row_key(&candidates[0], &key_fields, 0);
          // Premium matches ONE of the three: 1 sample + 1 premium = 2 votes —
          // a clear PLURALITY winner over the other two (1 vote each), but NOT
          // the required >= 3-of-4 majority.
          let premium = json!({ "rows": [row_with("$15,001 - $50,000")] });

          let resolved = resolve_disputed(0, &key, &candidates, &disputed_fields, &spec, &premium);
          assert!(resolved.is_none(), "2-of-4 (plurality, not majority) must HOLD per finding 14");
      }

      #[test]
      fn a_two_one_split_with_premium_majority_publishes_at_three_of_four() {
          let key_fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
          let disputed_fields = vec!["/amount_raw".to_owned()];
          let spec = ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: key_fields.clone(),
              critical_fields: vec!["/amount_raw".to_owned()],
          };
          let row_with = |amount: &str| {
              json!({
                  "asset_raw": "Apple Inc Common Stock",
                  "transaction_date_raw": "4/17/2026",
                  "amount_raw": amount,
              })
          };
          let candidates =
              vec![row_with("$15,001 - $50,000"), row_with("$15,001 - $50,000"), row_with("$1,001 - $15,000")];
          let key = row_key(&candidates[0], &key_fields, 0);
          let premium = json!({ "rows": [row_with("$15,001 - $50,000")] });

          let resolved = resolve_disputed(0, &key, &candidates, &disputed_fields, &spec, &premium)
              .expect("2 samples + premium concurring = 3-of-4, must resolve");
          assert_eq!(resolved.row["amount_raw"], json!("$15,001 - $50,000"));
          assert_eq!(resolved.confidence, 0.75f32);
      }

      #[test]
      fn canonical_plane_counting_lets_a_cosmetically_distinct_premium_vote_join_the_majority_group() {
          // The canonical-plane requirement is load-bearing, not redundant with
          // H28's key/agreement evolution: field_resolution has its OWN raw
          // vote-counting loop over candidates + premium, so it needs the SAME
          // canonicalization independently.
          let key_fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
          let disputed_fields = vec!["/amount_raw".to_owned()];
          let spec = ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: key_fields.clone(),
              critical_fields: vec!["/amount_raw".to_owned()],
          };
          let row_with = |amount: &str| {
              json!({
                  "asset_raw": "Apple Inc Common Stock",
                  "transaction_date_raw": "4/17/2026",
                  "amount_raw": amount,
              })
          };
          let candidates =
              vec![row_with("$15,001 - $50,000"), row_with("$15,001 - $50,000"), row_with("$1,001 - $15,000")];
          let key = row_key(&candidates[0], &key_fields, 0);
          // Premium's own transcription has a cosmetic double-space variant of
          // the SAME band the sample majority carries.
          let premium = json!({ "rows": [row_with("$15,001  - $50,000")] });

          let resolved = resolve_disputed(0, &key, &candidates, &disputed_fields, &spec, &premium)
              .expect("canonically-equal premium vote completes the 3-of-4 majority");
          // Modal-verbatim emission: the published value is the SAMPLES'
          // spelling (appears twice in the winning canonical group), never
          // premium's own cosmetically-distinct spelling.
          assert_eq!(resolved.row["amount_raw"], json!("$15,001 - $50,000"));
          assert_eq!(resolved.confidence, 0.75f32);
      }
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: mixed pass/fail against the pre-fix (changeset-only) code —
  - `premium_match_is_occurrence_aware_never_cross_matched_to_occurrence_zero`,
    `premium_siding_with_the_minority_holds_never_publishes_it`, and
    `a_two_one_split_with_premium_majority_publishes_at_three_of_four` already PASS unchanged
    (the changeset's occurrence fix and undeduped counting already make these correct — they are
    regression locks, not the gap this task closes).
  - `a_one_one_one_scatter_with_premium_matching_one_holds_at_two_of_four` FAILS: `assert!
    (resolved.is_none(), ...)` panics because the current `field_resolution` accepts on bare
    plurality (2 votes, no exact tie for first place) with no absolute floor — it returns
    `Some(PublishedRow { .. })`.
  - `canonical_plane_counting_lets_a_cosmetically_distinct_premium_vote_join_the_majority_group`
    FAILS: `resolve_disputed(...).expect(...)` panics — the current `field_resolution` compares raw
    `Value` equality, so premium's double-spaced string matches no candidate's exact string
    (`FieldResolution::Novel`) and `resolve_disputed` returns `None`.

- [ ] **Step 3: Write minimal implementation**

  Replace the `FieldResolution` enum:
  ```rust
  /// One field's resolution against the premium tiebreaker.
  enum FieldResolution {
      /// The winning canonical group reached the strict >= 3-of-4 floor —
      /// this is the modal VERBATIM value within it.
      Resolved(Value),
      /// No canonical group reached the floor (or none at all).
      Held,
  }
  ```

  Replace `field_resolution`'s body:
  ```rust
  /// Design-fixed reader count for one document's escalation: 3 samples + 1
  /// premium pass (design D8 — ONE escalation call per document). Finding
  /// 14's "≥3 of 4" is an absolute vote count against this constant, never a
  /// fraction of a row's own (possibly partial-presence) candidate count.
  const ESCALATION_WIN_THRESHOLD: usize = 3;

  /// Counts UNDEDUPED votes (`candidates`, one entry per carrying sample,
  /// plus the ONE premium vote) in the CANONICAL plane (H28) — cosmetic
  /// transcription variance never fractures a vote. Publishes only on a
  /// strict `>= 3`-of-4-readers majority (amendment-1 §A14); the emitted
  /// value is the modal VERBATIM string among the winning group
  /// (deterministic first-seen tie-break, both at the group level and within
  /// the winning group).
  fn field_resolution(field: &str, candidates: &[Value], premium_row: &Value) -> FieldResolution {
      let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
      for candidate in candidates {
          group_vote(&mut groups, field, candidate);
      }
      group_vote(&mut groups, field, premium_row);

      // First-seen group selection (matches H30b's field_modal exactly —
      // shared style, though with only 4 total votes two groups can never
      // BOTH reach the >= 3 floor at once, so the tie-break direction never
      // actually changes this function's outcome).
      let Some(top_count) = groups.iter().map(|(_, v)| v.len()).max() else {
          return FieldResolution::Held;
      };
      let winner = groups.iter().find(|(_, v)| v.len() == top_count);
      match winner {
          Some((_, verbatims)) if verbatims.len() >= ESCALATION_WIN_THRESHOLD => {
              FieldResolution::Resolved(modal_verbatim(verbatims))
          }
          _ => FieldResolution::Held,
      }
  }

  /// Adds one row's value at `field` to its canonical-plane vote group,
  /// creating the group on first sight. First-seen group order is preserved
  /// (grouping key = `canonical_field`'s output; group contents keep the
  /// verbatim strings so the modal value can be recovered).
  fn group_vote(groups: &mut Vec<(String, Vec<Value>)>, field: &str, row: &Value) {
      let verbatim = row.pointer(field).cloned().unwrap_or(Value::Null);
      let canon = canonical_field(field, &verbatim);
      match groups.iter_mut().find(|(existing, _)| *existing == canon) {
          Some((_, verbatims)) => verbatims.push(verbatim),
          None => groups.push((canon, vec![verbatim])),
      }
  }

  /// The most-frequent verbatim string within one canonical group; ties break
  /// to first sample order (the order `verbatims` was populated in — samples
  /// in sample order, premium last).
  fn modal_verbatim(verbatims: &[Value]) -> Value {
      let mut counts: Vec<(&Value, u32)> = Vec::new();
      for v in verbatims {
          match counts.iter_mut().find(|(existing, _)| *existing == v) {
              Some(entry) => entry.1 += 1,
              None => counts.push((v, 1)),
          }
      }
      let mut best: Option<(&Value, u32)> = None;
      for (value, count) in counts {
          let replace = match best {
              None => true,
              Some((_, best_count)) => count > best_count,
          };
          if replace {
              best = Some((value, count));
          }
      }
      best.map_or(Value::Null, |(v, _)| v.clone())
  }
  ```

  Update `resolve_disputed`'s match arm — replace:
  ```rust
              FieldResolution::Novel | FieldResolution::Tied => return None,
  ```
  with:
  ```rust
              FieldResolution::Held => return None,
  ```
  (`FieldResolution::Resolved(value) => { ... }`'s arm body is unchanged — it already just assigns
  `value` into the resolved row's pointer slot.) `premium_row_at` is unchanged — do not edit it.

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: PASS — all 5 `escalation_hardening_tests` tests green, plus every prior test module
  (`tests`, `align_tests`, `score_tests`, `sanity_tests`, `canonical_tests`) still green.
  Then: `cargo test -p pipeline --test consensus_extraction` PASS — Task 13/17's e2e escalation tests
  unaffected (their premium fixtures never depend on plurality-vs-majority edge cases).
  Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
  ```bash
  git add crates/pipeline/src/extraction/consensus.rs
  git commit -m "$(cat <<'EOF'
  fix(pipeline): strict >=3-of-4 canonical-plane escalation acceptance (goal 021 Phase 3, Task H29)

  field_resolution now requires an absolute >= 3-of-4-readers vote floor
  (amendment-1 A14) instead of bare plurality, counts votes in the canonical
  plane (H28) instead of raw string equality, and emits the modal verbatim
  value within the winning group instead of copying premium's own string.
  premium_row_at's occurrence-aware matching (A1, already landed by the
  surgical changeset) is unchanged and regression-locked by this task's tests.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
  EOF
  )"
  ```

  Per-task green gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H30: Family-aware voting + `[families]` config + pol2 (finding D, D5 / amendment-1 §2, §4)

Goal `021-llm-extraction.md` §"Phase 3 goal text (verbatim)" §3 "D — Cross-lab third vote": "2×Haiku
agreement + cross-lab dissent on a critical field = DISPUTE (escalate), never a 2/3 publish.
Cross-lab concurrence keeps 0.90 (ceiling unchanged...)." Amendment-1 §2 (the normative policy table
this task implements) and §4: "Family-aware semantics are CODED in pol2 and degenerate to same-family
rules while the vote set is 3×Haiku (property-tested), so enabling the cross-lab vote changes only
the composite's models component." `family_of`, `POLICY_VERSION` → `"pol2"` (the
landed Task-1 const — forward evolution; this task does NOT touch `config/extractor.toml` or
Task 9's `composite_model_id` tests). Slot note: executes after H29, before H30b (this cluster's
own ordering) and after committed Tasks 1–17 (goal `021-llm-extraction.md` "Execution (merged
order)" checklist).

**POLICY_VERSION divergence callout (transient, harmless):** Task 9's committed
`config/extractor.toml` (AMENDED, goal 021 Phase 3) already ships `[versions] policy = "pol2"`
from its OWN edit — NOT `"pol1"`. This task's Rust-side `policy::POLICY_VERSION` const is what
lags, at `"pol1"` through H27–H29's window, only catching up to `"pol2"` here at H30. That
window's divergence (TOML says pol2, Rust const says pol1) is harmless: the composite policy
only goes LIVE at Task 24's cutover (no production caller exists pre-cutover, per this doc's
Global Constraints), so no cache rows exist under any interim combination.

**Why this task's code diff is small:** `score`/`route`/`field_resolution` classify agreement by
requiring literally ALL N samples to agree (0.90) or by an absolute reader-count floor regardless of
which reader cast which vote (0.75, H29) — neither is parameterized by which model produced a
candidate. A cross-family dissent on a critical field therefore ALREADY breaks the 0.90 path (it is
not unanimous) and a 2-same-family-vote pair can NEVER reach H29's `>= 3`-of-4 floor by itself (2 of
4 < 3) — family-awareness is dormant by construction, not a new branch to add. This task's job is the
config-loading primitive (`family_of`, reading Task 9's already-landed plain `families:
BTreeMap<String, String>` field, for future audit/reporting consumers — H38/H40)
and the version-tag bump; it explicitly does NOT thread family identity into `score`/`route`/
`field_resolution`.

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` — bump `policy::POLICY_VERSION`
- Modify: `crates/pipeline/src/extraction/config.rs` — add `family_of` ONLY (the `families:
  BTreeMap<String, String>` field on `ExtractorConfig` is ALREADY landed by Task 9 — plain map,
  no wrapper type; this task does not re-declare it and does not touch `test_cfg()`)
- Test: `crates/pipeline/src/extraction/consensus.rs` (new inline `#[cfg(test)] mod pol2_tests`);
  `crates/pipeline/src/extraction/config.rs` (new inline tests in its existing `#[cfg(test)] mod
  tests`)

**Interfaces:**
- Consumes: committed Task 1's `pub mod policy { pub const POLICY_VERSION: &str = "pol1"; ... }`;
  committed Task 9's (AMENDED, goal 021 Phase 3) `pub struct ExtractorConfig { pub models:
  ModelsConfig, pub sampling: SamplingConfig, pub preprocess: PreprocessConfig, pub pricing:
  BTreeMap<String, ModelPricing>, pub budget: BudgetConfig, pub versions: VersionsConfig, pub
  quality: QualityConfig, pub escalation: EscalationConfig, pub families: BTreeMap<String,
  String>, pub audit: AuditConfig, pub drift: DriftConfig, pub cross_lab: CrossLabConfig }`
  (12 fields — `families` is ALREADY a plain `BTreeMap<String, String>`, no wrapper type) and
  `impl ExtractorConfig { pub fn load_from(path: &Path, lookup: impl Fn(&str) -> Option<String>)
  -> anyhow::Result<Self>; ... }`; committed Task 3/H28's `align`/`score`/`route`/`RowVerdict`
  pipeline (used unmodified by the property test).
- Produces (shared contract §"H30 — family-aware + pol2"): `pub fn family_of<'a>(cfg: &'a
  ExtractorConfig, model_id: &str) -> Option<&'a str>` (reads the already-landed `families` field
  — no new struct, no new field); `policy::POLICY_VERSION == "pol2"`.

- [ ] **Step 1: Write the failing tests**

  Add to `crates/pipeline/src/extraction/config.rs`'s existing `#[cfg(test)] mod tests { .. }` block
  (alongside `parses_the_committed_config_file` etc. — do not remove or reorder existing tests):
  ```rust
  #[test]
  fn family_of_is_none_for_an_unconfigured_model_and_some_for_a_configured_one() {
      use std::collections::BTreeMap;

      let mut families: BTreeMap<String, String> = BTreeMap::new();
      families.insert("claude-haiku-4-5-20251001".to_owned(), "anthropic".to_owned());
      let mut cfg = ExtractorConfig::load_from(&config_path(), |_| None).unwrap();
      cfg.families = families;

      assert_eq!(family_of(&cfg, "claude-haiku-4-5-20251001"), Some("anthropic"));
      assert_eq!(
          family_of(&cfg, "gemini-3-flash-lite"),
          None,
          "an unmapped model is UNKNOWN family, never assumed same-family"
      );
  }

  #[test]
  fn families_config_defaults_to_empty_when_absent_from_the_committed_toml() {
      // config/extractor.toml ships with no [families] table today (H44 wires
      // the first entry in) — #[serde(default)] must still parse cleanly.
      // `families` is Task 9's landed plain `BTreeMap<String, String>`, no
      // wrapper type.
      let cfg = ExtractorConfig::load_from(&config_path(), |_| None).unwrap();
      assert!(cfg.families.is_empty());
  }
  ```

  Append to `crates/pipeline/src/extraction/consensus.rs`:
  ```rust
  #[cfg(test)]
  #[allow(clippy::unwrap_used, clippy::float_cmp)]
  mod pol2_tests {
      use serde_json::json;

      use super::*;

      #[test]
      fn policy_version_is_pol2() {
          assert_eq!(policy::POLICY_VERSION, "pol2");
      }

      fn dummy_tool() -> DocumentToolSpec {
          DocumentToolSpec {
              tool_name: "record_ptr_transactions".to_owned(),
              tool_description: "test".to_owned(),
              input_schema: json!({}),
              prompt: "test".to_owned(),
          }
      }

      fn spec() -> ConsensusSpec {
          ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
              critical_fields: vec!["/amount_raw".to_owned(), "/transaction_type_raw".to_owned()],
          }
      }

      fn row(asset: &str, date: &str, amount: &str, kind: &str) -> Value {
          json!({
              "asset_raw": asset,
              "transaction_date_raw": date,
              "amount_raw": amount,
              "transaction_type_raw": kind,
          })
      }

      fn sample(rows: Vec<Value>) -> Value {
          json!({ "rows": rows })
      }

      /// Table-driven equivalence check (this workspace has no proptest/
      /// quickcheck dependency — see Task 9's Cargo.toml; a hand-enumerated
      /// disagreement matrix matches the existing test-authoring convention):
      /// for a SINGLE-family vote set (every sample tagged the same lab, or
      /// no [families] entries at all — the committed config's actual state
      /// today), `align`/`score`/`route`'s pol2-tagged output byte-matches
      /// what Task 13's own e2e tests already assert for pol1 — because
      /// neither function is parameterized by policy_version OR by family at
      /// all. Reuses Task 13's three disagreement-matrix fixtures verbatim.
      #[test]
      fn single_family_vote_sets_reproduce_the_task_13_pol1_verdicts_byte_for_byte() {
          let s = spec();

          // Matrix 1 (Task 13's full-agreement fixture): 3/3 agree -> 0.90.
          let r1 = row("Apple Inc Common Stock", "4/17/2026", "$15,001 - $50,000", "P");
          let r2 = row("Tesla Inc Common Stock", "4/18/2026", "$1,001 - $15,000", "P");
          let samples = vec![
              sample(vec![r1.clone(), r2.clone()]),
              sample(vec![r1.clone(), r2.clone()]),
              sample(vec![r1.clone(), r2.clone()]),
          ];
          let aligned = align(&samples, &s).unwrap();
          let outcome = route(score(&aligned, &s), &s, &no_sanity_check, None);
          assert_eq!(outcome.held.len(), 0);
          assert_eq!(outcome.published.len(), 2);
          for published in &outcome.published {
              assert_eq!(published.confidence, 0.9f32, "pol2 single-family ceiling == pol1's");
          }

          // Matrix 2 (Task 13's partial-dispute fixture): one row disagrees,
          // its sibling still publishes, no escalation pass supplied -> hold.
          let r2_disagree = {
              let mut v = r2.clone();
              v["amount_raw"] = json!("$15,001 - $50,000");
              v
          };
          let samples2 = vec![
              sample(vec![r1.clone(), r2.clone()]),
              sample(vec![r1.clone(), r2.clone()]),
              sample(vec![r1.clone(), r2_disagree]),
          ];
          let aligned2 = align(&samples2, &s).unwrap();
          let outcome2 = route(score(&aligned2, &s), &s, &no_sanity_check, None);
          assert_eq!(outcome2.published.len(), 1);
          assert_eq!(outcome2.published[0].confidence, 0.9f32);
          assert_eq!(outcome2.held.len(), 1);
          assert_eq!(outcome2.held[0].competing.len(), 3, "undeduped candidates, unchanged by pol2");
      }

      /// Mixed-family fixture (goal §D, amendment-1 §4): 2×haiku agree, a
      /// cross-lab dissent on a critical field -> Disputed, NEVER a 2/3
      /// publish at 0.90 — enforced by construction (Agreed requires ALL N
      /// samples, not a majority). After premium concurs with the haiku pair
      /// -> 3-of-4 -> 0.75, NEVER 0.90 (the ceiling requires full-sample
      /// unanimity, H29's floor is an absolute reader count, not a "family
      /// quorum").
      #[test]
      fn mixed_family_dissent_disputes_then_resolves_at_the_escalated_ceiling_never_agreed() {
          let s = spec();
          let haiku_row = row("Apple Inc Common Stock", "4/17/2026", "$15,001 - $50,000", "P");
          let gemini_dissent_row = row("Apple Inc Common Stock", "4/17/2026", "$1,001 - $15,000", "P");
          // Sample model_ids are carried for audit/family-lookup purposes
          // only (family_of, config.rs) — align/score/route never consult
          // them, which is exactly this task's "dormant" claim under test.
          let samples = vec![
              sample(vec![haiku_row.clone()]),   // claude-haiku-4-5-20251001
              sample(vec![haiku_row.clone()]),   // claude-haiku-4-5-20251001
              sample(vec![gemini_dissent_row]),  // gemini-3-flash-lite (hypothetical, H44)
          ];
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          assert!(
              matches!(verdicts[0], RowVerdict::Disputed { .. }),
              "2 haiku agree + 1 cross-lab dissent must NEVER publish at 0.90 on a 2/3 majority"
          );

          let premium = SamplePass {
              model_id: "claude-sonnet-5".to_owned(),
              payload: sample(vec![haiku_row]),
              usage: json!({}),
          };
          let outcome = route(verdicts, &s, &no_sanity_check, Some(&premium));
          assert_eq!(outcome.published.len(), 1);
          assert_eq!(
              outcome.published[0].confidence, 0.75f32,
              "3-of-4 (2 haiku + premium) resolves at CONF_ESCALATED, never CONF_AGREED"
          );
      }
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p pipeline --lib extraction::config::`
  Expected: FAIL to compile — `error[E0425]: cannot find function 'family_of' in this scope` (the
  only missing symbol — Task 9 already landed the `families: BTreeMap<String, String>` field on
  `ExtractorConfig`, so `cfg.families = families` compiles fine on its own).

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: `policy_version_is_pol2` FAILS — `assertion 'left == right' failed... left: "pol1",
  right: "pol2"` (the const has not been bumped yet). The other two `pol2_tests` tests already
  compile and PASS against the current `pol1`-tagged code (they assert behavior, not the version
  string — this is the "dormant" property holding even before the bump).

- [ ] **Step 3: Write minimal implementation**

  `ExtractorConfig`'s `families: BTreeMap<String, String>` field is ALREADY landed by Task 9
  (AMENDED, goal 021 Phase 3) — this task adds ONLY the lookup function, no new struct, no new
  field. In `crates/pipeline/src/extraction/config.rs`, add (near `composite_model_id`):
  ```rust
  /// Looks up `model_id`'s configured lab family. `None` when the model has
  /// no `[families]` entry — an absent mapping is UNKNOWN, never treated as
  /// "same family" as anything else (fail-closed default for the
  /// family-aware voting semantics documented on `consensus::policy`).
  #[must_use]
  pub fn family_of<'a>(cfg: &'a ExtractorConfig, model_id: &str) -> Option<&'a str> {
      cfg.families.get(model_id).map(String::as_str)
  }
  ```

  `crates/pipeline/tests/consensus_extraction.rs`'s `test_cfg()` needs NO change for this task —
  `families` has been part of `ExtractorConfig`'s shape since Task 9, not added incrementally
  here, so any existing construction of it already accounts for the field.

  In `crates/pipeline/src/extraction/consensus.rs`, bump the version constant:
  ```rust
      pub const POLICY_VERSION: &str = "pol2";
  ```
  (Leave every other `policy` item — `CONF_AGREED`, `CONF_ESCALATED`, `CONF_SANITY_CAPPED` — and
  their doc comments untouched. `config/extractor.toml`'s `[versions] policy = "pol2"` line has
  been in place since Task 9's edit and does not change here; `config.rs`'s
  `composite_model_id_matches_the_documented_format` test stays AS-IS. Only this task's Rust-side
  `POLICY_VERSION` const — lagging at `"pol1"` through H27–H29 — catches up to `"pol2"` here; the
  composite policy itself goes LIVE only at committed Task 24's single atomic cutover, amendment-1
  §3.)

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p pipeline --lib extraction::config::`
  Expected: PASS — all prior config tests plus the 2 new ones green.
  Run: `cargo test -p pipeline extraction::consensus`
  Expected: PASS — all 3 `pol2_tests` tests green, plus every prior test module still green.
  Run: `cargo test -p pipeline --test consensus_extraction`
  Expected: PASS — `test_cfg()` already compiles unchanged (no new field to add — Task 9 landed
  `families` from the start); all six existing tests unchanged.
  Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
  ```bash
  git add crates/pipeline/src/extraction/consensus.rs crates/pipeline/src/extraction/config.rs
  git commit -m "$(cat <<'EOF'
  feat(pipeline): family_of lab-family lookup + POLICY_VERSION pol2 (goal 021 Phase 3, Task H30)

  family_of (config.rs) reads the families: BTreeMap<String, String> field
  Task 9 already landed on ExtractorConfig -- no new field, no wrapper type
  -- plus the pol2 version-tag bump (consensus.rs's Rust-side const only;
  config/extractor.toml's versions.policy has said pol2 since Task 9's edit
  -- the composite goes LIVE only at Task 24's single atomic cutover,
  amendment-1 §3). Family-aware voting semantics (goal §D)
  are dormant by construction: score/route/field_resolution require full
  N-sample unanimity for 0.90 and an absolute >=3-of-4 reader floor for 0.75
  regardless of which model cast which vote, so a single-family vote set
  reproduces pol1's verdicts byte for byte (property-tested).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
  EOF
  )"
  ```

  Per-task green gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H30b: Field-wise modal publication — Agreed branch only (finding 6 / amendment-1 §A6)

Amendment-1 §A6: "On the Agreed branch, publish the per-field modal value across the 3 candidates in
the canonical plane rather than always taking `candidates[0]`; the published value is always one of
the models' own verbatim strings; a 3-way tie takes the first sample and records the tie in stats.
The escalation-resolved (Disputed) branch is explicitly OUT of this change — Disputed carries
per-sample undeduped candidates and reworking that vote multiplicity is not in scope here." `score()`'s
`Agreed` arm currently ships `candidates[0]` verbatim (a 1-of-3 misread on any unvoted secondary field
publishes at 0.90 unexamined); this task replaces that with the per-field modal (canonical plane)
value, reusing H29's `group_vote`/`modal_verbatim`. Slot note: executes after H30 (this cluster's
ordering); reuses H29's helpers, so it also requires H29 landed.

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` — replace `score`'s `RowVerdict::Agreed`
  construction; add `modal_row`/`field_modal` helpers; evolve `summarize_agreement` again to add the
  `modal_ties` counter
- Test: `crates/pipeline/src/extraction/consensus.rs` (new inline `#[cfg(test)] mod modal_tests`)

**Interfaces:**
- Consumes: committed Task 3's `pub fn score(aligned: &AlignedRows, spec: &ConsensusSpec) ->
  Vec<RowVerdict>` (unchanged signature, evolved `Agreed` construction); H29's `fn group_vote(groups:
  &mut Vec<(String, Vec<Value>)>, field: &str, row: &Value)` and `fn modal_verbatim(verbatims:
  &[Value]) -> Value`; H28's evolved `fn summarize_agreement(verdicts: &[RowVerdict], aligned:
  &AlignedRows, spec: &ConsensusSpec) -> serde_json::Value`.
- Produces (shared contract §"H30b — modal publication"): private `fn modal_row(candidates: &[Value])
  -> Value` and `fn field_modal(field: &str, candidates: &[Value]) -> (Value, bool)` (the `bool` is
  "this field's group selection was an unresolved tie"); `summarize_agreement`'s returned `Value`
  gains `"modal_ties": <u32>`.

- [ ] **Step 1: Write the failing tests**

  Append to `crates/pipeline/src/extraction/consensus.rs`:
  ```rust
  #[cfg(test)]
  #[allow(clippy::unwrap_used, clippy::float_cmp)]
  mod modal_tests {
      use serde_json::json;

      use super::*;

      fn dummy_tool() -> DocumentToolSpec {
          DocumentToolSpec {
              tool_name: "record_ptr_transactions".to_owned(),
              tool_description: "test".to_owned(),
              input_schema: json!({}),
              prompt: "test".to_owned(),
          }
      }

      fn spec() -> ConsensusSpec {
          ConsensusSpec {
              tool: dummy_tool(),
              rows_pointer: "/rows".to_owned(),
              key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
              // filing_status_raw is a SECONDARY (non-critical) field — this
              // is exactly the class finding 6 targets: an unvoted field a
              // 1-of-3 misread can slip through on.
              critical_fields: vec!["/amount_raw".to_owned()],
          }
      }

      fn sample(rows: Vec<Value>) -> Value {
          json!({ "rows": rows })
      }

      fn row(status: &str) -> Value {
          json!({
              "asset_raw": "Apple Inc Common Stock",
              "transaction_date_raw": "4/17/2026",
              "amount_raw": "$15,001 - $50,000",
              "filing_status_raw": status,
          })
      }

      #[test]
      fn agreed_row_publishes_the_two_of_three_modal_value_not_candidates_zero() {
          // candidates[0] (sample 0, doc order) carries a 1-of-3 misread;
          // samples 1 and 2 agree — committed code shipped candidates[0]'s
          // "New" at 0.90 unexamined (finding 6).
          let samples = vec![sample(vec![row("New")]), sample(vec![row("Amended")]), sample(vec![row("Amended")])];
          let s = spec();
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          match &verdicts[0] {
              RowVerdict::Agreed { row, .. } => {
                  assert_eq!(
                      row["filing_status_raw"], json!("Amended"),
                      "modal publication must carry the 2-of-3 value, not candidates[0]'s misread"
                  );
              }
              other => panic!("expected Agreed, got {other:?}"),
          }
      }

      #[test]
      fn a_full_three_way_split_on_a_secondary_field_falls_back_to_first_sample_and_counts_it() {
          let samples =
              vec![sample(vec![row("New")]), sample(vec![row("Amended")]), sample(vec![row("Corrected")])];
          let s = spec();
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          match &verdicts[0] {
              RowVerdict::Agreed { row, .. } => {
                  assert_eq!(row["filing_status_raw"], json!("New"), "3-way tie falls back to first sample");
              }
              other => panic!("expected Agreed, got {other:?}"),
          }
          let agreement = summarize_agreement(&verdicts, &aligned, &s);
          assert_eq!(agreement["modal_ties"], json!(1u32));
      }

      #[test]
      fn asset_description_raw_is_never_canonicalized_in_the_published_row() {
          // invariant 2 ("asset_description_raw always stored" — CLAUDE.md;
          // the consensus row's own raw field for this is asset_raw, copied
          // to asset_description_raw at the Silver normalize layer, see
          // crates/adapters/us_house/src/normalize.rs). Even full agreement
          // must publish the model's own verbatim spelling, never
          // canonical_field's collapsed form.
          let raw_variant = "Apple  Inc.  Common  Stock"; // extra whitespace + a period
          let noisy_row = |status: &str| {
              json!({
                  "asset_raw": raw_variant,
                  "transaction_date_raw": "4/17/2026",
                  "amount_raw": "$15,001 - $50,000",
                  "filing_status_raw": status,
              })
          };
          let samples =
              vec![sample(vec![noisy_row("New")]), sample(vec![noisy_row("New")]), sample(vec![noisy_row("New")])];
          let s = spec();
          let aligned = align(&samples, &s).unwrap();
          let verdicts = score(&aligned, &s);
          match &verdicts[0] {
              RowVerdict::Agreed { row, .. } => {
                  assert_eq!(
                      row["asset_raw"], json!(raw_variant),
                      "invariant 2: published value is always a verbatim model string"
                  );
              }
              other => panic!("expected Agreed, got {other:?}"),
          }
      }
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: `agreed_row_publishes_the_two_of_three_modal_value_not_candidates_zero` FAILS —
  `assertion 'left == right' failed... left: "New", right: "Amended"` (current `score` ships
  `candidates[0]`'s "New" unexamined). `a_full_three_way_split_on_a_secondary_field_falls_back_to_
  first_sample_and_counts_it` FAILS at its LAST assertion — `agreement["modal_ties"]` does not exist
  on the current `summarize_agreement` output (`Value::Null != json!(1u32)`); its first assertion
  ("New" == candidates[0]) already coincidentally passes today, since candidates[0] IS the first
  sample. `asset_description_raw_is_never_canonicalized_in_the_published_row` already PASSES
  unchanged (score's Agreed arm has never canonicalized the published value — this is a regression
  lock for the invariant this task must preserve while rewriting the Agreed arm).

- [ ] **Step 3: Write minimal implementation**

  Insert the modal-row helpers (anywhere after H29's `group_vote`/`modal_verbatim`):
  ```rust
  /// Per-field modal value across `candidates` in the canonical plane
  /// (amendment-1 §A6): groups by `canonical_field`, returns the modal
  /// VERBATIM string within the winning group plus whether the group
  /// selection itself was an unresolved tie (every group the same size —
  /// e.g. a full 3-way split with no majority reading).
  fn field_modal(field: &str, candidates: &[Value]) -> (Value, bool) {
      let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
      for candidate in candidates {
          group_vote(&mut groups, field, candidate);
      }
      let Some(top_count) = groups.iter().map(|(_, v)| v.len()).max() else {
          return (Value::Null, false);
      };
      let tie = groups.iter().filter(|(_, v)| v.len() == top_count).count() > 1;
      let winner = groups
          .iter()
          .find(|(_, v)| v.len() == top_count)
          .map_or(Value::Null, |(_, v)| modal_verbatim(v));
      (winner, tie)
  }

  /// Builds an Agreed row's published value: every top-level field of
  /// `candidates[0]`'s own shape, replaced field-by-field with its modal
  /// value across ALL candidates (never a straight copy of any one
  /// candidate — invariant 2 still holds because every emitted value is
  /// SOME candidate's own verbatim string, per `modal_verbatim`).
  fn modal_row(candidates: &[Value]) -> Value {
      let Some(base) = candidates.first() else {
          return Value::Null;
      };
      let Some(base_obj) = base.as_object() else {
          return base.clone();
      };
      let mut out = serde_json::Map::new();
      for key in base_obj.keys() {
          let pointer = format!("/{key}");
          let (value, _tie) = field_modal(&pointer, candidates);
          out.insert(key.clone(), value);
      }
      Value::Object(out)
  }
  ```

  In `score`, replace the `Agreed` construction — change:
  ```rust
              if aligned_row.presence == PresenceClass::InAll && disputed_fields.is_empty() {
                  RowVerdict::Agreed {
                      ordinal0: aligned_row.ordinal0,
                      row: aligned_row.candidates[0].clone(),
                  }
  ```
  to:
  ```rust
              if aligned_row.presence == PresenceClass::InAll && disputed_fields.is_empty() {
                  RowVerdict::Agreed {
                      ordinal0: aligned_row.ordinal0,
                      row: modal_row(&aligned_row.candidates),
                  }
  ```
  (The `Disputed` arm is untouched — finding 6 / amendment-1 §A6 explicitly excludes it.)

  Evolve `summarize_agreement` once more — add the `modal_ties` tally into the existing per-row loop
  (from H28's evolution) and the returned `json!` literal:
  ```rust
      let mut canonical_merges = 0u32;
      let mut verbatim_variants = 0u32;
      let mut modal_ties = 0u32;
      for (verdict, row) in verdicts.iter().zip(&aligned.rows) {
          let has_verbatim_variance = spec.critical_fields.iter().any(|field| {
              let mut raw = row.candidates.iter().map(|candidate| candidate.pointer(field));
              let Some(first) = raw.next() else {
                  return false;
              };
              raw.any(|value| value != first)
          });
          if has_verbatim_variance {
              verbatim_variants += 1;
              if disagreeing_fields(&row.candidates, &spec.critical_fields).is_empty() {
                  canonical_merges += 1;
              }
          }
          if matches!(verdict, RowVerdict::Agreed { .. }) {
              if let Some(obj) = row.candidates.first().and_then(Value::as_object) {
                  for key in obj.keys() {
                      let pointer = format!("/{key}");
                      let (_, tie) = field_modal(&pointer, &row.candidates);
                      if tie {
                          modal_ties += 1;
                      }
                  }
              }
          }
      }

      serde_json::json!({
          "total_rows": verdicts.len(),
          "agreed": agreed,
          "disputed": disputed,
          "canonical_merges": canonical_merges,
          "verbatim_variants": verbatim_variants,
          "modal_ties": modal_ties,
      })
  ```
  (This replaces H28's separate `for row in &aligned.rows` loop with the single `zip`-based loop
  above — one pass over `verdicts`/`aligned.rows` in lockstep, since they are 1:1 and same-order by
  construction from `score`.)

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test -p pipeline extraction::consensus`
  Expected: PASS — all 3 `modal_tests` tests green, plus every prior test module (`tests`,
  `align_tests`, `score_tests`, `sanity_tests`, `canonical_tests`, `escalation_hardening_tests`,
  `pol2_tests`) still green — including H28's `summarize_agreement_counts_canonical_merges_and_
  verbatim_variants` test, whose assertions are unaffected by the added `modal_ties` key.
  Then: `cargo test -p pipeline --test consensus_extraction` PASS — Task 13/17's e2e tests still
  green (their fixtures are already fully verbatim-agreed, so modal publication is a no-op on them).
  Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
  ```bash
  git add crates/pipeline/src/extraction/consensus.rs
  git commit -m "$(cat <<'EOF'
  feat(pipeline): field-wise modal publication on the Agreed branch (goal 021 Phase 3, Task H30b)

  score()'s Agreed arm now publishes the per-field modal (canonical-plane)
  value across all N candidates instead of always taking candidates[0] —
  amendment-1 A6. A 1-of-3 secondary-field misread no longer ships
  unexamined; a full N-way tie falls back to the first sample and is counted
  in summarize_agreement's new modal_ties stat. The escalation-resolved
  (Disputed) branch is untouched, per finding 6's explicit scope carve-out.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
  EOF
  )"
  ```

  Per-task green gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H31: QualityMetrics + quality-routed vote sets (finding 16)

**This task deliberately exceeds the ~1h guideline** — it spans `preprocess.rs`'s
quality-measurement pipeline, `config.rs`'s `QualityConfig`, and `consensus.rs`'s escalation
trigger as one coherent unit; splitting would leave `doc_quality_flagged` computed but wired to
only some of its consumers between commits. Do not split it.

Free preprocess by-products (residual skew, Otsu between-class variance, noise count) become
a measured `QualityMetrics` signal that routes a flagged document through the premium pass
up front instead of waiting for a dispute — goal 021 Phase 3 §3 finding 16. This task also
implements the Phase-2 controller's `_high_impact_document` discard-fix groundwork by
establishing the general "premium concordance check on Agreed rows" mechanism H32 later
reuses for the pixel/high-impact triggers.

**Files:**
- Modify: `crates/pipeline/src/extraction/preprocess.rs` (add `QualityMetrics`,
  `PreprocessOutput`; split `otsu_threshold` into `otsu_threshold_with_variance`; add
  `count_isolated_ink_pixels`, `preprocess_page_with_quality`, `page_quality`; change
  `preprocess_document`'s return type Task 8 shipped as `Vec<Vec<u8>>`)
- Modify: `crates/pipeline/tests/preprocess.rs` (extend `generate_preprocess_fixtures` with a
  new committed fixture; update the two Task 8 `preprocess_document` tests for the new return
  shape; add quality-measurement tests)
- Modify: `crates/pipeline/src/extraction/config.rs` (EDIT Task 9's already-landed
  `QualityConfig` in place — calibrated `impl Default` replacing the derived one; add
  `doc_quality_flagged`. `ExtractorConfig.quality`, `VersionsConfig.quality`, and
  `composite_model_id`'s quality folding are ALREADY present from Task 9 — no edit needed there)
- Modify: `crates/pipeline/src/extraction/consensus.rs` (`ConsensusExtractor`'s `images()`
  test/prod seam returns `PreprocessOutput`; `with_fixed_images` test constructor gains a
  `with_fixed_images_and_quality` sibling; `extract()`'s escalation trigger becomes
  `has_dispute || quality_flagged`; `route` gains a premium-concordance check on `Agreed` rows)
- Modify: `crates/pipeline/tests/consensus_extraction.rs` (`test_cfg()` gains the `quality`
  field; new pivotal tests)

**Interfaces:**
- Consumes: Task 6/8's `preprocess_page`/`preprocess_document`/`PreprocessCfg`/`NormRect`
  (`crates/pipeline/src/extraction/preprocess.rs`, unchanged public signature for
  `preprocess_page`); Task 9's `ExtractorConfig`/`VersionsConfig`/`composite_model_id`
  (`crates/pipeline/src/extraction/config.rs`); Task 17's `ConsensusExtractor::extract`,
  `route`, `SamplePass`, `RowVerdict`, `ConsensusSpec`, `premium_row_at` (private, same module
  — this task calls it directly), H28's `canonical_field(pointer: &str, value: &Value) ->
  String` (`crates/pipeline/src/extraction/consensus.rs` — canonical-plane comparison,
  invariant 2: publish stays verbatim, only the DISSENT CHECK compares canonically).
- Produces: `pub struct QualityMetrics { pub residual_skew_deg: f32, pub otsu_variance: f32,
  pub noise_count: u32 }` (`Debug, Clone, Copy, PartialEq`); `pub struct PreprocessOutput {
  pub pages_png: Vec<Vec<u8>>, pub quality: Vec<QualityMetrics> }` (`Debug, Clone`); `pub fn
  preprocess_document(pdf: &[u8], cfg: &PreprocessCfg) -> anyhow::Result<PreprocessOutput>`
  (return-shape evolution of Task 8's `Vec<Vec<u8>>` — same error semantics, same
  `PdfiumUnavailable` propagation); `pub fn page_quality(png: &[u8], max_edge: u32) ->
  anyhow::Result<QualityMetrics>` (single-page quality measurement without the PNG
  re-encode, used by this task's fixture-calibration tests); an EDIT to Task 9's already-landed
  `pub struct QualityConfig { pub max_residual_skew_deg: f32, pub min_otsu_variance: f32, pub
  max_noise_count: u32 }` — this task replaces its DERIVED `Default` (all-zero thresholds) with
  the calibrated `impl Default` = the shipped 1.5 / 0.02 / 1200 values this task's measurement
  procedure derives; `pub fn doc_quality_flagged(cfg: &ExtractorConfig, quality:
  &[QualityMetrics]) -> bool` (new). `ExtractorConfig.quality: QualityConfig` and
  `VersionsConfig.quality: String` (folded into `composite_model_id` as `+{quality}`) are
  ALREADY present on Task 9's landed `ExtractorConfig`/`VersionsConfig` (12-field shape,
  escalation/families/audit/drift/cross_lab included) — this task does not touch either struct's
  field list. `ConsensusExtractor::with_fixed_images_and_quality(transport, cfg, images:
  Vec<Vec<u8>>, quality: Vec<QualityMetrics>) -> Self` (`#[cfg(test)]`, additive — the existing
  3-arg `with_fixed_images(transport, cfg, images)` keeps its exact call sites from Tasks
  13/17 unchanged, now implemented in terms of this new constructor with an all-clean
  `QualityMetrics` vector). Evolves the escalation-aware `route` (chain 3 → 4 → 17 → **H31**):
  signature unchanged (`route(verdicts, spec, sanity, escalation) -> DocOutcome`), but the
  `Agreed` arm now also checks the escalation pass (when present, for ANY reason) against the
  row's critical fields and caps to `policy::CONF_SANITY_CAPPED` (0.79) on canonical-plane
  dissent — this is the GENERAL mechanism finding 15/16 both route through; H32 does not
  redefine it, only adds more ways to make `escalation: Some`.

Read `crates/pipeline/src/extraction/consensus.rs` and `config.rs` on your branch before
writing code (they are under active concurrent amendment by other Phase-3 hardening
clusters) — if a field/enum differs slightly from what is written here (e.g. `SamplingParams`
already carries an `effort` field, or `Effort` already exists), match the real file for
anything this task does not itself define, and use `SamplingParams { temperature: ...,
..Default::default() }` (it derives `Default`) rather than a full positional literal, so this
task's code does not fight over fields Task H33 owns.

- [ ] **Step 1: Write the failing test**

Append to `crates/pipeline/tests/preprocess.rs`, inside the fixture generator (extend the
existing `generate_preprocess_fixtures` test, same `GOVFOLIO_GENERATE_PREPROCESS_FIXTURES`
gate — do not add a second env var):

```rust
/// Deterministic salt-noise copy of `generate_skewed_block()`: flips every
/// pixel where `(x*7 + y*13) % 97 == 0` (no RNG anywhere — byte-deterministic
/// across runs, same discipline as every other Task 6 fixture generator).
/// Most flips land in the large white background (the 600x100 block covers
/// ~13% of the 900x500 canvas), producing thousands of isolated dark specks
/// post-binarization — the `noise_count` quality signal this fixture exists
/// to trip. This is the H31 "degraded scan" calibration fixture.
fn generate_noisy_skewed_block() -> GrayImage {
    let mut img = generate_skewed_block();
    let (width, height) = img.dimensions();
    for y in 0..height {
        for x in 0..width {
            if (x * 7 + y * 13) % 97 == 0 {
                let pixel = img.get_pixel_mut(x, y);
                pixel.0[0] = 255 - pixel.0[0];
            }
        }
    }
    img
}
```

Add the save call inside the existing `generate_preprocess_fixtures` test body, alongside the
three Task 6 saves:

```rust
    generate_noisy_skewed_block()
        .save(dir.join("noisy_skewed_block.png"))
        .unwrap();
```

Then append two new tests to the same file:

```rust
#[test]
fn clean_fixture_quality_metrics_clear_the_shipped_thresholds() {
    // The measurement procedure below (Step 3) derives config/extractor.toml's
    // [quality] defaults FROM this exact assertion holding on the clean fixture.
    let png = std::fs::read(fixtures_dir().join("skewed_block.png")).unwrap();
    let metrics = preprocess::page_quality(&png, 1568).unwrap();
    assert!(
        metrics.residual_skew_deg <= 1.5,
        "clean fixture residual skew {} must clear max_residual_skew_deg=1.5",
        metrics.residual_skew_deg
    );
    assert!(
        metrics.otsu_variance >= 0.02,
        "clean fixture otsu_variance {} must clear min_otsu_variance=0.02",
        metrics.otsu_variance
    );
    assert!(
        metrics.noise_count <= 1200,
        "clean fixture noise_count {} must clear max_noise_count=1200",
        metrics.noise_count
    );
}

#[test]
fn noise_salted_fixture_trips_the_noise_count_threshold() {
    let png = std::fs::read(fixtures_dir().join("noisy_skewed_block.png")).unwrap();
    let metrics = preprocess::page_quality(&png, 1568).unwrap();
    assert!(
        metrics.noise_count > 1200,
        "noise-salted fixture noise_count {} must exceed max_noise_count=1200 — \
         otherwise config/extractor.toml's [quality] threshold does not separate \
         clean from degraded scans",
        metrics.noise_count
    );
}
```

Also update the two existing Task 8 tests in the same file for the `PreprocessOutput` return
shape (`preprocess_document` no longer returns `Vec<Vec<u8>>`):

```rust
#[test]
fn preprocess_document_end_to_end_when_pdfium_is_present() {
    if pdfium_render::prelude::Pdfium::bind_to_system_library().is_err() {
        eprintln!(
            "SKIP: preprocess_document_end_to_end_when_pdfium_is_present — \
             pdfium is not installed on this host"
        );
        return;
    }
    let pdf = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("adapters")
            .join("us_house")
            .join("fixtures")
            .join("scanned_paper_ptr")
            .join("input.pdf"),
    )
    .unwrap();
    let cfg = preprocess::PreprocessCfg { max_edge: 800 };
    let output = preprocess::preprocess_document(&pdf, &cfg).unwrap();
    assert!(!output.pages_png.is_empty());
    assert_eq!(
        output.pages_png.len(),
        output.quality.len(),
        "one QualityMetrics per page (goal 021 v2 H31)"
    );
    for page_png in &output.pages_png {
        let decoded = image::load_from_memory(page_png).unwrap().to_luma8();
        let longest = decoded.width().max(decoded.height());
        assert!(
            longest <= 800,
            "preprocess_document must honor cfg.max_edge=800, got {longest}"
        );
    }
}
```

(`preprocess_document_propagates_pdfium_unavailable` needs no edit — it asserts on the `Err`
branch, which never constructs a `PreprocessOutput`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pipeline --test preprocess`
Expected: FAIL to compile — `error[E0433]: failed to resolve: could not find 'page_quality' in
'preprocess'` (the new tests), and `error[E0609]: no field 'pages_png' on type 'Vec<Vec<u8>>'`
(the updated Task 8 test), since `preprocess_document` still returns `Vec<Vec<u8>>` and
`page_quality` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

**Measurement procedure** (perform once, before finalizing `config/extractor.toml`'s
`[quality]` values — mirrors Task 18's procedure: compute on committed fixtures, record the
values, pick a threshold strictly between them; pdfium raster output is not byte-stable
across builds — per design D1/§3.7 and finding 16 — so calibration runs on the PURE-IMAGE-
STAGE committed PNGs from Task 6, never on a live pdfium raster):

1. Implement `page_quality`/`QualityMetrics` per Step 3's code below.
2. Run `cargo test -p pipeline --test preprocess -- --nocapture clean_fixture_quality_metrics_clear_the_shipped_thresholds noise_salted_fixture_trips_the_noise_count_threshold` and read the assertion-failure messages (or temporarily add `eprintln!("{metrics:?}")` calls) to record the measured values for `skewed_block.png` (clean) and `noisy_skewed_block.png` (degraded).
3. `residual_skew_deg`: re-running the deskew angle search on the ALREADY-deskewed image measures leftover misalignment. The clean fixture's exact 4° synthetic skew is corrected by `best_deskew_angle_deg`'s 0.25°-step search, so residual measures near 0 (well under 1.5°); a genuinely warped/noisy scan does not fully converge under one global rotation and residual grows.
4. `otsu_variance`: `otsu_threshold_with_variance` normalizes the between-class variance by the maximum possible value (`255²/4`, the case of a population split 50/50 between pure black and pure white) so it is comparable across image sizes. The clean fixture (stark black rectangle on white) measures HIGH separation (near the ceiling); a foggy/low-contrast scan measures LOW — `min_otsu_variance = 0.02` is a low floor that only flags genuinely poor separation, not merely non-maximal contrast.
5. `noise_count`: raw isolated-dark-pixel count (Step 3's `count_isolated_ink_pixels`). The clean fixture measures near 0 (a few anti-aliased boundary pixels from the bilinear rotation, at most low tens); the noise-salted fixture's ~1-in-97 background flip rate over a ~390,000px white background measures in the low thousands. `max_noise_count = 1200` sits strictly between the two, closer to the clean side (the noisy fixture must clear it by a wide margin so the signal is robust to fixture-generation changes).
6. Record: `config/extractor.toml`'s `[quality]` table ships `max_residual_skew_deg = 1.5`,
   `min_otsu_variance = 0.02`, `max_noise_count = 1200` — these are the shipped defaults this
   procedure derives (Task 9 already carries this table; this task adds the Rust side that
   parses and acts on it).

Replace `otsu_threshold` in `crates/pipeline/src/extraction/preprocess.rs` (Task 6) with the
variance-exposing split (keep every other Task 6 function — `ink_density`, `best_deskew_angle_deg`,
`row_projection_variance`, `rotate_luma`, `binarize`, `margin_crop`, `resize_longest_edge`,
`encode_png` — byte-for-byte unchanged):

```rust
/// Maximum possible Otsu between-class variance: the population split
/// exactly 50/50 between pure black (0) and pure white (255).
const MAX_OTSU_BETWEEN_CLASS_VARIANCE: f64 = 255.0 * 255.0 / 4.0;

/// Otsu's method AND the normalized (0.0..=1.0-ish) between-class variance
/// at the chosen threshold — the separation quality the search maximized,
/// a free by-product (goal 021 v2 H31, finding 16). `otsu_threshold` keeps
/// its exact previous call sites by discarding the second element.
fn otsu_threshold_with_variance(img: &GrayImage) -> (u8, f32) {
    let mut histogram = [0u32; 256];
    for pixel in img.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }
    let total = f64::from(img.width()) * f64::from(img.height());
    if total == 0.0 {
        return (DARK_THRESHOLD, 0.0);
    }
    let sum_all: f64 = histogram
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * f64::from(c))
        .sum();
    let mut sum_bg = 0.0f64;
    let mut weight_bg = 0.0f64;
    let mut best_thresh = 0u8;
    let mut best_variance = 0.0f64;
    for (t, &count) in histogram.iter().enumerate() {
        weight_bg += f64::from(count);
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg <= 0.0 {
            break;
        }
        sum_bg += t as f64 * f64::from(count);
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_all - sum_bg) / weight_fg;
        let variance = weight_bg * weight_fg * (mean_bg - mean_fg).powi(2);
        if variance > best_variance {
            best_variance = variance;
            best_thresh = t as u8;
        }
    }
    let normalized = (best_variance / (total * total) / MAX_OTSU_BETWEEN_CLASS_VARIANCE) as f32;
    (best_thresh, normalized)
}

fn otsu_threshold(img: &GrayImage) -> u8 {
    otsu_threshold_with_variance(img).0
}
```

Add `QualityMetrics`/`PreprocessOutput` and the quality-computing pipeline:

```rust
/// Free preprocess by-products (design amendment-1 A16, finding 16):
/// residual skew after deskewing, Otsu between-class variance (separation
/// quality), and isolated-ink-pixel ("noise speck") count. One per page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityMetrics {
    pub residual_skew_deg: f32,
    pub otsu_variance: f32,
    pub noise_count: u32,
}

/// [`preprocess_document`]'s return shape (goal 021 v2 H31 — evolves Task
/// 8's plain `Vec<Vec<u8>>`; `ConsensusExtractor::extract` (Task 13/17)
/// threads `.quality` into the premium-trigger disjunction).
#[derive(Debug, Clone)]
pub struct PreprocessOutput {
    pub pages_png: Vec<Vec<u8>>,
    pub quality: Vec<QualityMetrics>,
}

/// Counts dark (post-binarization) pixels with NO dark 4-neighbor — a
/// cheap, single-pass proxy for scan-artifact salt noise: a genuine printed
/// stroke or rule is never a single isolated pixel, but noise specks
/// usually are.
fn count_isolated_ink_pixels(binary: &GrayImage) -> u32 {
    let (width, height) = binary.dimensions();
    let is_dark = |x: i64, y: i64| -> bool {
        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
            return false;
        }
        binary.get_pixel(x as u32, y as u32).0[0] == 0
    };
    let mut count = 0u32;
    for y in 0..height {
        for x in 0..width {
            if !is_dark(i64::from(x), i64::from(y)) {
                continue;
            }
            let neighbor_dark = is_dark(i64::from(x) - 1, i64::from(y))
                || is_dark(i64::from(x) + 1, i64::from(y))
                || is_dark(i64::from(x), i64::from(y) - 1)
                || is_dark(i64::from(x), i64::from(y) + 1);
            if !neighbor_dark {
                count += 1;
            }
        }
    }
    count
}

/// The shared engine behind [`preprocess_page`] (Task 6, public signature
/// UNCHANGED) and [`preprocess_document`] (this task, return-shape
/// evolved): runs the identical grayscale -> deskew -> Otsu -> binarize ->
/// crop -> resize pipeline once, measuring [`QualityMetrics`] as a
/// byproduct instead of discarding the intermediate images.
fn preprocess_page_with_quality(png: &[u8], max_edge: u32) -> anyhow::Result<(Vec<u8>, QualityMetrics)> {
    let decoded = image::load_from_memory(png).context("decoding page PNG")?;
    let gray = decoded.to_luma8();
    let angle = best_deskew_angle_deg(&gray);
    let deskewed = rotate_luma(&gray, angle);
    let (threshold, otsu_variance) = otsu_threshold_with_variance(&deskewed);
    let binarized = binarize(&deskewed, threshold);
    let residual_skew_deg = best_deskew_angle_deg(&deskewed).abs();
    let noise_count = count_isolated_ink_pixels(&binarized);
    let cropped = margin_crop(&binarized, MARGIN_PADDING_PX);
    let resized = resize_longest_edge(&cropped, max_edge);
    let bytes = encode_png(&resized)?;
    Ok((bytes, QualityMetrics { residual_skew_deg, otsu_variance, noise_count }))
}

pub fn preprocess_page(png: &[u8], max_edge: u32) -> anyhow::Result<Vec<u8>> {
    Ok(preprocess_page_with_quality(png, max_edge)?.0)
}

/// Single-page quality measurement without the PNG re-encode cost — used
/// by this task's fixture-calibration tests and any caller that only needs
/// the metrics, not the preprocessed bytes.
///
/// # Errors
/// The input is not a decodable PNG.
pub fn page_quality(png: &[u8], max_edge: u32) -> anyhow::Result<QualityMetrics> {
    Ok(preprocess_page_with_quality(png, max_edge)?.1)
}

pub fn preprocess_document(pdf: &[u8], cfg: &PreprocessCfg) -> anyhow::Result<PreprocessOutput> {
    let raw_pages = rasterize(pdf, cfg.max_edge)?;
    let mut pages_png = Vec::with_capacity(raw_pages.len());
    let mut quality = Vec::with_capacity(raw_pages.len());
    for (index, png) in raw_pages.iter().enumerate() {
        let (bytes, metrics) = preprocess_page_with_quality(png, cfg.max_edge)
            .with_context(|| format!("preprocessing rasterized page {index}"))?;
        pages_png.push(bytes);
        quality.push(metrics);
    }
    Ok(PreprocessOutput { pages_png, quality })
}
```

`ExtractorConfig.quality: QualityConfig`, `VersionsConfig.quality: String`, and
`composite_model_id`'s `+{quality}` folding are ALL already present on Task 9's landed
`config.rs` (AMENDED, goal 021 Phase 3 — the full 12-field `ExtractorConfig` shape, including
`escalation`/`families`/`audit`/`drift`/`cross_lab`, which this task must NOT drop by
re-declaring the struct). This task EDITS Task 9's already-landed `QualityConfig` in place —
replacing its derived `Default` (which would silently ship all-zero thresholds) with the
calibrated one this task's measurement procedure derives — and ADDS `doc_quality_flagged`. In
`crates/pipeline/src/extraction/config.rs`, above the existing `#[cfg(test)] mod tests`:

```rust
use crate::extraction::preprocess::QualityMetrics;

// EDIT: Task 9's landed `QualityConfig` derives `#[derive(Debug, Clone, Default,
// Deserialize)]`, which silently ships an all-zero-thresholds `Default`. Change
// the derive to drop `Default` (add `Copy, PartialEq`) and add the calibrated
// `impl Default` below. Field names/types are UNCHANGED from Task 9 — derive +
// impl edit only, do not touch the field list or any other struct in this file.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct QualityConfig {
    pub max_residual_skew_deg: f32,
    pub min_otsu_variance: f32,
    pub max_noise_count: u32,
}

/// Preprocess-quality routing thresholds (amendment-1 A16, goal 021 v2 H31,
/// finding 16). Values calibrated on the committed Task 6 PNGs (this task's
/// measurement procedure — pdfium output is not byte-stable across builds,
/// design D1).
impl Default for QualityConfig {
    fn default() -> Self {
        Self { max_residual_skew_deg: 1.5, min_otsu_variance: 0.02, max_noise_count: 1200 }
    }
}

/// True when ANY page's measured quality crosses a configured threshold —
/// `ConsensusExtractor::extract` ORs this into the premium-trigger
/// disjunction (H32) so a degraded scan gets the premium pass even on
/// unanimous sample agreement (design amendment-1 A16).
#[must_use]
pub fn doc_quality_flagged(cfg: &ExtractorConfig, quality: &[QualityMetrics]) -> bool {
    quality.iter().any(|q| {
        q.residual_skew_deg > cfg.quality.max_residual_skew_deg
            || q.otsu_variance < cfg.quality.min_otsu_variance
            || q.noise_count > cfg.quality.max_noise_count
    })
}
```

Add a test to `config.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn quality_config_parses_and_folds_the_version_into_the_composite_tag() {
        let cfg = ExtractorConfig::load_from(&config_path(), |_| None).unwrap();
        assert!((cfg.quality.max_residual_skew_deg - 1.5).abs() < f32::EPSILON);
        assert!((cfg.quality.min_otsu_variance - 0.02).abs() < f32::EPSILON);
        assert_eq!(cfg.quality.max_noise_count, 1200);
        assert_eq!(cfg.versions.quality, "q1");
        assert!(
            composite_model_id(&cfg).ends_with("+q1"),
            "composite id must end with the folded quality version: {}",
            composite_model_id(&cfg)
        );
    }
```

In `crates/pipeline/src/extraction/consensus.rs`, change `ConsensusExtractor`'s test seam and
`images()` to the new `PreprocessOutput` return shape:

```rust
pub struct ConsensusExtractor<'a, T: Transport> {
    transport: &'a T,
    cfg: &'a ExtractorConfig,
    #[cfg(test)]
    fixed_output: Option<preprocess::PreprocessOutput>,
}

impl<'a, T: Transport> ConsensusExtractor<'a, T> {
    #[must_use]
    pub fn new(transport: &'a T, cfg: &'a ExtractorConfig) -> Self {
        Self {
            transport,
            cfg,
            #[cfg(test)]
            fixed_output: None,
        }
    }

    /// Test-only seam: skips `preprocess_document` (no pdfium needed in CI)
    /// and feeds pre-built page images straight to `run_samples`, with a
    /// clean (never-flagged) `QualityMetrics` per page. Existing Task 13/17
    /// call sites (`with_fixed_images(&transport, &cfg, vec![..])`) are
    /// UNCHANGED — this is now implemented in terms of the new
    /// quality-aware constructor below.
    #[cfg(test)]
    #[must_use]
    pub fn with_fixed_images(transport: &'a T, cfg: &'a ExtractorConfig, images: Vec<Vec<u8>>) -> Self {
        let quality = vec![
            preprocess::QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 };
            images.len()
        ];
        Self::with_fixed_images_and_quality(transport, cfg, images, quality)
    }

    /// Test-only seam (goal 021 v2 H31): like `with_fixed_images`, but lets
    /// a test inject specific per-page `QualityMetrics` — real quality
    /// computation requires a genuinely decodable PNG, which the fixed
    /// fake-byte images used across Tasks 13/17's tests are not.
    #[cfg(test)]
    #[must_use]
    pub fn with_fixed_images_and_quality(
        transport: &'a T,
        cfg: &'a ExtractorConfig,
        images: Vec<Vec<u8>>,
        quality: Vec<preprocess::QualityMetrics>,
    ) -> Self {
        Self {
            transport,
            cfg,
            fixed_output: Some(preprocess::PreprocessOutput { pages_png: images, quality }),
        }
    }

    #[cfg(not(test))]
    fn images(&self, pdf_bytes: &[u8]) -> anyhow::Result<preprocess::PreprocessOutput> {
        let pcfg = preprocess::PreprocessCfg { max_edge: self.cfg.preprocess.max_edge };
        preprocess::preprocess_document(pdf_bytes, &pcfg)
    }

    #[cfg(test)]
    fn images(&self, pdf_bytes: &[u8]) -> anyhow::Result<preprocess::PreprocessOutput> {
        if let Some(output) = &self.fixed_output {
            return Ok(output.clone());
        }
        let pcfg = preprocess::PreprocessCfg { max_edge: self.cfg.preprocess.max_edge };
        preprocess::preprocess_document(pdf_bytes, &pcfg)
    }
```

Evolve `extract()`'s escalation trigger (keep the `images`/`run_samples`/`align`/`score`
prelude's SHAPE, only its first line and the trigger condition change) and evolve `route`'s
`Agreed` arm:

```rust
    pub async fn extract(
        &self,
        pdf_bytes: &[u8],
        spec: &ConsensusSpec,
        sanity: SanityCheck<'_>,
    ) -> anyhow::Result<DocOutcome> {
        let preprocessed = self.images(pdf_bytes)?;
        let samples = run_samples(
            self.transport,
            &self.cfg.models.primary,
            &preprocessed.pages_png,
            spec,
            self.cfg,
        )
        .await?;
        let payloads: Vec<serde_json::Value> = samples.iter().map(|s| s.payload.clone()).collect();
        let aligned = align(&payloads, spec)?;
        let verdicts = score(&aligned, spec);
        let agreement = summarize_agreement(&verdicts, &aligned, spec);
        let header = vote_header(&payloads, spec)?;

        // ONE escalation pass per document (design D8; amendment-1 A16
        // extends the trigger to quality-flagged docs even when unanimous —
        // H32 extends this disjunction further with pixel-ambiguity and the
        // high-impact floor; the invariant this task establishes is that
        // there is still exactly ONE `Option<SamplePass>` slot and ONE send
        // site regardless of how many conditions can set it).
        let has_dispute = verdicts.iter().any(|v| matches!(v, RowVerdict::Disputed { .. }));
        let quality_flagged =
            crate::extraction::config::doc_quality_flagged(self.cfg, &preprocessed.quality);
        let premium_needed = has_dispute || quality_flagged;
        let escalation: Option<SamplePass> = if premium_needed {
            let sampling = SamplingParams { temperature: None, ..Default::default() };
            let request = build_image_request(
                &self.cfg.models.escalation,
                &preprocessed.pages_png,
                &spec.tool,
                &sampling,
            );
            let response = self
                .transport
                .send(&request)
                .await
                .context("consensus escalation call")?;
            let payload = extract_tool_payload(&response, &spec.tool.tool_name)
                .context("consensus escalation tool output")?;
            let usage = response.get("usage").cloned().unwrap_or(serde_json::Value::Null);
            Some(SamplePass { model_id: self.cfg.models.escalation.clone(), payload, usage })
        } else {
            None
        };

        let mut outcome = route(verdicts, spec, sanity, escalation.as_ref());
        anyhow::ensure!(
            !outcome.published.is_empty(),
            "needs_llm_extraction: consensus produced zero publishable rows for this document \
             ({} held, {} sample rows) — freeze + review_task (invariant 6; us_house is not in \
             zero_rows::allowed)",
            outcome.held.len(),
            payloads.len()
        );

        outcome.stats = build_stats(&samples, self.cfg, agreement, escalation.as_ref().map(|p| &p.usage));
        outcome.header = header;
        outcome.samples = samples;
        Ok(outcome)
    }
```

Evolve `route`'s `Agreed` arm (chain 3 → 4 → 17 → **H31**; the `Disputed` arm is untouched —
Task 17's `resolve_disputed`/`premium_row_at`/`field_resolution` stay exactly as landed):

```rust
pub fn route(
    verdicts: Vec<RowVerdict>,
    spec: &ConsensusSpec,
    sanity: SanityCheck<'_>,
    escalation: Option<&SamplePass>,
) -> DocOutcome {
    let mut outcome = DocOutcome::default();
    for verdict in verdicts {
        match verdict {
            RowVerdict::Agreed { ordinal0, row } => {
                let sanity_violations = sanity(&row);
                let dissents = escalation.is_some_and(|pass| premium_dissents(&row, pass, spec));
                let confidence = if !sanity_violations.is_empty() || dissents {
                    policy::CONF_SANITY_CAPPED
                } else {
                    policy::CONF_AGREED
                };
                outcome.published.push(PublishedRow { ordinal0, row, confidence });
            }
            RowVerdict::Disputed { ordinal0, key, candidates, disputed_fields } => {
                let resolution = escalation.and_then(|pass| {
                    resolve_disputed(ordinal0, &key, &candidates, &disputed_fields, spec, &pass.payload)
                });
                match resolution {
                    Some(resolved) => {
                        let confidence = if sanity(&resolved.row).is_empty() {
                            resolved.confidence
                        } else {
                            policy::CONF_SANITY_CAPPED
                        };
                        outcome.published.push(PublishedRow {
                            ordinal0: resolved.ordinal0,
                            row: resolved.row,
                            confidence,
                        });
                    }
                    None => outcome.held.push(HeldRow { ordinal0, competing: candidates }),
                }
            }
        }
    }
    outcome
}

/// True when the escalation pass's matching row disagrees, on the
/// canonical plane (H28), with `row` on any of `spec.critical_fields`. This
/// is the GENERAL premium-concordance check (goal 021 v2 H31, finding 16;
/// H32 reuses it verbatim for the pixel-ambiguity and high-impact-floor
/// triggers — WHY escalation fired never changes what dissent means).
/// Occurrence 0 only, same documented limitation as `premium_row_at`'s
/// existing callers (Task 17): no fixture in this task exercises duplicate
/// content keys within one premium payload.
fn premium_dissents(row: &Value, pass: &SamplePass, spec: &ConsensusSpec) -> bool {
    let key = row_key(row, &spec.key_fields, 0);
    let Some(premium_row) = premium_row_at(&pass.payload, &key, spec) else {
        return false;
    };
    spec.critical_fields.iter().any(|field| {
        let ours = canonical_field(field, row.pointer(field).unwrap_or(&Value::Null));
        let theirs = canonical_field(field, premium_row.pointer(field).unwrap_or(&Value::Null));
        ours != theirs
    })
}
```

(`RowVerdict::Disputed`'s `key` field and `resolve_disputed`'s `key: &RowKey` parameter are
H28/H29's evolution of Task 3/17, already landed at this execution frontier — this task's
`Disputed` arm shown above matches that shape verbatim, changed only by whitespace from
Task 17's version; do not re-author it.)

Finally, add `quality: QualityConfig::default()` and `quality: "q1".to_owned()` to every
existing struct literal these new required fields break: `crates/pipeline/tests/consensus_extraction.rs`'s
`test_cfg()` (its `ExtractorConfig { .. }` literal gains `quality: QualityConfig::default(),`;
its `versions: VersionsConfig { .. }` literal gains `quality: "q1".to_owned(),`), and any
`ConsensusSpec`/`ExtractorConfig` literal inside `crates/pipeline/src/extraction/consensus.rs`'s
own `#[cfg(test)]` modules that construct a full `ExtractorConfig` (not via `load_from`) —
`grep -n "ExtractorConfig {" crates/pipeline/src/extraction/consensus.rs
crates/pipeline/tests/*.rs` on your branch and add the one field to each hit; call sites built
via `ExtractorConfig::load_from(...)` need no edit (the new fields are `#[serde(default)]`-safe).

Add to `crates/pipeline/tests/consensus_extraction.rs` (reusing `spec()`, `row()`,
`tool_response()`, `test_cfg()`, `MockTransport`, `no_sanity_issues`):

```rust
use pipeline::extraction::preprocess::QualityMetrics;

fn flagged_quality() -> Vec<QualityMetrics> {
    // Trips max_noise_count (default 1200) — same axis
    // `noise_salted_fixture_trips_the_noise_count_threshold` (preprocess.rs)
    // calibrates against the committed fixture.
    vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 5000 }]
}

#[tokio::test]
async fn quality_flagged_doc_fires_the_premium_pass_even_with_full_agreement() {
    let rows = json!([row("2026-06-01", "A")]);
    let premium = json!([row("2026-06-01", "A")]); // concurs
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
        tool_response(premium),
    ]);
    let cfg = test_cfg();
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![b"fake-png".to_vec()],
        flagged_quality(),
    );
    let outcome = extractor
        .extract(b"%PDF-fake", &spec(), &no_sanity_issues)
        .await
        .unwrap();

    assert_eq!(
        transport.requests().len(),
        4,
        "3 samples + exactly 1 quality-triggered premium call, unanimous rows"
    );
    assert_eq!(outcome.published.len(), 1);
    assert_eq!(outcome.published[0].confidence, 0.9f32, "premium concurs -> 0.90 stands");
}

#[tokio::test]
async fn quality_flagged_doc_caps_to_0_79_on_premium_dissent() {
    let rows = json!([row("2026-06-01", "A")]);
    let premium = json!([row("2026-06-01", "Z")]); // dissents on the critical amount_band field
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
        tool_response(premium),
    ]);
    let cfg = test_cfg();
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![b"fake-png".to_vec()],
        flagged_quality(),
    );
    let outcome = extractor
        .extract(b"%PDF-fake", &spec(), &no_sanity_issues)
        .await
        .unwrap();

    assert_eq!(transport.requests().len(), 4, "still exactly 1 premium call");
    assert_eq!(
        outcome.published.len(),
        1,
        "dissent CAPS an Agreed row — it never rewrites the value or holds it"
    );
    assert_eq!(outcome.published[0].confidence, 0.79f32, "premium dissent on a critical field");
    assert_eq!(
        outcome.published[0].row,
        row("2026-06-01", "A"),
        "published value stays the model's own verbatim string (invariant 2)"
    );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --test preprocess`
Expected: PASS — `clean_fixture_quality_metrics_clear_the_shipped_thresholds`,
`noise_salted_fixture_trips_the_noise_count_threshold`,
`preprocess_document_end_to_end_when_pdfium_is_present` (self-skips without a local pdfium
install), plus every pre-existing Task 6/8 test unchanged.

Run: `cargo test -p pipeline --lib extraction::config::`
Expected: PASS — `quality_config_parses_and_folds_the_version_into_the_composite_tag` plus
Task 9's three unrelated tests (`parses_the_committed_config_file`,
`env_overrides_win_over_the_file`, `absent_budget_names_the_missing_key`).

Run: `cargo test -p pipeline --test consensus_extraction`
Expected: PASS — the two new quality-trigger tests plus every pre-existing Task 13/17 test
(`with_fixed_images`'s implicit all-clean quality vector keeps them at their original
behavior).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/pipeline/src/extraction/preprocess.rs \
        crates/pipeline/tests/preprocess.rs \
        crates/pipeline/src/extraction/config.rs \
        crates/pipeline/src/extraction/consensus.rs \
        crates/pipeline/tests/consensus_extraction.rs
git commit -m "$(cat <<'EOF'
feat(pipeline): QualityMetrics + quality-routed premium trigger (goal 021 v2 H31)

Measures residual skew, Otsu between-class variance, and isolated-ink-pixel
noise as free preprocess by-products, calibrated on the committed Task 6
fixtures (design amendment-1 A16, finding 16). A flagged document fires the
single shared premium pass even when every sample agrees, and 0.90 now
additionally requires premium concordance on flagged docs — the general
Agreed-row concordance mechanism H32 extends to the pixel and high-impact
triggers.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H32: Single-slot premium trigger disjunction + pixel-ambiguity signal + high-impact floor (findings 15, 16-trigger, §6.3 fix)

**This task deliberately exceeds the ~1h guideline** — the single-slot premium trigger
disjunction unifies three findings (pixel-ambiguity, quality, high-impact) behind ONE escalation
slot; splitting would leave the slot responding to only some of its required triggers between
commits. Do not split it.

Generalizes H31's "escalation fires, then concordance caps 0.79 on dissent" mechanism to two
more triggers — pixel-ambiguous checkbox reads (finding 15) and the §6.3 high-impact floor —
while preserving the one-premium-call-per-document invariant the Phase-2 controller resolution
(`.superpowers/sdd/progress.md`) already settled: *"high-impact rows (band >= 500001.00 /
watchlist) get the retained v1 second-model cross-check EVEN when consensus is unanimous
(design 3.6 bias decorrelation — unanimity does not decorrelate same-model bias). Mismatch ->
cap to mandatory review (0.79) or hold; never silent. The plan's `_high_impact_document` marker
in Task 24 is to be implemented as this action, not left as a marker."* This task is that
implementation — for the consensus path (Task 24's own v1-cross-check-parity wiring is a
separate, already-scheduled task; this task only fixes the consensus-side discard).

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` (`PixelSignal`/`PixelVerdict`;
  `ConsensusSpec` gains `high_impact_values`/`watchlist_pointer`; `extract()` gains the `pixel`
  parameter and computes the 4-way `premium_needed` disjunction ONCE; `route` gains a
  `pixel_verdicts` parameter; `high_impact_floor` helper)
- Modify: `crates/pipeline/src/extraction/mod.rs` (re-export `PixelSignal`, `PixelVerdict`)
- Modify: `crates/adapters/us_house/src/consensus.rs` (Task 18's `pixel_ambiguity` gray-zone
  detector over the checkbox geometry; `consensus_spec()` populates `high_impact_values`)
- Modify: `crates/pipeline/tests/consensus_extraction.rs` (the 2^4 property test; a
  pixel-triggered concurrence-stands test)

**Interfaces:**
- Consumes: H31's `premium_dissents`, `doc_quality_flagged`, `PreprocessOutput` (same module /
  crate); Task 18's `FormGeometry`, `RowBandGeometry`, `INK_THRESHOLD`,
  `pipeline::extraction::preprocess::{ink_density, NormRect}` (`crates/adapters/us_house/src/consensus.rs`);
  `crate::extraction::WATCHLIST_POLITICIANS: &[&str]` (`crates/pipeline/src/extraction/mod.rs:40`,
  already defined, currently an empty stub).
- Produces: `pub type PixelSignal<'a> = &'a (dyn Fn(&serde_json::Value) -> PixelVerdict + Send +
  Sync)`; `pub enum PixelVerdict { Clear, Ambiguous, Conflict(Vec<String>) }` (`Debug, Clone,
  PartialEq`); `ConsensusSpec` gains `pub high_impact_values: Vec<(String, Vec<String>)>` (row-
  relative JSON pointer -> the set of string values that mark a row high-impact — e.g. us_house's
  `("/band_column", vec!["F","G","H","I","J"])`, letter-aware per amendment-1 A11's `band_column`
  enum) and `pub watchlist_pointer: Option<String>` (header-relative pointer checked against
  `WATCHLIST_POLITICIANS`) — both plain data, so `ConsensusSpec` keeps its existing `#[derive(Debug,
  Clone)]`; evolves `extract()`'s arity to `extract(&self, pdf_bytes: &[u8], spec: &ConsensusSpec,
  sanity: SanityCheck<'_>, pixel: PixelSignal<'_>) -> anyhow::Result<DocOutcome>` (Task 24's call
  site is updated by the changeset workstream — reference, do not duplicate); evolves `route`
  (chain 3 → 4 → 17 → H31 → **H32**) to `route(verdicts, spec, sanity, pixel_verdicts: &[PixelVerdict],
  escalation) -> DocOutcome`, index-aligned with `verdicts` (amendment-1 A11: "pixel check becomes
  index-to-index exact"); `pub fn pixel_ambiguity<'a>(page_images: &'a [image::GrayImage], geometry:
  &'a FormGeometry) -> impl Fn(&serde_json::Value) -> PixelVerdict + Send + Sync + 'a`
  (`crates/adapters/us_house/src/consensus.rs` — the gray-zone detector, same Cell-based
  one-call-per-row contract as `checkbox_sanity`); `pub const GRAY_BAND: f32` (us_house).

- [ ] **Step 1: Write the failing test**

Add to `crates/pipeline/tests/consensus_extraction.rs` (reusing `spec()`, `row()`,
`tool_response()`, `test_cfg()`, `MockTransport`, `no_sanity_issues`, H31's `QualityMetrics`):

```rust
use pipeline::extraction::consensus::PixelVerdict;

fn clean_quality() -> Vec<QualityMetrics> {
    vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }]
}

fn no_pixel_ambiguity(_row: &Value) -> PixelVerdict {
    PixelVerdict::Clear
}

#[tokio::test]
async fn pixel_ambiguous_unanimous_row_with_premium_concurrence_keeps_0_90() {
    let rows = json!([row("2026-06-01", "A")]);
    let premium = json!([row("2026-06-01", "A")]); // concurs
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
        tool_response(premium),
    ]);
    let cfg = test_cfg();
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![b"fake-png".to_vec()],
        clean_quality(),
    );
    let always_ambiguous = |_row: &Value| PixelVerdict::Ambiguous;
    let outcome = extractor
        .extract(b"%PDF-fake", &spec(), &no_sanity_issues, &always_ambiguous)
        .await
        .unwrap();

    assert_eq!(transport.requests().len(), 4, "pixel ambiguity alone fires the ONE premium call");
    assert_eq!(outcome.published[0].confidence, 0.9f32, "premium concurs -> 0.90 stands");
}

#[tokio::test]
async fn all_sixteen_premium_trigger_combinations_send_exactly_n_or_n_plus_one_never_n_plus_two() {
    for bits in 0u8..16 {
        let dispute = bits & 0b0001 != 0;
        let quality = bits & 0b0010 != 0;
        let pixel_amb = bits & 0b0100 != 0;
        let high_impact = bits & 0b1000 != 0;

        let band_agree = if high_impact { "F" } else { "A" };
        let sample_a = json!([row("2026-06-01", band_agree)]);
        let sample_b = if dispute { json!([row("2026-06-01", "Z")]) } else { sample_a.clone() };
        let sample_c = sample_a.clone();
        let mut responses =
            vec![tool_response(sample_a), tool_response(sample_b), tool_response(sample_c)];
        let premium_needed = dispute || quality || pixel_amb || high_impact;
        if premium_needed {
            responses.push(tool_response(json!([row("2026-06-01", band_agree)])));
        }
        let transport = MockTransport::returning(responses);
        let cfg = test_cfg();
        let mut sweep_spec = spec();
        sweep_spec.high_impact_values =
            vec![("/amount_band".to_owned(), vec!["F".to_owned()])];
        let quality_metrics = if quality {
            vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 5000 }]
        } else {
            vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }]
        };
        let extractor = ConsensusExtractor::with_fixed_images_and_quality(
            &transport,
            &cfg,
            vec![b"fake-png".to_vec()],
            quality_metrics,
        );
        let pixel_signal = move |_row: &Value| {
            if pixel_amb { PixelVerdict::Ambiguous } else { PixelVerdict::Clear }
        };
        let outcome = extractor
            .extract(b"%PDF-fake", &sweep_spec, &no_sanity_issues, &pixel_signal)
            .await
            .unwrap();

        let sent = transport.requests().len();
        let expected = if premium_needed { 4 } else { 3 };
        assert_eq!(
            sent, expected,
            "bits={bits:04b} dispute={dispute} quality={quality} pixel_amb={pixel_amb} \
             high_impact={high_impact}: expected {expected} requests, got {sent}"
        );
        assert!(sent <= 4, "NEVER n+2 — exactly one premium slot, one send site");
        drop(outcome);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pipeline --test consensus_extraction`
Expected: FAIL to compile — `error[E0433]: failed to resolve: could not find 'PixelVerdict' in
'consensus'`, and `extract()` called with 4 arguments where H31's signature takes 3
(`error[E0061]: this method takes 3 arguments but 4 arguments were supplied`); `ConsensusSpec`
has no field `high_impact_values` (`error[E0560]`).

- [ ] **Step 3: Write minimal implementation**

In `crates/pipeline/src/extraction/consensus.rs`, add the pixel seam and extend `ConsensusSpec`
(add the two new fields to the existing struct; every existing `ConsensusSpec { .. }` literal
across this file's `#[cfg(test)]` modules and `crates/pipeline/tests/consensus_extraction.rs`'s
`spec()` gains `high_impact_values: Vec::new(), watchlist_pointer: None,` — `grep -n
"ConsensusSpec {" crates/pipeline/src/extraction/consensus.rs crates/pipeline/tests/*.rs
crates/adapters/us_house/src/consensus.rs` on your branch and add the two fields to each hit):

```rust
/// Per-row pixel evidence seam (adapter -> pipeline, goal 021 v2 H32,
/// finding 15): same one-call-per-row-in-document-order contract as
/// `SanityCheck`. `Clear`/`Ambiguous` feed `extract()`'s premium-trigger
/// disjunction; `Conflict` folds into `route`'s existing cap lane exactly
/// like `checkbox_sanity`'s violation strings (a superset, not a
/// replacement — Task 24 wires both closures).
pub type PixelSignal<'a> = &'a (dyn Fn(&Value) -> PixelVerdict + Send + Sync);

/// Pixel-check outcome for one row (amendment-1 A15). Pixel signal selects
/// SCRUTINY, never a value — `Ambiguous` only ever widens the vote set
/// (fires the premium pass); `Conflict` only ever caps confidence; neither
/// rewrites `row`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelVerdict {
    Clear,
    Ambiguous,
    Conflict(Vec<String>),
}
```

Add the two plain-data fields to `ConsensusSpec` (Task 1's struct — keep `tool`, `rows_pointer`,
`key_fields`, `critical_fields` exactly as they are):

```rust
#[derive(Debug, Clone)]
pub struct ConsensusSpec {
    pub tool: DocumentToolSpec,
    pub rows_pointer: String,
    pub key_fields: Vec<String>,
    pub critical_fields: Vec<String>,
    /// §6.3 high-impact floor (goal 021 v2 H32, Phase-2 controller
    /// resolution, `.superpowers/sdd/progress.md`): row-relative pointer ->
    /// the closed set of values marking a row high-impact regardless of
    /// consensus agreement (e.g. us_house's top amount-band letters).
    /// Empty = no high-impact floor for this adapter.
    pub high_impact_values: Vec<(String, Vec<String>)>,
    /// Header-relative pointer for the `WATCHLIST_POLITICIANS` check.
    /// `None` = no watchlist check for this adapter.
    pub watchlist_pointer: Option<String>,
}
```

Add `high_impact_floor` (checked against the SAMPLE payloads and voted header — before
escalation, since it must feed the trigger decision):

```rust
/// True when any row or the header trips `spec`'s high-impact floor
/// (§6.3: top amount bands or a watchlist filer) — the Phase-2 controller
/// resolution's fix for the committed plan's discarded
/// `_high_impact_document` marker: high-impact documents get the premium
/// pass even on unanimous agreement, because same-model unanimity does not
/// decorrelate same-model bias (design §3.6).
fn high_impact_floor(payloads: &[Value], header: &Value, spec: &ConsensusSpec) -> bool {
    let band_hit = spec.high_impact_values.iter().any(|(pointer, values)| {
        payloads.iter().any(|payload| {
            payload
                .pointer(&spec.rows_pointer)
                .and_then(Value::as_array)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row.pointer(pointer)
                            .and_then(Value::as_str)
                            .is_some_and(|v| values.iter().any(|hv| hv == v))
                    })
                })
        })
    });
    let watchlist_hit = spec.watchlist_pointer.as_deref().is_some_and(|pointer| {
        header
            .pointer(pointer)
            .and_then(Value::as_str)
            .is_some_and(|name| crate::extraction::WATCHLIST_POLITICIANS.contains(&name))
    });
    band_hit || watchlist_hit
}
```

Evolve `extract()` (H31's version — only the trigger computation and the `pixel` parameter
change; `images`/`run_samples`/`align`/`score`/`escalation`-building/`build_stats` prelude keep
H31's shape):

```rust
    pub async fn extract(
        &self,
        pdf_bytes: &[u8],
        spec: &ConsensusSpec,
        sanity: SanityCheck<'_>,
        pixel: PixelSignal<'_>,
    ) -> anyhow::Result<DocOutcome> {
        let preprocessed = self.images(pdf_bytes)?;
        let samples = run_samples(
            self.transport,
            &self.cfg.models.primary,
            &preprocessed.pages_png,
            spec,
            self.cfg,
        )
        .await?;
        let payloads: Vec<serde_json::Value> = samples.iter().map(|s| s.payload.clone()).collect();
        let aligned = align(&payloads, spec)?;
        let verdicts = score(&aligned, spec);
        let agreement = summarize_agreement(&verdicts, &aligned, spec);
        let header = vote_header(&payloads, spec)?;

        // Pixel evidence evaluated ONCE per row, in document order (same
        // precondition as `checkbox_sanity`'s Cell-based counter) — reused
        // for BOTH the premium-trigger decision below and `route`'s per-row
        // capping, so the closure is never invoked twice for one row.
        let pixel_verdicts: Vec<PixelVerdict> = verdicts
            .iter()
            .map(|verdict| {
                let representative = match verdict {
                    RowVerdict::Agreed { row, .. } => row,
                    RowVerdict::Disputed { candidates, .. } => &candidates[0],
                };
                pixel(representative)
            })
            .collect();
        let pixel_ambiguous_any_row =
            pixel_verdicts.iter().any(|v| matches!(v, PixelVerdict::Ambiguous));

        // Exactly ONE premium slot, ONE send site (design D8, amendment-1
        // §2's one-premium-call invariant): every trigger ORs into the same
        // boolean, computed once, before any transport call.
        let has_dispute = verdicts.iter().any(|v| matches!(v, RowVerdict::Disputed { .. }));
        let quality_flagged =
            crate::extraction::config::doc_quality_flagged(self.cfg, &preprocessed.quality);
        let high_impact = high_impact_floor(&payloads, &header, spec);
        let premium_needed = quality_flagged || pixel_ambiguous_any_row || has_dispute || high_impact;

        let escalation: Option<SamplePass> = if premium_needed {
            let sampling = SamplingParams { temperature: None, ..Default::default() };
            let request = build_image_request(
                &self.cfg.models.escalation,
                &preprocessed.pages_png,
                &spec.tool,
                &sampling,
            );
            let response = self
                .transport
                .send(&request)
                .await
                .context("consensus escalation call")?;
            let payload = extract_tool_payload(&response, &spec.tool.tool_name)
                .context("consensus escalation tool output")?;
            let usage = response.get("usage").cloned().unwrap_or(serde_json::Value::Null);
            Some(SamplePass { model_id: self.cfg.models.escalation.clone(), payload, usage })
        } else {
            None
        };

        let mut outcome = route(verdicts, spec, sanity, &pixel_verdicts, escalation.as_ref());
        anyhow::ensure!(
            !outcome.published.is_empty(),
            "needs_llm_extraction: consensus produced zero publishable rows for this document \
             ({} held, {} sample rows) — freeze + review_task (invariant 6; us_house is not in \
             zero_rows::allowed)",
            outcome.held.len(),
            payloads.len()
        );

        outcome.stats = build_stats(&samples, self.cfg, agreement, escalation.as_ref().map(|p| &p.usage));
        outcome.header = header;
        outcome.samples = samples;
        Ok(outcome)
    }
```

Evolve `route` (chain 3 → 4 → 17 → H31 → **H32**) to fold `pixel_verdicts` into the existing
violations lane, index-aligned with `verdicts`:

```rust
pub fn route(
    verdicts: Vec<RowVerdict>,
    spec: &ConsensusSpec,
    sanity: SanityCheck<'_>,
    pixel_verdicts: &[PixelVerdict],
    escalation: Option<&SamplePass>,
) -> DocOutcome {
    let mut outcome = DocOutcome::default();
    for (index, verdict) in verdicts.into_iter().enumerate() {
        let pixel_conflict = match pixel_verdicts.get(index) {
            Some(PixelVerdict::Conflict(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        match verdict {
            RowVerdict::Agreed { ordinal0, row } => {
                let mut violations = sanity(&row);
                violations.extend(pixel_conflict);
                let dissents = escalation.is_some_and(|pass| premium_dissents(&row, pass, spec));
                let confidence = if !violations.is_empty() || dissents {
                    policy::CONF_SANITY_CAPPED
                } else {
                    policy::CONF_AGREED
                };
                outcome.published.push(PublishedRow { ordinal0, row, confidence });
            }
            RowVerdict::Disputed { ordinal0, key, candidates, disputed_fields } => {
                let resolution = escalation.and_then(|pass| {
                    resolve_disputed(ordinal0, &key, &candidates, &disputed_fields, spec, &pass.payload)
                });
                match resolution {
                    Some(resolved) => {
                        let mut violations = sanity(&resolved.row);
                        violations.extend(pixel_conflict);
                        let confidence = if violations.is_empty() {
                            resolved.confidence
                        } else {
                            policy::CONF_SANITY_CAPPED
                        };
                        outcome.published.push(PublishedRow {
                            ordinal0: resolved.ordinal0,
                            row: resolved.row,
                            confidence,
                        });
                    }
                    None => outcome.held.push(HeldRow { ordinal0, competing: candidates }),
                }
            }
        }
    }
    outcome
}
```

Update `crates/pipeline/src/extraction/mod.rs`'s consensus re-export list to add `PixelSignal,
PixelVerdict` alongside the existing names.

In `crates/adapters/us_house/src/consensus.rs`, add the gray-zone pixel-ambiguity detector
(beside Task 18's `checkbox_sanity` — do not edit that function) and extend `consensus_spec()`:

```rust
/// Gray-zone width around `INK_THRESHOLD` (amendment-1 A15): an ink density
/// within this band of the threshold is a coin-flip read, not confident
/// evidence either way. Half the calibrated checked-vs-unchecked density
/// gap from Task 18's measurement procedure (checked ~0.41, unchecked
/// ~0.03 — see `INK_THRESHOLD`'s doc comment): narrow enough that a
/// clearly checked or unchecked cell never falls in it, wide enough to
/// catch a faint mark a single scan artifact could push either side of the
/// hard cutoff.
pub const GRAY_BAND: f32 = 0.05;

/// Ink density above which a band cell counts toward "checked" for the
/// ambiguity count, even below `INK_THRESHOLD` — smudges and stray marks
/// that are not a confident check but are not blank paper either.
const NOISE_FLOOR: f32 = 0.05;

/// The H32 pixel-ambiguity signal over Task 18's checkbox geometry: same
/// Cell-based one-call-per-row contract as `checkbox_sanity` (do not bind
/// both closures over the SAME row without H35b's shared classification —
/// see that task). Ambiguous when the amount-band row's ink density sits
/// within `GRAY_BAND` of `INK_THRESHOLD` (a marginal read), or when 2+ band
/// cells both read above `NOISE_FLOOR` (a smudge or a genuine double-mark).
pub fn pixel_ambiguity<'a>(
    page_images: &'a [image::GrayImage],
    geometry: &'a FormGeometry,
) -> impl Fn(&serde_json::Value) -> pipeline::extraction::consensus::PixelVerdict + Send + Sync + 'a
{
    use pipeline::extraction::consensus::PixelVerdict;
    let next_row = std::cell::Cell::new(0usize);
    move |_row: &serde_json::Value| {
        let row_index = next_row.get();
        next_row.set(row_index + 1);

        let Some(band) = geometry.rows.get(row_index) else {
            return PixelVerdict::Clear;
        };
        let Some(page) = page_images.get(band.page_index) else {
            return PixelVerdict::Clear;
        };

        let densities: Vec<f32> = band.bands.iter().map(|rect| ink_density(page, *rect)).collect();
        let above_noise_floor = densities.iter().filter(|&&d| d > NOISE_FLOOR).count();
        let near_threshold = densities.iter().any(|&d| (d - INK_THRESHOLD).abs() <= GRAY_BAND);

        if near_threshold || above_noise_floor >= 2 {
            PixelVerdict::Ambiguous
        } else {
            PixelVerdict::Clear
        }
    }
}
```

Extend `consensus_spec()`'s literal (Task 18/amendment-1 A11's `band_column` enum makes the
high-impact check letter-exact — no free-string band comparison):

```rust
pub fn consensus_spec() -> ConsensusSpec {
    ConsensusSpec {
        tool: consensus_tool_spec(),
        rows_pointer: "/rows".to_owned(),
        key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
        critical_fields: vec![
            "/band_column".to_owned(),
            "/over_1m_spouse_dc".to_owned(),
            "/transaction_type_raw".to_owned(),
            "/transaction_date_raw".to_owned(),
            "/notification_date_raw".to_owned(),
            "/owner_code_raw".to_owned(),
            "/asset_raw".to_owned(),
        ],
        high_impact_values: vec![(
            "/band_column".to_owned(),
            vec!["F".to_owned(), "G".to_owned(), "H".to_owned(), "I".to_owned(), "J".to_owned()],
        )],
        watchlist_pointer: Some("/filer_name_raw".to_owned()),
    }
}
```

(`key_fields`/`critical_fields` above match the state `consensus_spec()` already carries at
this execution frontier per amendment-1 A11/A12 — if your branch's Task 18 differs, keep its
`key_fields`/`critical_fields` exactly as landed and add ONLY the two new fields shown.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --test consensus_extraction`
Expected: PASS — `pixel_ambiguous_unanimous_row_with_premium_concurrence_keeps_0_90`,
`all_sixteen_premium_trigger_combinations_send_exactly_n_or_n_plus_one_never_n_plus_two` (16
iterations, one assertion each), plus every H31/Task 13/17 test with `no_pixel_ambiguity`
or an equivalent always-`Clear` closure threaded through their now-4-argument `extract()`
calls (update those pre-existing calls to pass `&no_pixel_ambiguity` as the fourth argument).

Run: `cargo test -p us_house --lib consensus::`
Expected: PASS — Task 18's four `checkbox_sanity` tests unchanged, plus this task's new
`pixel_ambiguity` unit tests (added alongside them, painting cells the same
`painted_page`-style way — see Step 3's fixture-writing helper in Task 18's test module).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/pipeline/src/extraction/consensus.rs \
        crates/pipeline/src/extraction/mod.rs \
        crates/adapters/us_house/src/consensus.rs \
        crates/pipeline/tests/consensus_extraction.rs
git commit -m "$(cat <<'EOF'
feat(pipeline): single-slot premium disjunction + pixel-ambiguity + high-impact floor (goal 021 v2 H32)

extract()'s premium_needed becomes ONE boolean (quality_flagged ||
pixel_ambiguous_any_row || any_disputed || high_impact_floor) computed
once, feeding the SAME Option<SamplePass> slot and ONE send site the
dispute branch already used (property-tested over all 16 trigger
combinations: request count is always n or n+1, never n+2). Implements the
Phase-2 controller resolution recorded in .superpowers/sdd/progress.md —
the committed plan's discarded `_high_impact_document` marker now actually
fires the premium pass on high-impact rows even under unanimous agreement.
us_house gains a gray-zone checkbox-ambiguity detector over Task 18's ROI
geometry (amendment-1 A15).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H33: Escalation params hardening (finding 7)

Codifies design D8/amendment-1 A7: the premium pass never receives a `thinking` config
(Sonnet 5 omission is adaptive), `output_config.effort` is an `extractor.toml` knob, and
`max_tokens` is sized so `effort`'s reasoning budget cannot starve the forced tool call.

**Files:**
- Modify: `crates/pipeline/src/extraction/config.rs` (`Effort` ALREADY lives here, Task 9 — it
  already derives `Serialize`/`Deserialize`, nothing to extend; EDIT Task 9's already-landed
  `EscalationConfig` in place — calibrated `impl Default`, `max_tokens: 8192` — `EscalationConfig`
  and `ExtractorConfig.escalation` themselves are ALREADY present, no re-declaration)
- Modify: `crates/pipeline/src/extraction/consensus.rs` (H32's escalation-building block in
  `extract()` reads `cfg.escalation.effort`/`.max_tokens`)
- Modify: `crates/pipeline/tests/consensus_extraction.rs` and/or
  `crates/pipeline/src/extraction/anthropic.rs`'s `#[cfg(test)] mod tests` (thinking-absence
  and max_tokens-override tests)

**Interfaces:**
- Consumes: `pipeline::extraction::config::Effort` (Task 9 — `Effort` is DEFINED in
  `config.rs`, not `anthropic.rs`, because the TOML loader deserializes it there; a `Copy`
  fieldless enum `Low | Medium | High`, already deriving `Serialize`/`Deserialize` — nothing for
  this task to extend); `pipeline::extraction::anthropic::{SamplingParams, build_image_request}`
  (Task 10 — `SamplingParams { temperature: Option<f32>, effort: Option<Effort> }`, importing
  `Effort` FROM `config.rs`; `build_image_request` already emits `output_config: {"effort":
  "low"|"medium"|"high"}` only when `sampling.effort.is_some()`, and NEVER emits a `thinking`
  key — read the current `anthropic.rs`/`config.rs` on your branch to confirm before wiring).
- Produces: an EDIT to Task 9's already-landed `pub struct EscalationConfig { pub effort:
  Option<Effort>, pub max_tokens: u32 }` (`ExtractorConfig.escalation: EscalationConfig` and its
  `#[serde(default)]` are ALREADY present, Task 9) — this task replaces its DERIVED `Default`
  (which would silently ship `max_tokens: 0`) with the calibrated `impl Default` = `{ effort:
  None, max_tokens: 8192 }`, matching `config/extractor.toml`'s committed `[escalation]` table;
  the escalation call site in `ConsensusExtractor::extract` now builds `SamplingParams {
  temperature: None, effort: self.cfg.escalation.effort }` and overrides the built request's
  `max_tokens` from `self.cfg.escalation.max_tokens` (`build_image_request`'s own signature is
  UNCHANGED — the override is a post-hoc field write on the returned `serde_json::Value`, the
  same pattern `build_image_request` itself already uses internally for the
  `temperature`/`output_config` keys, so every OTHER caller of `build_image_request` — the
  sample tier — is unaffected).

- [ ] **Step 1: Write the failing test**

Add to `crates/pipeline/src/extraction/anthropic.rs`'s existing `#[cfg(test)] mod tests`
(reusing `image_spec()` from Task 10):

```rust
    #[test]
    fn build_image_request_never_emits_a_thinking_key_for_either_tier() {
        let sample_tier = build_image_request(
            "claude-haiku-4-5-20251001",
            &[b"png-bytes".to_vec()],
            &image_spec(),
            &SamplingParams { temperature: Some(0.7), ..Default::default() },
        );
        assert!(
            sample_tier.get("thinking").is_none(),
            "sample-tier request must never carry a thinking key"
        );

        let escalation_tier = build_image_request(
            "claude-sonnet-5",
            &[b"png-bytes".to_vec()],
            &image_spec(),
            &SamplingParams { temperature: None, effort: Some(Effort::Medium) },
        );
        assert!(
            escalation_tier.get("thinking").is_none(),
            "escalation-tier request must never carry a thinking key (amendment-1 A7)"
        );
        assert_eq!(escalation_tier["output_config"]["effort"], serde_json::json!("medium"));
    }
```

Add to `crates/pipeline/src/extraction/config.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn escalation_config_parses_the_committed_max_tokens_and_absent_effort() {
        let cfg = ExtractorConfig::load_from(&config_path(), |_| None).unwrap();
        assert_eq!(cfg.escalation.max_tokens, 8192);
        assert_eq!(cfg.escalation.effort, None, "committed toml leaves effort commented out — adaptive");
    }
```

Add to `crates/pipeline/tests/consensus_extraction.rs`:

```rust
#[tokio::test]
async fn escalation_call_uses_the_configured_max_tokens_not_the_sample_tier_default() {
    let rows = json!([row("2026-06-01", "A")]);
    let sample_b = json!([row("2026-06-01", "B")]); // forces a dispute -> escalation fires
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(sample_b),
        tool_response(rows.clone()),
        tool_response(rows),
    ]);
    let mut cfg = test_cfg();
    cfg.escalation.max_tokens = 8192;
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![b"fake-png".to_vec()],
        vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }],
    );
    let _ = extractor
        .extract(b"%PDF-fake", &spec(), &no_sanity_issues, &no_pixel_ambiguity)
        .await
        .unwrap();

    let requests = transport.requests();
    assert_eq!(requests[3]["max_tokens"], json!(8192), "escalation request must use cfg.escalation.max_tokens");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pipeline --lib extraction::anthropic::tests::build_image_request_never_emits_a_thinking_key_for_either_tier`
Expected: PASS if `Effort`/`SamplingParams`/`build_image_request` already carry the effort/no-thinking
shape from Task 10 on your branch (this assertion is then a regression guard, not a red step) —
if it instead FAILS to compile (`error[E0433]: cannot find enum 'Effort'` /
`error[E0560]: struct 'SamplingParams' has no field named 'effort'`), Task 10 has not yet
landed the effort shape and this task's `anthropic.rs` edit (below) is required, not optional.

Run: `cargo test -p pipeline --lib extraction::config::tests::escalation_config_parses_the_committed_max_tokens_and_absent_effort`
Expected: PASS already (`ExtractorConfig.escalation` and its TOML parsing are Task 9's, landed —
this is a regression guard, not a red step; the DERIVED `Default`'s `max_tokens: 0` this task
fixes only matters for Rust-constructed `EscalationConfig::default()`, not for TOML-parsed
values — the committed `config/extractor.toml` sets `max_tokens = 8192` explicitly).

Run: `cargo test -p pipeline --test consensus_extraction escalation_call_uses_the_configured_max_tokens_not_the_sample_tier_default`
Expected: FAIL — an assertion failure (`requests[3]["max_tokens"]` is `json!(16000)`, the module
constant `MAX_TOKENS`, not `8192`) — `ExtractorConfig.escalation` itself already exists (Task
9), so this is not a compile error; `extract()` simply does not read it yet.

- [ ] **Step 3: Write minimal implementation**

`Effort` already lives in `crates/pipeline/src/extraction/config.rs` (Task 9), already deriving
`Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize` — this task does NOT
touch `anthropic.rs` for `Effort` and does NOT add `use crate::extraction::anthropic::Effort;`
anywhere (that direction is backwards: `anthropic.rs` imports `Effort` FROM `config.rs`, per
Task 10, never the other way).

In `crates/pipeline/src/extraction/config.rs`, EDIT Task 9's already-landed `EscalationConfig`
in place (above `#[cfg(test)] mod tests`) — drop the derived `Default` (which would silently
ship `max_tokens: 0`) in favor of the calibrated one below. `ExtractorConfig.escalation` and its
`#[serde(default)]` are ALREADY present (Task 9) — no re-declaration needed:

```rust
// EDIT: Task 9's landed `EscalationConfig` derives `#[derive(Debug, Clone,
// Default, Deserialize)]`, which silently ships `max_tokens: 0`. Change the
// derive to drop `Default` (add `Copy, PartialEq, Eq`), add a per-field
// serde default for max_tokens, and add the calibrated `impl Default`
// below. Field names/types are UNCHANGED from Task 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct EscalationConfig {
    pub effort: Option<Effort>,
    #[serde(default = "default_escalation_max_tokens")]
    pub max_tokens: u32,
}

/// `[escalation]` — premium-pass output shaping (amendment-1 A7, finding
/// 7). `effort` is emitted as `output_config.effort` only when `Some` (Task
/// 10's `build_image_request`); `max_tokens` sizes the escalation call so
/// the model's internal reasoning (when `effort` is set) cannot exhaust the
/// token budget before it emits the forced tool call. Sizing rationale: the
/// tool's own JSON output for a scanned PTR page tops out at roughly 2k
/// tokens (a handful of rows, each a few hundred tokens of schema-shaped
/// JSON); `8192` leaves >6k tokens of headroom for `effort: medium`-or-lower
/// reasoning before the tool call, well inside Sonnet 5's per-request
/// ceiling — `effort: high` is deliberately NOT the config default for this
/// reason (an operator raising it should also review this constant).
fn default_escalation_max_tokens() -> u32 {
    8192
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self { effort: None, max_tokens: default_escalation_max_tokens() }
    }
}
```

In `crates/pipeline/src/extraction/consensus.rs`, update `extract()`'s escalation-building
block (H32's version — only the `sampling` construction and the post-build `max_tokens`
override change; everything else in the block is unchanged):

```rust
        let escalation: Option<SamplePass> = if premium_needed {
            let sampling = SamplingParams { temperature: None, effort: self.cfg.escalation.effort };
            let mut request = build_image_request(
                &self.cfg.models.escalation,
                &preprocessed.pages_png,
                &spec.tool,
                &sampling,
            );
            request["max_tokens"] = serde_json::json!(self.cfg.escalation.max_tokens);
            let response = self
                .transport
                .send(&request)
                .await
                .context("consensus escalation call")?;
            let payload = extract_tool_payload(&response, &spec.tool.tool_name)
                .context("consensus escalation tool output")?;
            let usage = response.get("usage").cloned().unwrap_or(serde_json::Value::Null);
            Some(SamplePass { model_id: self.cfg.models.escalation.clone(), payload, usage })
        } else {
            None
        };
```

No `ExtractorConfig { .. }` struct literal needs a new field for this task — `escalation` has
been part of `ExtractorConfig`'s shape since Task 9, not added incrementally here.

The live-smoke `stop_reason == tool_use` assertion over a real escalation call belongs to Task
25 (changeset) — this task does not add or duplicate a live test; the offline tests above are
the complete acceptance surface for the params themselves.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --lib extraction::anthropic::tests::build_image_request_never_emits_a_thinking_key_for_either_tier`
Expected: PASS.

Run: `cargo test -p pipeline --lib extraction::config::tests::escalation_config_parses_the_committed_max_tokens_and_absent_effort`
Expected: PASS.

Run: `cargo test -p pipeline --test consensus_extraction escalation_call_uses_the_configured_max_tokens_not_the_sample_tier_default`
Expected: PASS.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/pipeline/src/extraction/anthropic.rs \
        crates/pipeline/src/extraction/config.rs \
        crates/pipeline/src/extraction/consensus.rs \
        crates/pipeline/tests/consensus_extraction.rs
git commit -m "$(cat <<'EOF'
feat(pipeline): EscalationConfig — configurable effort + sized max_tokens (goal 021 v2 H33)

The premium pass's output_config.effort becomes an extractor.toml knob
(default absent -> adaptive); max_tokens is sized (8192) so effort's
reasoning budget cannot starve the forced tool call; asserts NO request,
either tier, ever carries a thinking key (amendment-1 A7, finding 7).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H34: Row-count completeness gate (finding 3)

**This task deliberately exceeds the ~1h guideline** — the row-count gate spans `preprocess.rs`'s
projection-profile primitive, `consensus.rs`'s doc-level post-pass, and `extractor.rs`'s
review-task wiring as one coherent recall-gate unit; splitting would leave a firing gate with no
reviewer-visible task between commits. Do not split it.

Consensus is structurally blind to correlated omission: three sample passes skipping the same
faint row publish "complete" at 0.90. A horizontal-rule projection-profile primitive counts
POPULATED table rows per page independent of the model's own row count; a mismatch either
direction caps every published row of the document to 0.79 and opens a doc-level review task
(the row DTO carries no page attribution, so the cap cannot be row-scoped). This is the
system's only recall gate (goal §3 finding 3).

**Files:**
- Modify: `crates/pipeline/src/extraction/preprocess.rs` (add `rule_row_estimate`)
- Modify: `crates/pipeline/tests/preprocess.rs` (extend the fixture generator with a synthetic
  ruled-table PNG; pivotal tests over `rule_row_estimate`)
- Modify: `crates/pipeline/src/extraction/consensus.rs` (add `row_count_mismatch` pure helper;
  wire a doc-level post-pass in `extract()`)
- Modify: `crates/adapters/us_house/src/consensus.rs` (`consensus_spec()` gains `table_regions`)
- Modify: `crates/adapters/us_house/src/extractor.rs` (`persist_consensus_run` opens a
  `row_count_mismatch` review task, mirroring `persist_held`'s `consensus_row_hold` idiom)
- Modify: `crates/pipeline/tests/consensus_extraction.rs` (doc-level integration tests +
  `persist_row_count_mismatch`'s db-gated idempotency test)

**Interfaces:**
- Consumes: `pipeline::extraction::preprocess::NormRect` (Task 6); H32's `ConsensusSpec` (this
  task adds a THIRD new field, `table_regions`, alongside H32's `high_impact_values` /
  `watchlist_pointer`); H32's `extract()`/`route` (this task's post-pass runs AFTER `route`
  produces `outcome.published`/`outcome.held`, mutating confidence — it does not change
  `route`'s or `extract()`'s signature).
- Produces: `pub fn rule_row_estimate(img: &GrayImage, table_region: NormRect) -> u32`
  (`crates/pipeline/src/extraction/preprocess.rs` — horizontal-rule projection profile: counts
  POPULATED row bands between consecutive ruled lines inside `table_region`, not merely ruled
  lines); `ConsensusSpec.table_regions: Vec<NormRect>` (one region per page; empty = the
  adapter has no calibrated table geometry — the gate emits nothing, same convention H35b's
  guard relies on); a private `row_count_mismatch(estimated: u32, actual: u32,
  template_recognized: bool) -> Option<(u32, u32)>` pure helper in `consensus.rs`, and
  `extract()`'s post-`route` pass that calls it and, on `Some`, caps every `outcome.published`
  row's confidence to `policy::CONF_SANITY_CAPPED` and records `{"row_count_mismatch": {"estimated":
  .., "actual": ..}}` into `outcome.stats.agreement`. `template_recognized` is HARDCODED `true`
  in this task's `extract()` wiring (`crates/adapters/us_house/fixtures/scanned_paper_ptr` is
  the only calibrated revision at this execution frontier) — H35b replaces that literal with
  the real `classify_template(..).is_some()` result; this task's OWN pivotal test (c) below
  exercises the `false` branch directly against the pure helper, without needing
  `classify_template` to exist yet. Also: `pub async fn persist_row_count_mismatch(pool:
  &PgPool, document_sha256: &str, outcome: &DocOutcome) -> anyhow::Result<u32>`
  (`crates/adapters/us_house/src/extractor.rs`) — A3 mandates a review task for the cap; since
  `extract()` has no `pool`, the trigger routes through `outcome.stats.agreement` and committed
  Task 20's `persist_consensus_run` opens the task at the persist layer, mirroring
  `persist_held`/`consensus_row_hold` exactly.

- [ ] **Step 1: Write the failing test**

Append to `crates/pipeline/tests/preprocess.rs`, inside the fixture generator:

```rust
const TABLE_CANVAS_W: u32 = 800;
const TABLE_CANVAS_H: u32 = 300;

/// A synthetic ruled table: 4 horizontal rule lines bounding 3 row bands;
/// row bands 0 and 2 carry a short filled "content" mark, row band 1 is
/// left blank. `rule_row_estimate` over this fixture must return 2
/// (populated rows), not 3 (total ruled slots) — the H34 pivotal fixture.
fn generate_ruled_table_3rows_2populated() -> GrayImage {
    let mut img = GrayImage::from_pixel(TABLE_CANVAS_W, TABLE_CANVAS_H, Luma([255u8]));
    let rule_ys = [40u32, 100, 160, 220];
    for &y in &rule_ys {
        imageproc::drawing::draw_filled_rect_mut(
            &mut img,
            imageproc::rect::Rect::at(20, y as i32).of_size(TABLE_CANVAS_W - 40, 3),
            Luma([0u8]),
        );
    }
    // Content marks inside row bands 0 (40..100) and 2 (160..220); band 1 (100..160) stays blank.
    imageproc::drawing::draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(60, 60).of_size(300, 20),
        Luma([0u8]),
    );
    imageproc::drawing::draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(60, 180).of_size(300, 20),
        Luma([0u8]),
    );
    img
}
```

Add the save call inside `generate_preprocess_fixtures`:

```rust
    generate_ruled_table_3rows_2populated()
        .save(dir.join("ruled_table_3rows_2populated.png"))
        .unwrap();
```

Append the pivotal tests:

```rust
#[test]
fn rule_row_estimate_counts_populated_rows_not_total_ruled_slots() {
    let img = image::open(fixtures_dir().join("ruled_table_3rows_2populated.png"))
        .unwrap()
        .into_luma8();
    let table_region = NormRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
    let estimate = preprocess::rule_row_estimate(&img, table_region);
    assert_eq!(estimate, 2, "3 ruled slots, 2 populated — the gate estimates POPULATED rows");
}

#[test]
fn rule_row_estimate_is_zero_outside_the_table_region() {
    let img = image::open(fixtures_dir().join("ruled_table_3rows_2populated.png"))
        .unwrap()
        .into_luma8();
    let outside_region = NormRect { x: 0.0, y: 0.0, w: 0.02, h: 0.02 };
    assert_eq!(preprocess::rule_row_estimate(&img, outside_region), 0);
}
```

Add to `crates/pipeline/src/extraction/consensus.rs`'s `#[cfg(test)]` tests (a new small `mod
row_count_tests` beside the existing test modules, mirroring their `use super::*;` convention):

```rust
#[cfg(test)]
mod row_count_tests {
    use super::*;

    #[test]
    fn mismatch_is_reported_when_the_template_is_recognized() {
        assert_eq!(row_count_mismatch(3, 2, true), Some((3, 2)));
    }

    #[test]
    fn a_match_reports_nothing() {
        assert_eq!(row_count_mismatch(3, 3, true), None);
    }

    #[test]
    fn an_unrecognized_template_reports_nothing_even_on_a_real_mismatch() {
        assert_eq!(row_count_mismatch(3, 2, false), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pipeline --test preprocess`
Expected: FAIL to compile — `error[E0433]: failed to resolve: could not find 'rule_row_estimate' in 'preprocess'`.

Run: `cargo test -p pipeline --lib extraction::consensus::row_count_tests::`
Expected: FAIL to compile — `error[E0425]: cannot find function 'row_count_mismatch' in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add to `crates/pipeline/src/extraction/preprocess.rs`:

```rust
/// A row of the region counts as a printed RULE when its dark-pixel
/// fraction (within the region's width) exceeds this — a horizontal rule
/// spans nearly the full table width.
const RULE_ROW_DARK_FRACTION: f32 = 0.5;
/// A row band between two rules counts as POPULATED when its average
/// non-rule dark-pixel fraction exceeds this — well above stray
/// anti-aliasing noise, well below a rule row's own fraction.
const CONTENT_DARK_FRACTION: f32 = 0.02;

/// Horizontal-rule projection profile (goal 021 v2 H34, finding 3): counts
/// POPULATED table rows inside `table_region` — the number of row bands
/// (between consecutive ruled lines) whose average non-rule darkness
/// exceeds [`CONTENT_DARK_FRACTION`]. N ruled lines bound N-1 row-band
/// slots; a blank slot (no transaction printed) does not count. CPU-only,
/// ~$0 (design amendment-1 A3) — the only recall gate in the system.
#[must_use]
pub fn rule_row_estimate(img: &GrayImage, table_region: NormRect) -> u32 {
    let (width, height) = img.dimensions();
    let x0 = (table_region.x * width as f32).round().clamp(0.0, width as f32) as u32;
    let y0 = (table_region.y * height as f32).round().clamp(0.0, height as f32) as u32;
    let x1 = ((table_region.x + table_region.w) * width as f32)
        .round()
        .clamp(0.0, width as f32) as u32;
    let y1 = ((table_region.y + table_region.h) * height as f32)
        .round()
        .clamp(0.0, height as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        return 0;
    }
    let region_width = (x1 - x0) as f32;

    let mut row_darkness = Vec::with_capacity((y1 - y0) as usize);
    let mut is_rule_row = Vec::with_capacity((y1 - y0) as usize);
    for y in y0..y1 {
        let dark = (x0..x1).filter(|&x| img.get_pixel(x, y).0[0] < 128).count() as f32;
        let fraction = dark / region_width;
        is_rule_row.push(fraction > RULE_ROW_DARK_FRACTION);
        row_darkness.push(fraction);
    }

    let mut boundaries = Vec::new();
    let mut in_rule = false;
    for (i, &rule) in is_rule_row.iter().enumerate() {
        if rule && !in_rule {
            boundaries.push(i);
        }
        in_rule = rule;
    }
    if boundaries.len() < 2 {
        return 0;
    }

    let mut populated = 0u32;
    for window in boundaries.windows(2) {
        let (start, end) = (window[0], window[1]);
        let band: Vec<f32> = row_darkness[start..end]
            .iter()
            .copied()
            .filter(|&f| f <= RULE_ROW_DARK_FRACTION)
            .collect();
        if band.is_empty() {
            continue;
        }
        let avg = band.iter().sum::<f32>() / band.len() as f32;
        if avg > CONTENT_DARK_FRACTION {
            populated += 1;
        }
    }
    populated
}
```

Add `pub table_regions: Vec<NormRect>` to `ConsensusSpec` (alongside H32's two new fields) and
add `table_regions: Vec::new(),` to every existing `ConsensusSpec { .. }` literal (same
`grep`-and-patch step H32 already applies — do this in the SAME pass if both tasks land
together, or repeat the grep if H34 lands independently).

Add the pure gate helper and the `extract()` post-pass to `crates/pipeline/src/extraction/consensus.rs`:

```rust
/// Doc-level row-count completeness check (goal 021 v2 H34, finding 3):
/// `estimated` (summed `rule_row_estimate` across pages) vs `actual`
/// (published + held row count) — any mismatch is the system's only
/// recall gate. Gated by template recognition (H35b): `template_recognized
/// = false` returns `None` unconditionally — an unrecognized page has no
/// calibrated `table_regions`, so a raw pixel "mismatch" there is not
/// evidence of anything.
fn row_count_mismatch(estimated: u32, actual: u32, template_recognized: bool) -> Option<(u32, u32)> {
    if !template_recognized || estimated == actual {
        None
    } else {
        Some((estimated, actual))
    }
}
```

Append to `extract()`, AFTER `route` produces `outcome` and the `needs_llm_extraction` guard
(H32's version — insert this block between the `anyhow::ensure!` and the `outcome.stats = ..`
line; nothing before it changes):

```rust
        // Doc-level row-count completeness gate (H34, finding 3). Estimated
        // is summed independent of published/held split — the row DTO has
        // no page attribution, so the cap below is doc-level by
        // construction. `template_recognized` is HARDCODED true here; H35b
        // replaces this literal with the real classify_template(..) result.
        let template_recognized = true;
        if !spec.table_regions.is_empty() {
            let estimated: u32 = preprocessed
                .pages_png
                .iter()
                .zip(&spec.table_regions)
                .map(|(png, region)| {
                    image::load_from_memory(png)
                        .map(|decoded| preprocess::rule_row_estimate(&decoded.to_luma8(), *region))
                        .unwrap_or(0)
                })
                .sum();
            let actual = (outcome.published.len() + outcome.held.len()) as u32;
            if let Some((estimated, actual)) = row_count_mismatch(estimated, actual, template_recognized) {
                for published in &mut outcome.published {
                    published.confidence = policy::CONF_SANITY_CAPPED;
                }
                if let Some(obj) = outcome.stats.agreement.as_object_mut() {
                    obj.insert(
                        "row_count_mismatch".to_owned(),
                        serde_json::json!({ "estimated": estimated, "actual": actual }),
                    );
                } else {
                    outcome.stats.agreement = serde_json::json!({
                        "row_count_mismatch": { "estimated": estimated, "actual": actual }
                    });
                }
            }
        }
```

In `crates/adapters/us_house/src/consensus.rs`, add `table_regions` to `consensus_spec()`'s
literal, alongside H32's `high_impact_values`/`watchlist_pointer`:

**Measurement procedure** (Task-18 style, extends its Step 1-2): reuse the same 1600px raster
of `crates/adapters/us_house/fixtures/scanned_paper_ptr/input.pdf` Task 18 already produces;
locate the printed table's full ruled body (from the top rule under the column-header band
through the bottom rule of the last printed row) rather than individual cells; normalize the
bounding box the same way (`x = px_x / image_width as f32`, etc.).

```rust
/// The scanned PTR's printed table body (measured against the same 1600px
/// raster as `fixture_2f4b2b6e`'s checkbox geometry, spanning from the rule
/// under the column-header band through the last printed row's bottom
/// rule) — H34's `rule_row_estimate` region.
pub const TABLE_REGION_2F4B2B6E: NormRect = NormRect { x: 0.06, y: 0.38, w: 0.90, h: 0.10 };
```

```rust
pub fn consensus_spec() -> ConsensusSpec {
    ConsensusSpec {
        tool: consensus_tool_spec(),
        rows_pointer: "/rows".to_owned(),
        key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
        critical_fields: vec![
            "/band_column".to_owned(),
            "/over_1m_spouse_dc".to_owned(),
            "/transaction_type_raw".to_owned(),
            "/transaction_date_raw".to_owned(),
            "/notification_date_raw".to_owned(),
            "/owner_code_raw".to_owned(),
            "/asset_raw".to_owned(),
        ],
        high_impact_values: vec![(
            "/band_column".to_owned(),
            vec!["F".to_owned(), "G".to_owned(), "H".to_owned(), "I".to_owned(), "J".to_owned()],
        )],
        watchlist_pointer: Some("/filer_name_raw".to_owned()),
        table_regions: vec![TABLE_REGION_2F4B2B6E],
    }
}
```

Add doc-level integration tests to `crates/pipeline/tests/consensus_extraction.rs`:

```rust
#[tokio::test]
async fn row_count_mismatch_caps_every_published_row_to_0_79() {
    let rows = json!([row("2026-06-01", "A")]);
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
    ]);
    let cfg = test_cfg();
    let mut mismatch_spec = spec();
    mismatch_spec.table_regions = vec![NormRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }];
    // The fixed image is a 1x1-ish fake PNG that decodes to a table region
    // estimating 0 rows against 1 actually published -> guaranteed mismatch.
    let one_px = {
        let mut bytes = Vec::new();
        image::GrayImage::from_pixel(4, 4, image::Luma([255u8]))
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    };
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![one_px],
        vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }],
    );
    let outcome = extractor
        .extract(b"%PDF-fake", &mismatch_spec, &no_sanity_issues, &no_pixel_ambiguity)
        .await
        .unwrap();

    assert_eq!(outcome.published.len(), 1);
    assert_eq!(outcome.published[0].confidence, 0.79f32, "row_count_mismatch caps 0.79");
    assert_eq!(
        outcome.stats.agreement["row_count_mismatch"]["actual"],
        json!(1)
    );
}

#[tokio::test]
async fn matching_row_count_never_caps() {
    let rows = json!([row("2026-06-01", "A")]);
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
    ]);
    let cfg = test_cfg();
    // table_regions empty -> gate emits nothing regardless of the true count.
    let clean_spec = spec();
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![b"fake-png".to_vec()],
        vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }],
    );
    let outcome = extractor
        .extract(b"%PDF-fake", &clean_spec, &no_sanity_issues, &no_pixel_ambiguity)
        .await
        .unwrap();

    assert_eq!(outcome.published[0].confidence, 0.9f32, "no table_regions -> gate never fires");
    assert!(outcome.stats.agreement.get("row_count_mismatch").is_none());
}
```

**Doc-level review task (A3's remaining requirement).** A mismatch caps confidence, but that
mutation is invisible to a reviewer unless a review task exists. `extract()` itself has no
`pool` (this task's own Interfaces note: it is genuinely pure code), so the trigger routes
through `DocOutcome.stats.agreement["row_count_mismatch"]` — the SAME field the gate above
already writes — and committed Task 20's `persist_consensus_run` (`crates/adapters/us_house/
src/extractor.rs`, the ONE entry point every LLM-seam caller already invokes) is amended to
open it there, mirroring Task 20's own `persist_held`/`consensus_row_hold` idiom exactly (same
`open_review_task_once` dedup primitive, same document-scoped `"raw_document"`/`"us_house:
<sha256>"` target convention):

```rust
// crates/adapters/us_house/src/extractor.rs — new review reason, beside
// the existing REVIEW_REASON_ROW_HOLD:
pub const REVIEW_REASON_ROW_COUNT_MISMATCH: &str = "row_count_mismatch";

/// Opens one `row_count_mismatch` review task when `extract()`'s doc-level
/// row-count gate (H34) fired — read from `outcome.stats.agreement`, since
/// `extract()` has no `pool` and cannot open the task itself. Same target
/// convention as `persist_held`/`consensus_row_hold`: document-scoped,
/// idempotent.
///
/// Returns how many tasks were NEWLY opened (0 on a dedup rerun, or when
/// the gate did not fire).
///
/// # Errors
/// Database failure.
pub async fn persist_row_count_mismatch(
    pool: &PgPool,
    document_sha256: &str,
    outcome: &DocOutcome,
) -> anyhow::Result<u32> {
    if outcome.stats.agreement.get("row_count_mismatch").is_none() {
        return Ok(0);
    }
    let target_id = format!("us_house:{document_sha256}");
    let inserted =
        open_review_task_once(pool, "raw_document", &target_id, REVIEW_REASON_ROW_COUNT_MISMATCH)
            .await?;
    Ok(u32::from(inserted))
}
```

Wire it into `persist_consensus_run`, alongside the existing `persist_held` call:

```rust
pub async fn persist_consensus_run(
    pool: &PgPool,
    document_sha256: &str,
    samples: &[SamplePass],
    silver_rows: &[StagingRow],
    outcome: &DocOutcome,
    cfg: &ExtractorConfig,
    escalated: bool,
) -> anyhow::Result<()> {
    persist_samples(pool, document_sha256, CONSENSUS_TAG, samples).await?;
    persist_published(pool, document_sha256, silver_rows, outcome, cfg, escalated).await?;
    persist_held(pool, document_sha256, &outcome.held).await?;
    persist_row_count_mismatch(pool, document_sha256, outcome).await?; // H34
    Ok(())
}
```

Add one db-gated test to `crates/pipeline/tests/consensus_extraction.rs` (or wherever committed
Task 20's own `persist_held` idempotency test lives — mirror its exact structure):

```rust
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn row_count_mismatch_opens_exactly_one_review_task_and_dedupes_on_rerun(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    const SHA: &str = "shaH34RowCountMismatch00000000000000000000000000000000000000";
    // Construct via whichever DocOutcome API Task 17 actually shipped (a
    // struct literal or a builder) — read it first; the required state is
    // ONLY `stats.agreement` carrying a `row_count_mismatch` key.
    let mut outcome = empty_doc_outcome();
    outcome.stats.agreement = json!({"row_count_mismatch": {"estimated": 2, "actual": 3}});

    let inserted = persist_row_count_mismatch(&pool, SHA, &outcome).await.unwrap();
    assert_eq!(inserted, 1);
    let inserted_again = persist_row_count_mismatch(&pool, SHA, &outcome).await.unwrap();
    assert_eq!(inserted_again, 0, "rerun dedupes via open_review_task_once, no second task");

    let n_tasks: i64 = sqlx::query_scalar(
        "select count(*) from review_task where target_kind = 'raw_document' \
         and target_id = $1 and reason = 'row_count_mismatch'",
    )
    .bind(format!("us_house:{SHA}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n_tasks, 1);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --test preprocess`
Expected: PASS — `rule_row_estimate_counts_populated_rows_not_total_ruled_slots` (returns 2),
`rule_row_estimate_is_zero_outside_the_table_region`.

Run: `cargo test -p pipeline --lib extraction::consensus::row_count_tests::`
Expected: PASS — all three `row_count_mismatch` cases.

Run: `cargo test -p pipeline --test consensus_extraction`
Expected: PASS — `row_count_mismatch_caps_every_published_row_to_0_79`,
`matching_row_count_never_caps`, plus every earlier test (H31/H32/H33's `spec()`-users default
to `table_regions: Vec::new()`, so the gate is inert unless a test opts in).

Run: `cargo test -p us_house --lib consensus::`
Expected: PASS — Task 18's tests unchanged; `consensus_spec()`'s new field does not affect the
`checkbox_sanity` tests (they construct their own synthetic `FormGeometry`, not
`consensus_spec()`'s `ConsensusSpec`).

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p pipeline --test consensus_extraction row_count_mismatch_opens_exactly_one_review_task_and_dedupes_on_rerun -- --ignored`
Expected: PASS — one task opened, rerun dedupes to zero, exactly one `row_count_mismatch` row
in `review_task`.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/pipeline/src/extraction/preprocess.rs \
        crates/pipeline/tests/preprocess.rs \
        crates/pipeline/src/extraction/consensus.rs \
        crates/adapters/us_house/src/consensus.rs \
        crates/adapters/us_house/src/extractor.rs \
        crates/pipeline/tests/consensus_extraction.rs
git commit -m "$(cat <<'EOF'
feat(pipeline): row-count completeness gate — the only recall check (goal 021 v2 H34)

rule_row_estimate counts POPULATED table rows per page via a horizontal-
rule projection profile, CPU-only. A doc-level mismatch against the
consensus row count (either direction) caps every published row to 0.79 —
the system's only defense against three correlated passes silently
skipping the same faint row (design amendment-1 A3, finding 3). Gated by
template recognition; H35b wires the real classify_template result in.
persist_consensus_run now also opens a row_count_mismatch review task
(mirroring persist_held/consensus_row_hold) whenever the gate fires, read
from DocOutcome since extract() itself has no pool.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H35a: Template fingerprint primitive + 2026 revision fingerprint (finding 4)

All ROI geometry (Task 18's checkbox cells, H34's table region) is calibrated from the single
2026 committed fixture. Blind application to a different form revision (2012+ backfill) either
floods review with false 0.79 caps or silently voids the checks. Before ANY coordinate-based
pixel check runs, classify the printed template by ink-fingerprinting STATIC template regions
(present on every page regardless of transaction content) against known per-revision
fingerprints — goal §3 finding 4.

**Files:**
- Modify: `crates/adapters/us_house/src/consensus.rs` (`TemplateFingerprint`,
  `classify_template`, `fixture_2026_v1`)

**Interfaces:**
- Consumes: `pipeline::extraction::preprocess::{ink_density, NormRect}` (Task 6); `image::GrayImage`.
- Produces: `pub struct TemplateFingerprint { pub revision: &'static str, pub probes: Vec<(NormRect, f32)> }`
  (`probes` = static template regions with their MEASURED expected ink density); `pub fn
  classify_template(pages: &[image::GrayImage], known: &[TemplateFingerprint]) -> Option<&'static str>`
  (checks the FIRST page only — the form header/column-header band are page-1-only static
  content on the scanned paper PTR; returns the first fingerprint whose every probe is within
  `TEMPLATE_PROBE_TOLERANCE` of its measured expected density, `None` if no known revision
  matches); `pub const TEMPLATE_PROBE_TOLERANCE: f32`; `pub fn fixture_2026_v1() ->
  TemplateFingerprint` (revision literal `"2026-v1"`, probes measured from
  `crates/adapters/us_house/fixtures/scanned_paper_ptr/input.pdf`).

- [ ] **Step 1: Write the failing test**

Add to `crates/adapters/us_house/src/consensus.rs`'s existing `#[cfg(test)] mod tests` (reusing
Task 18's `fixtures_dir()`/`painted_page()`):

```rust
    #[test]
    fn the_2026_fixture_page_classifies_as_2026_v1() {
        let pdf = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("scanned_paper_ptr")
                .join("input.pdf"),
        )
        .unwrap();
        let pages = pipeline::extraction::preprocess::rasterize(&pdf, 1600).unwrap();
        let decoded: Vec<GrayImage> = pages
            .iter()
            .map(|png| image::load_from_memory(png).unwrap().into_luma8())
            .collect();
        let classified = classify_template(&decoded, &[fixture_2026_v1()]);
        assert_eq!(classified, Some("2026-v1"));
    }

    #[test]
    fn a_blank_page_classifies_as_none() {
        let blank = GrayImage::from_pixel(1600, 2000, image::Luma([255u8]));
        let classified = classify_template(&[blank], &[fixture_2026_v1()]);
        assert_eq!(classified, None, "a foreign/blank page must not match the 2026 fingerprint");
    }

    #[test]
    fn probe_tolerance_is_stated_and_positive() {
        assert!(TEMPLATE_PROBE_TOLERANCE > 0.0 && TEMPLATE_PROBE_TOLERANCE < 1.0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p us_house --lib consensus::tests::the_2026_fixture_page_classifies_as_2026_v1`
Expected: FAIL to compile — `error[E0433]: failed to resolve: could not find 'classify_template' in this scope` / `could not find 'fixture_2026_v1'`.

- [ ] **Step 3: Write minimal implementation**

**Measurement procedure** (Task-18 style, reuses its Step 1-3 exactly — same rasterized
fixture, same crop-and-view technique — extended to STATIC template regions instead of
per-row checkbox cells):

1. Rasterize the fixture at 1600px (same command as Task 18's Step 1) and view page 1 with the
   Read tool.
2. Locate two STATIC probe regions present regardless of transaction content: (a) the form
   header block (the printed title/instructions block above the table — present even on a
   filing with zero transactions), and (b) the column-header band (the row printing `P`/`S`/`S
   (partial)`/`E` and the ten `$…-$…` band headers, immediately above the first transaction
   row — the SAME band Task 18's `type_p`/`bands[..]` sit just below).
3. Crop narrow bounding boxes around each with `image::imageops::crop_imm`, re-view, narrow
   the guess the same way Task 18's Step 3 does.
4. Normalize each region the same way (`x = px_x / width`, etc.).
5. Measure `ink_density` at each region on the fixture page; record both values as the
   fingerprint's expected densities.
6. Calibrate `TEMPLATE_PROBE_TOLERANCE` between the fixture's measured densities and a blank
   page's (0.0 density at both probes, since a blank page prints neither the header block nor
   the column-header band) — any tolerance strictly less than the smaller measured density
   cleanly separates "fixture" from "blank/foreign", so a generous illustrative `0.08` is used
   below; replace with the measured margin once Step 2's numbers are recorded.

```rust
/// A form revision's fingerprint: STATIC template regions (present
/// regardless of transaction content) with their expected ink density,
/// measured once from a committed fixture (goal 021 v2 H35a, finding 4).
#[derive(Debug, Clone)]
pub struct TemplateFingerprint {
    pub revision: &'static str,
    pub probes: Vec<(NormRect, f32)>,
}

/// Maximum ink-density deviation from a probe's expected value before it
/// stops counting as a match. Measured margin: the 2026 fixture's two
/// probes both read well above 0.10 (printed text/rules); a blank or
/// differently-laid-out page reads near 0.0 at these SAME coordinates —
/// 0.08 sits strictly below the fixture's measured floor.
pub const TEMPLATE_PROBE_TOLERANCE: f32 = 0.08;

/// Classifies `pages`' first page against `known` fingerprints (goal 021
/// v2 H35a, finding 4). Runs BEFORE any coordinate-based pixel check
/// (Task 18's `checkbox_sanity`, H32's `pixel_ambiguity`, H34's
/// `rule_row_estimate`) — an unrecognized template makes H35b's guard make
/// all three emit nothing rather than apply 2026 geometry to a different
/// form layout.
#[must_use]
pub fn classify_template(pages: &[GrayImage], known: &[TemplateFingerprint]) -> Option<&'static str> {
    let page = pages.first()?;
    known
        .iter()
        .find(|fingerprint| {
            fingerprint
                .probes
                .iter()
                .all(|(region, expected)| (ink_density(page, *region) - expected).abs() <= TEMPLATE_PROBE_TOLERANCE)
        })
        .map(|fingerprint| fingerprint.revision)
}

/// The 2026 scanned-paper-PTR revision fingerprint, measured against
/// `crates/adapters/us_house/fixtures/scanned_paper_ptr/input.pdf` (sha256
/// 2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c) — see
/// the measurement procedure above this function's introducing task.
#[must_use]
pub fn fixture_2026_v1() -> TemplateFingerprint {
    TemplateFingerprint {
        revision: "2026-v1",
        probes: vec![
            // Form header block (title/instructions above the table).
            (NormRect { x: 0.06, y: 0.06, w: 0.88, h: 0.10 }, 0.18),
            // Column-header band (P/S/S(partial)/E + the ten band-letter headers).
            (NormRect { x: 0.06, y: 0.34, w: 0.90, h: 0.03 }, 0.22),
        ],
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p us_house --lib consensus::tests::`
Expected: PASS — `the_2026_fixture_page_classifies_as_2026_v1`, `a_blank_page_classifies_as_none`,
`probe_tolerance_is_stated_and_positive`, plus every Task 18 test unchanged. If
`the_2026_fixture_page_classifies_as_2026_v1` fails because the illustrative probe
coordinates/expected densities above do not match the real fixture, this is expected on the
FIRST run — perform the measurement procedure's Steps 1-5 against the actual rasterized page
and replace the two `NormRect`/expected-density pairs in `fixture_2026_v1` with the measured
values, then re-run.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/adapters/us_house/src/consensus.rs
git commit -m "$(cat <<'EOF'
feat(us_house): template fingerprint primitive + 2026-v1 revision (goal 021 v2 H35a)

classify_template ink-fingerprints two STATIC template regions (form
header, column-header band) against per-revision expected densities
measured from the committed fixture. Ungates H35b's guard over Task 18's
checkbox check, H32's pixel-ambiguity signal, and H34's row-count gate —
none of those may apply 2026 geometry to an unrecognized form revision
(design amendment-1 A4, finding 4).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H35b: Guard wiring over ALL pixel checks + AUTHORITY write-back (finding 4)

Wires H35a's `classify_template` in front of every coordinate-based pixel check: an
unrecognized template makes `checkbox_sanity` (Task 18), the `pixel_ambiguity` signal (H32),
and the row-count gate (H34) ALL emit nothing — no votes, no caps, no held rows from pixel
evidence — and records `"template_unrecognized": true` in stats instead. Same-task SAF
write-back: per-revision geometry is regime knowledge (AUTHORITY.md quirk), appended in this
commit alongside the guard.

**Files:**
- Modify: `crates/adapters/us_house/src/consensus.rs` (`checkbox_sanity` and `pixel_ambiguity`
  self-guard via `classify_template`; `consensus_spec()` becomes page-aware)
- Modify: `crates/pipeline/src/extraction/consensus.rs` (`extract()` reads
  `spec.template_recognized` for the stats flag and replaces H34's hardcoded
  `template_recognized = true` literal)
- Modify: `docs/regimes/us_house/AUTHORITY.md` (append to `## Quirks log (append-only, dated)`)
- Create (generated + committed by Step 2, not hand-authored):
  `crates/adapters/us_house/tests/fixtures/unknown_template.png` (the red-step fixture)

**Interfaces:**
- Consumes: H35a's `classify_template`, `fixture_2026_v1`, `TemplateFingerprint`
  (`crates/adapters/us_house/src/consensus.rs`); Task 18's `checkbox_sanity`; H32's
  `pixel_ambiguity`; H34's `consensus_spec()`/`table_regions`/`row_count_mismatch`.
- Produces: `ConsensusSpec.template_recognized: Option<bool>` (`None` = the adapter does not
  participate in template classification at all; `Some(true/false)` = it does, with the
  result) — set by us_house's now-page-aware `pub fn consensus_spec(pages: &[image::GrayImage])
  -> ConsensusSpec` (evolves Task 18's zero-argument `consensus_spec()`; every OTHER field is
  unchanged from H34's literal); `checkbox_sanity`/`pixel_ambiguity` (Task 18/H32) each
  internally call `classify_template` ONCE at closure-construction time and short-circuit to
  no violations / `PixelVerdict::Clear` when unrecognized — this needs NO change to Task
  24's (out-of-scope) call sites, since the guard lives inside the closures themselves;
  `extract()` (`crates/pipeline/src/extraction/consensus.rs`) reads `spec.template_recognized`
  for both the row-count gate's `template_recognized` value (replacing H34's hardcoded `true`)
  and the `"template_unrecognized": true` stats flag.

- [ ] **Step 1: Write the failing test**

Add to `crates/adapters/us_house/src/consensus.rs`'s existing `#[cfg(test)] mod tests`. First,
the concrete "unknown template" red-step fixture — a synthetic foreign page with NO ink at
`fixture_2026_v1()`'s two fingerprint probes (so `classify_template` genuinely reports `None`,
not merely "blank") but REAL ink painted into `test_geometry()`'s row-0 `type_s` checkbox cell
(so, absent the guard, `checkbox_sanity` WOULD detect a genuine conflict — this is what proves
the guard suppresses a REAL conflict, not just an absent one). No RNG, byte-deterministic, and
uses only `image::GrayImage::put_pixel` (no `imageproc` — that dependency does not land until
H42):

```rust
    /// A synthetic foreign page (goal 021 v2 H35b, finding 4): blank at both
    /// of `fixture_2026_v1()`'s fingerprint probes, but with a real ink mark
    /// in `test_geometry()`'s row-0 `type_s` checkbox cell.
    fn paint_unknown_template_page() -> GrayImage {
        let mut img = GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
        let geometry = test_geometry();
        let cell = geometry.rows[0].type_s;
        let x0 = (cell.x * 1600.0).round() as u32;
        let y0 = (cell.y * 2069.0).round() as u32;
        let w = ((cell.w * 1600.0).round() as u32).max(1);
        let h = ((cell.h * 2069.0).round() as u32).max(1);
        for y in y0..(y0 + h).min(2069) {
            for x in x0..(x0 + w).min(1600) {
                img.put_pixel(x, y, image::Luma([0u8]));
            }
        }
        img
    }

    /// Regenerates the committed `unknown_template.png` (mirrors Task 6/H31's
    /// `GOVFOLIO_GENERATE_*`-gated idiom — a normal `cargo test` never writes
    /// to the source tree).
    #[test]
    fn generate_unknown_template_fixture() {
        if std::env::var("GOVFOLIO_GENERATE_UNKNOWN_TEMPLATE_FIXTURE").is_err() {
            eprintln!(
                "SKIP: generate_unknown_template_fixture — set \
                 GOVFOLIO_GENERATE_UNKNOWN_TEMPLATE_FIXTURE=1 to (re)write the committed fixture"
            );
            return;
        }
        paint_unknown_template_page()
            .save(fixtures_dir().join("unknown_template.png"))
            .unwrap();
    }

    #[test]
    fn checkbox_sanity_emits_nothing_on_an_unrecognized_page_with_a_real_checkbox_mark() {
        let foreign = image::open(fixtures_dir().join("unknown_template.png"))
            .unwrap()
            .into_luma8();
        // Confirms this fixture is genuinely unrecognized (not merely
        // blank) — classify_template must read near-zero ink at BOTH
        // fingerprint probes despite the painted checkbox mark elsewhere.
        assert_eq!(classify_template(&[foreign.clone()], &[fixture_2026_v1()]), None);

        let geometry = test_geometry();
        let sanity = checkbox_sanity(&[foreign], &geometry);
        // The model claims "P" while the fixture's PAINTED mark is "S" — over
        // the 2026 fixture this WOULD be a genuine roi_checkbox_conflict
        // (same disagreement shape as Task 18's
        // type_checkbox_conflict_fires_when_pixel_says_p_but_model_says_s);
        // the guard must suppress it entirely here.
        let row = json!({"transaction_type_raw": "P"});
        assert!(
            sanity(&row).is_empty(),
            "an unrecognized template must suppress a REAL checkbox conflict, not just an absent one"
        );
    }

    #[test]
    fn pixel_ambiguity_emits_clear_on_an_unrecognized_page() {
        let blank = GrayImage::from_pixel(1600, 2000, image::Luma([255u8]));
        let geometry = test_geometry();
        let signal = pixel_ambiguity(&[blank], &geometry);
        assert!(matches!(
            signal(&json!({})),
            pipeline::extraction::consensus::PixelVerdict::Clear
        ));
    }

    #[test]
    fn consensus_spec_reports_template_unrecognized_for_a_blank_page() {
        let blank = GrayImage::from_pixel(1600, 2000, image::Luma([255u8]));
        let spec = consensus_spec(&[blank]);
        assert_eq!(spec.template_recognized, Some(false));
        assert!(spec.table_regions.is_empty(), "unrecognized template must not carry table geometry");
    }
```

Add to `crates/pipeline/tests/consensus_extraction.rs`:

```rust
#[tokio::test]
async fn unrecognized_template_row_count_mismatch_never_caps_and_flags_stats() {
    let rows = json!([row("2026-06-01", "A")]);
    let transport = MockTransport::returning(vec![
        tool_response(rows.clone()),
        tool_response(rows.clone()),
        tool_response(rows),
    ]);
    let cfg = test_cfg();
    let mut unrecognized_spec = spec();
    unrecognized_spec.table_regions = vec![NormRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }];
    unrecognized_spec.template_recognized = Some(false);
    let one_px = {
        let mut bytes = Vec::new();
        image::GrayImage::from_pixel(4, 4, image::Luma([255u8]))
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    };
    let extractor = ConsensusExtractor::with_fixed_images_and_quality(
        &transport,
        &cfg,
        vec![one_px],
        vec![QualityMetrics { residual_skew_deg: 0.0, otsu_variance: 1.0, noise_count: 0 }],
    );
    let outcome = extractor
        .extract(b"%PDF-fake", &unrecognized_spec, &no_sanity_issues, &no_pixel_ambiguity)
        .await
        .unwrap();

    assert_eq!(
        outcome.published[0].confidence, 0.9f32,
        "unrecognized template -> row-count gate emits nothing, no cap"
    );
    assert!(outcome.stats.agreement.get("row_count_mismatch").is_none());
    assert_eq!(outcome.stats.agreement["template_unrecognized"], json!(true));
}
```

- [ ] **Step 2: Run test to verify it fails**

First, run once locally to produce the committed fixture (Step 5 commits it):

```bash
GOVFOLIO_GENERATE_UNKNOWN_TEMPLATE_FIXTURE=1 cargo test -p us_house --lib \
  consensus::tests::generate_unknown_template_fixture -- --nocapture
```

Run: `cargo test -p us_house --lib consensus::tests::checkbox_sanity_emits_nothing_on_an_unrecognized_page_with_a_real_checkbox_mark`
Expected: FAIL — `checkbox_sanity` still applies 2026 geometry unconditionally (no guard yet),
so it correctly DETECTS the painted "S" mark vs. the model's claimed "P" and returns a non-empty
violation — `sanity(&row).is_empty()` fails. This is the real pre-guard behavior the fixture was
built to prove (not an unrelated blank-page no-op): the SAME disagreement shape as Task 18's
`type_checkbox_conflict_fires_when_pixel_says_p_but_model_says_s`, over a page that is
independently confirmed unrecognized by the `classify_template` assertion earlier in the test.

Run: `cargo test -p pipeline --test consensus_extraction unrecognized_template_row_count_mismatch_never_caps_and_flags_stats`
Expected: FAIL to compile — `error[E0609]: no field 'template_recognized' on type 'ConsensusSpec'`.

- [ ] **Step 3: Write the guard + write-back**

In `crates/adapters/us_house/src/consensus.rs`, wrap `checkbox_sanity` (Task 18) — classify
ONCE at construction, short-circuit if unrecognized:

```rust
pub fn checkbox_sanity<'a>(
    page_images: &'a [image::GrayImage],
    geometry: &'a FormGeometry,
) -> impl Fn(&serde_json::Value) -> Vec<String> + Send + Sync + 'a {
    let recognized = classify_template(page_images, &[fixture_2026_v1()]).is_some();
    let next_row = std::cell::Cell::new(0usize);
    move |row: &serde_json::Value| {
        let row_index = next_row.get();
        next_row.set(row_index + 1);
        if !recognized {
            return Vec::new();
        }
        let Some(band) = geometry.rows.get(row_index) else {
            return Vec::new();
        };
        let Some(page) = page_images.get(band.page_index) else {
            return Vec::new();
        };

        let mut violations = Vec::new();

        let type_cells: [(&str, NormRect); 4] = [
            ("P", band.type_p),
            ("S", band.type_s),
            ("S (partial)", band.type_s_partial),
            ("E", band.type_e),
        ];
        let checked_types: Vec<&str> = type_cells
            .iter()
            .filter(|(_, rect)| ink_density(page, *rect) > INK_THRESHOLD)
            .map(|(token, _)| *token)
            .collect();
        if let [only] = checked_types.as_slice()
            && let Some(model_type) = row.get("transaction_type_raw").and_then(Value::as_str)
            && model_type != *only
        {
            violations.push("roi_checkbox_conflict:transaction_type_raw".to_owned());
        }

        // Post-A11 (amendment-1): the model transcribes the printed band
        // COLUMN LETTER (A..J, `band_column`), never the verbatim dollar
        // string — decorrelates shared band-transcription errors and makes
        // this pixel cross-check index-to-index exact against BAND_LETTERS,
        // not BANDS' verbatim labels.
        const BAND_LETTERS: [&str; 10] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];
        let checked_band_letters: Vec<&str> = BAND_LETTERS
            .iter()
            .zip(band.bands.iter())
            .filter(|(_, rect)| ink_density(page, **rect) > INK_THRESHOLD)
            .map(|(letter, _)| *letter)
            .collect();
        if let [only] = checked_band_letters.as_slice()
            && let Some(model_band_column) = row.get("band_column").and_then(Value::as_str)
            && model_band_column != *only
        {
            violations.push("roi_checkbox_conflict:band_column".to_owned());
        }

        violations
    }
}
```

(Body unchanged from Task 18 below the new `recognized` guard, EXCEPT the band-column field
rename above — amendment-1 A11 renamed the free-string `amount_raw`/verbatim-label comparison
to the closed-vocab `band_column`/letter comparison; `transaction_type_raw`'s string
representation is unaffected, since `TransactionType`'s `serde(rename)`s already match Task 18's
existing token strings exactly.)

Apply the identical guard to H32's `pixel_ambiguity`:

```rust
pub fn pixel_ambiguity<'a>(
    page_images: &'a [image::GrayImage],
    geometry: &'a FormGeometry,
) -> impl Fn(&serde_json::Value) -> pipeline::extraction::consensus::PixelVerdict + Send + Sync + 'a
{
    use pipeline::extraction::consensus::PixelVerdict;
    let recognized = classify_template(page_images, &[fixture_2026_v1()]).is_some();
    let next_row = std::cell::Cell::new(0usize);
    move |_row: &serde_json::Value| {
        let row_index = next_row.get();
        next_row.set(row_index + 1);
        if !recognized {
            return PixelVerdict::Clear;
        }
        let Some(band) = geometry.rows.get(row_index) else {
            return PixelVerdict::Clear;
        };
        let Some(page) = page_images.get(band.page_index) else {
            return PixelVerdict::Clear;
        };
        let densities: Vec<f32> = band.bands.iter().map(|rect| ink_density(page, *rect)).collect();
        let above_noise_floor = densities.iter().filter(|&&d| d > NOISE_FLOOR).count();
        let near_threshold = densities.iter().any(|&d| (d - INK_THRESHOLD).abs() <= GRAY_BAND);
        if near_threshold || above_noise_floor >= 2 {
            PixelVerdict::Ambiguous
        } else {
            PixelVerdict::Clear
        }
    }
}
```

Evolve `consensus_spec()` to be page-aware (H34's literal, plus the guard):

```rust
pub fn consensus_spec(pages: &[image::GrayImage]) -> ConsensusSpec {
    let recognized = classify_template(pages, &[fixture_2026_v1()]).is_some();
    ConsensusSpec {
        tool: consensus_tool_spec(),
        rows_pointer: "/rows".to_owned(),
        key_fields: vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()],
        critical_fields: vec![
            "/band_column".to_owned(),
            "/over_1m_spouse_dc".to_owned(),
            "/transaction_type_raw".to_owned(),
            "/transaction_date_raw".to_owned(),
            "/notification_date_raw".to_owned(),
            "/owner_code_raw".to_owned(),
            "/asset_raw".to_owned(),
        ],
        high_impact_values: vec![(
            "/band_column".to_owned(),
            vec!["F".to_owned(), "G".to_owned(), "H".to_owned(), "I".to_owned(), "J".to_owned()],
        )],
        watchlist_pointer: Some("/filer_name_raw".to_owned()),
        table_regions: if recognized { vec![TABLE_REGION_2F4B2B6E] } else { Vec::new() },
        template_recognized: Some(recognized),
    }
}
```

(Task 24's call site, out of this task's scope, is updated by the changeset workstream to pass
`us_house::consensus::consensus_spec(&pages)` instead of the old zero-argument form — reference,
do not duplicate that edit here.)

Add `pub template_recognized: Option<bool>` to `ConsensusSpec`
(`crates/pipeline/src/extraction/consensus.rs`) and add `template_recognized: None,` to every
OTHER existing `ConsensusSpec { .. }` literal that is not us_house's (pipeline-internal test
`spec()` helpers default to "adapter does not classify templates" — `grep -n "ConsensusSpec {"
crates/pipeline/src/extraction/consensus.rs crates/pipeline/tests/*.rs` and patch each hit,
same discipline as H32/H34's field additions).

Update `extract()`'s row-count block (H34's version) to consume it instead of the hardcoded
literal, and set the stats flag:

```rust
        let template_recognized = spec.template_recognized.unwrap_or(true);
        if spec.template_recognized == Some(false)
            && let Some(obj) = outcome.stats.agreement.as_object_mut()
        {
            obj.insert("template_unrecognized".to_owned(), serde_json::json!(true));
        }
        if !spec.table_regions.is_empty() {
            // .. unchanged from H34, now using `template_recognized` above
            // instead of a literal `true`.
        }
```

- [ ] **Step 3b: Write the SAF write-back**

Append to `docs/regimes/us_house/AUTHORITY.md`'s `## Quirks log (append-only, dated)` section
(after the existing 2026-07-06 `robots.txt` entry):

```
- 2026-07-08 · **Per-revision template geometry is regime knowledge (goal 021 v2 H35a/b,
  finding 4)**: every coordinate-based pixel check over the scanned paper PTR (Task 18's ROI
  checkbox cross-check, the H32 pixel-ambiguity signal, and H34's row-count gate) is
  calibrated ONLY against the single 2026 committed fixture (`scanned_paper_ptr/input.pdf`,
  sha256 `2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c`). A
  `classify_template` ink-fingerprint check (two static probes: the form header block and the
  column-header band) runs before ANY of those three checks; an unrecognized page — expected
  for pre-2015/backfill form revisions once that scope opens (see the 2026-07-06 "Historical
  index schema fork" entry above) — makes all three emit nothing (no votes, no caps) and logs
  `template_unrecognized: true` in `pipeline_run.stats` instead of a false positive. Adding a
  new revision means measuring a NEW `TemplateFingerprint` + geometry set against a fixture of
  that revision, not extending the 2026 coordinates.
```

Then re-run the validator:

```bash
cargo run -p pipeline --bin validate-survey -- us_house
```
Expected: PASS (front matter unchanged, only the append-only Quirks log grew).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p us_house --lib consensus::tests::`
Expected: PASS — `checkbox_sanity_emits_nothing_on_an_unrecognized_page_with_a_real_checkbox_mark`,
`pixel_ambiguity_emits_clear_on_an_unrecognized_page`,
`consensus_spec_reports_template_unrecognized_for_a_blank_page`, plus every prior Task
18/H32/H35a test unchanged (they all pass the REAL 2026 fixture pages, which still classify
`Some("2026-v1")`, so `recognized` stays `true` and behavior is byte-identical to before this
task).

Run: `cargo test -p pipeline --test consensus_extraction unrecognized_template_row_count_mismatch_never_caps_and_flags_stats`
Expected: PASS.

Run: `cargo run -p pipeline --bin validate-survey -- us_house`
Expected: PASS.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
--workspace && cargo run -p pipeline --bin conformance -- us_house`

- [ ] **Step 5: Commit**
```bash
git add crates/adapters/us_house/src/consensus.rs \
        crates/pipeline/src/extraction/consensus.rs \
        crates/pipeline/tests/consensus_extraction.rs \
        crates/adapters/us_house/tests/fixtures/unknown_template.png \
        docs/regimes/us_house/AUTHORITY.md
git commit -m "$(cat <<'EOF'
feat(us_house): template-recognition guard over every pixel check + SAF write-back (goal 021 v2 H35b)

checkbox_sanity, the H32 pixel-ambiguity signal, and H34's row-count gate
all self-guard via H35a's classify_template: an unrecognized form revision
makes every one of them emit nothing rather than apply 2026-calibrated
geometry to a different layout — `template_unrecognized: true` lands in
stats instead. Per-revision geometry recorded as regime knowledge in
AUTHORITY.md's quirks log (design amendment-1 A4, finding 4).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H36: Batch outcome parity — Silver mapping at poll (review catch)

**This task deliberately exceeds the ~1h guideline** — the Silver-mapping fix threads through
both `us_house::extractor`'s `silver_rows` rename/AMENDED body and `worker::consensus_batch`'s
`resolve_document` rewrite together; splitting would leave the batch path caching a shape
`validated()` still rejects, between commits. Do not split it.

**Review-catch context.** The committed Task 23 `resolve_document` (`crates/worker/src/consensus_batch.rs`) pg_puts the RAW comparator DTO `Value` rows straight from `RowVerdict`/`resolve_disputed` — it never threads `doc_id`, never majority-votes the document header, and never maps through a Silver-row shape. Task 24's `validated()` (the SilverRow-deserialize + extractor-tag gate that guards EVERY `extraction_cache` read, sync or batch) rejects any row that is not `{doc_id, row_ordinal, ..., extractor}`-shaped. Consequence: a batch-resolved document can never publish through the adapter cache path — the very next tier-2 read of that sha freezes on `validated()`'s own fail-closed error. This task fixes that by giving the sync and batch paths ONE shared Silver-mapping function they both call, so they can never diverge again.

**Files:**
- Modify: `crates/adapters/us_house/src/extractor.rs` (rename+export `to_staging_rows` → `pub fn silver_rows`; export `ConsensusHeader`; export `doc_id_from_url`; update `extract_live`'s call site; update the three Task-24 tests that reference `to_staging_rows`)
- Modify: `crates/worker/src/consensus_batch.rs` (`resolve_document`: majority-vote the header, resolve `doc_id`, map through `silver_rows`, then `pg_put`)
- Test: `crates/worker/tests/consensus_batch_poll.rs` (new db-gated pivotal test + a reusable `CountingTransport`/`test_spec` fixture H37 also uses)

**Interfaces:**
- Consumes (Task 24's cutover — read `crates/adapters/us_house/src/extractor.rs` first; this task's pivotal test asserts against the POST-cutover tag `"us_house_ptr/consensus@1"` and the exact-set-membership `validated()`, so land Task 24 before running this task's Step 2/4 if it has not landed yet on this branch): the pre-rename SIGNATURES verbatim from Task 24's own text — the BODY this task's Step 3 works from is Task 24's §3f AMENDED text (goal 021 Phase 3: deserializes `crate::consensus::LlmConsensusRow`, maps `amount_raw` via `crate::consensus::band_from_column`, closed-vocab fields via `enum_field_str`), NOT the frozen v1 `LlmTransactionRow` body —
  ```rust
  struct ConsensusHeader { filer_name_raw: String, filer_status_raw: String, state_district_raw: String, signed_date_raw: String }
  fn to_staging_rows(header: &ConsensusHeader, published: &[PublishedRow], doc_id: &str) -> anyhow::Result<Vec<StagingRow>>
  fn doc_id_from_url(url: &str) -> Option<String>
  ```
  `pipeline::extraction::consensus::{align, score, vote_header, policy, resolve_disputed, ConsensusSpec, RowVerdict, PublishedRow}` (Task 17/23, post-H29 arity — `RowVerdict::Disputed` carries `key: RowKey` and `resolve_disputed` takes `key: &RowKey` as its second argument per the H29 changeset, already landed by the time this task runs per the merged execution order).
- Produces (this task): `pub fn silver_rows(header: &ConsensusHeader, published: &[PublishedRow], doc_id: &str) -> anyhow::Result<Vec<StagingRow>>` and `pub struct ConsensusHeader` and `pub fn doc_id_from_url(url: &str) -> Option<String>` in `us_house::extractor` — the ONE Silver-mapping fn both `extract_live` (sync) and `worker::consensus_batch::resolve_document` (batch) call; `worker::consensus_batch::poll_resolve_doc_id(pool, sha) -> anyhow::Result<Option<String>>` (the batch-side doc_id lookup — same query + same `doc_id_from_url` mapping the sync `resolve_doc_id` uses, duplicated only because the sync fn's signature takes a `RunCtx` — bronze + clock + politeness client — this poll-only path has no reason to construct).

- [ ] **Step 1: Write the failing test**

Add to `crates/worker/tests/consensus_batch_poll.rs` (this file already has `row`/`succeeded`/`record_batch_submitted`/`ingest_batch_results`/`ExtractionBatchRow`/`CONSENSUS_TAG`/`COMPOSITE_MODEL_ID` from Task 23, plus its own top-level `use async_trait::async_trait;` and `use pipeline::extraction::{BatchTransport, CacheKey, pg_get};` — reuse all of those, do NOT re-import `async_trait`, `CacheKey`, or `pg_get` a second time (duplicate `use` of the same name is a hard compile error, E0252); only the NEW names below need a new `use` line):

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

use pipeline::adapter::BronzeStore;
use pipeline::extraction::anthropic::DocumentToolSpec;
use pipeline::extraction::consensus::ConsensusSpec;
use pipeline::extraction::preprocess::PreprocessCfg;
use pipeline::extraction::Transport;
use worker::consensus_batch::resolve_document;

/// Counts `send` calls and always errors — proves an escalation call never
/// happens on a scenario where none should fire (H36: full agreement).
#[derive(Default)]
struct NeverCalledTransport {
    calls: AtomicUsize,
}

#[async_trait]
impl Transport for NeverCalledTransport {
    async fn send(&self, _body: &Value) -> anyhow::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("NeverCalledTransport: unexpected escalation call")
    }
}

/// A minimal, self-contained `ConsensusSpec` independent of the real
/// us_house schema (Task 18's `LlmConsensusRow`/`band_column` shape is a
/// forward reference from this test's perspective — `align`/`score`/
/// `vote_header` operate purely on the `key_fields`/`critical_fields`/
/// `rows_pointer` a spec supplies, so this fixture proves the GENERIC
/// batch-parity mechanism without depending on it). If
/// `us_house::consensus::consensus_spec()` has landed by the time this test
/// runs, both work — this fixture stays as the fast, network-schema-agnostic
/// path.
fn test_spec() -> ConsensusSpec {
    ConsensusSpec {
        tool: DocumentToolSpec {
            tool_name: "record_rows".to_owned(),
            tool_description: "record every row".to_owned(),
            input_schema: json!({"type": "object"}),
            prompt: "transcribe every row".to_owned(),
        },
        rows_pointer: "/rows".to_owned(),
        key_fields: vec![
            "/asset_raw".to_owned(),
            "/transaction_date_raw".to_owned(),
        ],
        critical_fields: vec![
            "/band_column".to_owned(),
            "/transaction_type_raw".to_owned(),
            "/owner_code_raw".to_owned(),
        ],
    }
}

/// The document-header fields `vote_header` majority-votes, carried
/// top-level in the SAME sample payload as `/rows` (`ConsensusHeader`'s
/// field names, Task 24).
fn header_fields() -> Value {
    json!({
        "filer_name_raw": "Diana Harshbarger",
        "filer_status_raw": "Member",
        "state_district_raw": "TN01",
        "signed_date_raw": "2026 MAY -6",
    })
}

/// Builds one sample's document payload with a row matching Task 18's
/// `LlmConsensusRow` shape (`#[serde(deny_unknown_fields)]` — `silver_rows`
/// deserializes an agreed/resolved row straight into that DTO, so this
/// fixture's field names and closed-vocab values must match it exactly):
/// `band_column` is the printed column LETTER (A..J), never a dollar-band
/// string, and `over_1m_spouse_dc` is the separate column-K boolean.
fn sample_payload(band_column: &str) -> Value {
    let mut payload = header_fields();
    payload["rows"] = json!([{
        "row_id_raw": null,
        "owner_code_raw": "SP",
        "asset_raw": "Boeing Co",
        "asset_type_code_raw": null,
        "transaction_type_raw": "S",
        "transaction_date_raw": "1/5/2026",
        "notification_date_raw": "1/9/2026",
        "band_column": band_column,
        "over_1m_spouse_dc": false,
        "cap_gains_over_200": null,
        "filing_status_raw": "New",
        "subholding_of_raw": null,
        "description_raw": null,
        "comments_raw": null,
        "vehicle_owner_code_raw": null,
        "vehicle_location_raw": null,
    }]);
    payload
}

async fn insert_raw_document(pool: &PgPool, sha: &str, doc_id: &str) {
    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, source_url, fetched_at) \
         values ($1, $2, $3, 'application/pdf', $4, now())",
    )
    .bind(format!("01H36RAWDOC{doc_id:>016}"))
    .bind("file:///dev/null")
    .bind(sha)
    .bind(format!(
        "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/{doc_id}.pdf"
    ))
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn resolve_document_caches_silver_shaped_rows_and_is_idempotent(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    const SHA: &str = "shaH36FullAgreement00000000000000000000000000000000000000000";
    insert_raw_document(&pool, SHA, "9115899").await;

    record_batch_submitted(
        &pool,
        "msgbatch_h36",
        "us_house",
        CONSENSUS_TAG,
        COMPOSITE_MODEL_ID,
        &[SHA.to_owned()],
    )
    .await
    .unwrap();
    let results = vec![
        (format!("{SHA}:0"), succeeded(sample_payload("B"))),
        (format!("{SHA}:1"), succeeded(sample_payload("B"))),
        (format!("{SHA}:2"), succeeded(sample_payload("B"))),
    ];
    ingest_batch_results(&pool, "msgbatch_h36", results).await.unwrap();

    let bronze =
        BronzeStore::open(std::env::temp_dir().join("govfolio-h36-test-bronze")).unwrap();
    let preprocess_cfg = PreprocessCfg { max_edge: 1568 };
    let transport = NeverCalledTransport::default();
    let spec = test_spec();

    resolve_document(
        &pool,
        &transport,
        &bronze,
        SHA,
        "us_house",
        CONSENSUS_TAG,
        COMPOSITE_MODEL_ID,
        "claude-sonnet-5",
        &spec,
        &preprocess_cfg,
    )
    .await
    .unwrap();
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "3/3 agreement must never escalate"
    );

    let key = CacheKey::new(SHA, CONSENSUS_TAG, COMPOSITE_MODEL_ID);
    let cached = pg_get(&pool, &key)
        .await
        .unwrap()
        .expect("published rows must be cached");
    assert_eq!(cached.len(), 1);
    let payload = &cached[0].payload;
    // A `validated()`-equivalent check: SilverRow-shaped, tagged, and an
    // exact policy_v1 confidence-set member — everything `validated()` (Task
    // 24) itself checks, without depending on the private `SilverRow` type.
    assert_eq!(payload["extractor"], json!("us_house_ptr/consensus@1"));
    assert_eq!(payload["doc_id"], json!("9115899"));
    assert_eq!(payload["row_ordinal"], json!(1));
    assert_eq!(payload["filer_name_raw"], json!("Diana Harshbarger"));
    assert!(
        [0.9_f32, 0.75, 0.79]
            .iter()
            .any(|c| c.to_bits() == cached[0].confidence.to_bits()),
        "confidence must be an exact policy_v1 member: {}",
        cached[0].confidence
    );

    // A second resolution of the same document: idempotent (pg_put's ON
    // CONFLICT DO NOTHING), no duplicate cache row.
    resolve_document(
        &pool,
        &transport,
        &bronze,
        SHA,
        "us_house",
        CONSENSUS_TAG,
        COMPOSITE_MODEL_ID,
        "claude-sonnet-5",
        &spec,
        &preprocess_cfg,
    )
    .await
    .unwrap();
    let cached_again = pg_get(&pool, &key).await.unwrap().unwrap();
    assert_eq!(cached_again.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test consensus_batch_poll -- --ignored`
Expected: FAIL to compile — `resolve_document`'s current (pre-H36) body pg_puts raw DTO rows and never calls `silver_rows`/`vote_header`/`poll_resolve_doc_id`, none of which exist yet; the compiler also has no `NeverCalledTransport`/`test_spec` conflicts (new names).

- [ ] **Step 3: Write minimal implementation**

In `crates/adapters/us_house/src/extractor.rs`: make `ConsensusHeader` and `doc_id_from_url` `pub`, and rename `to_staging_rows` to `pub fn silver_rows` (identical body — only the signature's visibility and name change):

```rust
/// One row as `silver_rows` needs it: the document header (threaded once per
/// document, NOT re-sampled per row) plus the row itself. `pub` (goal 021
/// Phase 3 H36 — review catch): the batch-poll path
/// (`worker::consensus_batch::resolve_document`) deserializes into this
/// EXACT type from its own `vote_header` call so sync and batch never
/// diverge on header identity.
#[derive(Debug, Clone, Deserialize)]
pub struct ConsensusHeader {
    filer_name_raw: String,
    filer_status_raw: String,
    state_district_raw: String,
    signed_date_raw: String,
}
```

```rust
/// Renders one of `LlmConsensusRow`'s closed-vocab enum fields back to the
/// verbatim string v1's free-`String` equivalent carried — the enums'
/// `Serialize` impls (Task 18) were deliberately named/renamed to match
/// those strings exactly, so this is a mechanical round-trip through
/// `serde_json`, not a lossy remapping. (Task 24 §3f AMENDED — goal 021
/// Phase 3; reused verbatim here, this task does not redefine it if Task 24
/// already landed it in this file.)
fn enum_field_str<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    let rendered = serde_json::to_value(value).context("serializing consensus enum field")?;
    rendered
        .as_str()
        .map(str::to_owned)
        .context("consensus enum field did not serialize to a JSON string")
}

/// Assembles Silver rows from a consensus outcome: `doc_id` and
/// `row_ordinal` are threaded here (the model never knows them), the tag is
/// stamped, and each row's confidence is exactly whatever `consensus::score`
/// assigned — never re-derived here. `pub` + renamed from `to_staging_rows`
/// (goal 021 Phase 3 H36 — review catch): the batch-poll path
/// (`worker::consensus_batch::resolve_document`) reuses this EXACT fn so
/// sync and batch extraction can never diverge on how a consensus outcome
/// becomes a cached Silver row — before this change, the batch path cached
/// the raw comparator DTO verbatim, which `validated()`'s SilverRow + tag
/// gate rejected on the very next read. BODY is Task 24's §3f AMENDED text
/// (goal 021 Phase 3): deserializes `crate::consensus::LlmConsensusRow`
/// (Task 18's strict closed-vocab DTO), NOT the frozen v1 `LlmTransactionRow`
/// — `amount_raw` is mapped from the band-column letter via
/// `crate::consensus::band_from_column`; closed-vocab enum fields render
/// via `enum_field_str` above. Silver SHAPE stays byte-identical to v1.
pub fn silver_rows(
    header: &ConsensusHeader,
    published: &[PublishedRow],
    doc_id: &str,
) -> anyhow::Result<Vec<StagingRow>> {
    anyhow::ensure!(
        !published.is_empty(),
        "consensus extraction produced zero published rows — fail closed (invariant 6)"
    );
    let mut rows = Vec::with_capacity(published.len());
    for candidate in published {
        let row: crate::consensus::LlmConsensusRow = serde_json::from_value(candidate.row.clone())
            .context("consensus-agreed row does not match the LlmConsensusRow shape — fail closed")?;
        let row_ordinal = candidate
            .ordinal0
            .checked_add(1)
            .context("row ordinal overflow")?;
        let silver = SilverRow {
            doc_id: doc_id.to_owned(),
            row_ordinal,
            filer_name_raw: header.filer_name_raw.clone(),
            filer_status_raw: header.filer_status_raw.clone(),
            state_district_raw: header.state_district_raw.clone(),
            row_id_raw: row.row_id_raw,
            owner_code_raw: row.owner_code_raw.as_ref().map(enum_field_str).transpose()?,
            asset_raw: row.asset_raw,
            asset_type_code_raw: row.asset_type_code_raw,
            transaction_type_raw: enum_field_str(&row.transaction_type_raw)?,
            transaction_date_raw: row.transaction_date_raw,
            notification_date_raw: row.notification_date_raw,
            amount_raw: crate::consensus::band_from_column(row.band_column).to_owned(),
            cap_gains_over_200: row.cap_gains_over_200,
            filing_status_raw: enum_field_str(&row.filing_status_raw)?,
            subholding_of_raw: row.subholding_of_raw,
            description_raw: row.description_raw,
            comments_raw: row.comments_raw,
            vehicle_owner_code_raw: row.vehicle_owner_code_raw,
            vehicle_location_raw: row.vehicle_location_raw,
            signed_date_raw: header.signed_date_raw.clone(),
            extractor: EXTRACTOR_LLM.to_owned(),
        };
        rows.push(StagingRow {
            payload: serde_json::to_value(&silver).context("serializing consensus silver payload")?,
            confidence: candidate.confidence,
        });
    }
    Ok(rows)
}
```

```rust
/// `…/ptr-pdfs/<year>/<DocID>.pdf` → `DocID` (4–8 digits, regime doc §2.2
/// shape). `pub` (goal 021 Phase 3 H36): the batch-poll path
/// (`worker::consensus_batch::poll_resolve_doc_id`) reuses this EXACT
/// mapping — the part that actually decides what a valid DocID looks like —
/// rather than re-deriving it.
pub fn doc_id_from_url(url: &str) -> Option<String> {
    let stem = url.rsplit('/').next()?.strip_suffix(".pdf")?;
    ((4..=8).contains(&stem.len()) && stem.bytes().all(|b| b.is_ascii_digit()))
        .then(|| stem.to_owned())
}
```

Update `extract_live`'s call site (the only caller): `let rows = to_staging_rows(&header, &outcome.published, &doc_id)?;` → `let rows = silver_rows(&header, &outcome.published, &doc_id)?;`. Rename the three Task-24 tests' calls (`staging_rows_thread_doc_id_and_stamp_header_plus_confidence`, `zero_published_rows_fail_closed`, `cached_rows_foreign_tag_or_garbage_payload_fail_closed`) from `to_staging_rows(...)` to `silver_rows(...)` — no other change.

In `crates/worker/src/consensus_batch.rs`, extend the existing `use pipeline::extraction::consensus::{...}` line to add `vote_header` and `PublishedRow`, and add:

```rust
use us_house::extractor::{ConsensusHeader, silver_rows};
```

Add the doc_id resolver and rewrite `resolve_document`:

```rust
/// Resolves the DocID for one document via THE SAME lookup the sync path's
/// `us_house::extractor::resolve_doc_id` uses (goal 021 Phase 3 H36) —
/// duplicated as a plain SQL query here because the sync fn takes a
/// `RunCtx` (bronze + clock + politeness client) this poll-only path has no
/// reason to construct. `doc_id_from_url` — the part that actually decides
/// what a valid DocID looks like — is reused verbatim (`pub`, exactly for
/// this cross-crate call), never re-derived.
///
/// # Errors
/// Database failure reading `raw_document.source_url`.
pub async fn poll_resolve_doc_id(pool: &PgPool, sha: &str) -> anyhow::Result<Option<String>> {
    let source_url: Option<Option<String>> =
        sqlx::query_scalar("select source_url from raw_document where sha256 = $1")
            .bind(sha)
            .fetch_optional(pool)
            .await
            .context("reading raw_document.source_url")?;
    Ok(source_url
        .flatten()
        .as_deref()
        .and_then(us_house::extractor::doc_id_from_url))
}
```

```rust
pub async fn resolve_document<T: Transport>(
    pool: &PgPool,
    transport: &T,
    bronze: &BronzeStore,
    sha: &str,
    regime_code: &str,
    consensus_tag: &str,
    composite_model_id: &str,
    escalation_model: &str,
    spec: &ConsensusSpec,
    preprocess_cfg: &PreprocessCfg,
) -> anyhow::Result<()> {
    let samples = load_samples(pool, sha, consensus_tag).await?;
    let sample_values: Vec<Value> = samples.iter().map(|s| s.payload.clone()).collect();
    let aligned = align(&sample_values, spec)?;
    let verdicts = score(&aligned, spec);

    let has_disputed = verdicts
        .iter()
        .any(|v| matches!(v, RowVerdict::Disputed { .. }));
    let escalation_row = if has_disputed {
        let doc_bytes = bronze.get(&RawDocRef {
            sha256: sha.to_owned(),
        })?;
        let preprocessed = preprocess_document(&doc_bytes, preprocess_cfg)?;
        Some(escalate(transport, escalation_model, &preprocessed.pages_png, &spec.tool).await?)
    } else {
        None
    };

    let mut published: Vec<PublishedRow> = Vec::new();
    for verdict in verdicts {
        match verdict {
            RowVerdict::Agreed { ordinal0, row } => {
                published.push(PublishedRow {
                    ordinal0,
                    row,
                    confidence: policy::CONF_AGREED,
                });
            }
            RowVerdict::Disputed {
                ordinal0,
                key,
                candidates,
                disputed_fields,
            } => {
                // Reuse the sync path's tiebreaker verbatim (Task 17/H29) —
                // same resolution, same CONF_ESCALATED, no batch/sync divergence.
                let resolved = escalation_row.as_ref().and_then(|premium| {
                    resolve_disputed(ordinal0, &key, &candidates, &disputed_fields, spec, premium)
                });
                match resolved {
                    Some(published_row) => published.push(published_row),
                    None => {
                        open_review_task_once(
                            pool,
                            "raw_document",
                            &format!("{regime_code}:{sha}"),
                            "consensus_row_hold",
                        )
                        .await?;
                    }
                }
            }
        }
    }

    if !published.is_empty() {
        // H36 (goal 021 Phase 3, review catch): route the poll-side outcome
        // through the EXACT SAME Silver-mapping fn the sync path uses
        // (`us_house::extractor::silver_rows`) — before this fix,
        // `resolve_document` cached the raw DTO `Value` rows verbatim, which
        // Task 24's `validated()` (SilverRow shape + extractor tag gate)
        // rejects on the very next cache read, freezing the document behind
        // a fail-closed error it could never recover from. Header identity
        // fields are majority-voted from the SAME sample payloads
        // `align`/`score` already consumed (`vote_header`, Task 13) — never
        // a second sampling pass, exactly mirroring the sync path's
        // `extract_live`.
        let header_value = vote_header(&sample_values, spec)?;
        let header: ConsensusHeader = serde_json::from_value(header_value)
            .context("consensus header does not match the ConsensusHeader shape")?;
        let doc_id = poll_resolve_doc_id(pool, sha).await?.with_context(|| {
            format!(
                "consensus-batch-poll: DocID unresolvable from raw_document.source_url \
                 for {sha} — freeze + review_task (invariant 6)"
            )
        })?;
        let rows = silver_rows(&header, &published, &doc_id)?;
        let key = CacheKey::new(sha, consensus_tag, composite_model_id);
        pg_put(
            pool,
            &key,
            &rows,
            &serde_json::json!({"extracted_by": composite_model_id, "consensus_tag": consensus_tag}),
        )
        .await?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test consensus_batch_poll -- --ignored`
Expected: PASS (`resolve_document_caches_silver_shaped_rows_and_is_idempotent` plus Task 23's existing `expired_items_land_in_resubmit_never_in_holds`).

Run: `cargo test -p pipeline` (Task 24's renamed `silver_rows` tests) and `cargo test -p worker`
Expected: PASS, no references to `to_staging_rows` remain.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/adapters/us_house/src/extractor.rs crates/worker/src/consensus_batch.rs crates/worker/tests/consensus_batch_poll.rs
git commit -m "$(cat <<'EOF'
fix(worker): batch poll routes through the shared Silver-mapping fn (goal 021 Phase 3 H36)

Review catch: the committed Task 23 resolve_document pg_put the raw
comparator DTO rows verbatim and never threaded doc_id/header, which
Task 24's validated() (SilverRow shape + extractor-tag gate) rejects on
the very next cache read — a batch-resolved document could never publish
through the adapter cache path.

Exposes us_house::extractor::to_staging_rows as pub fn silver_rows (same
body, renamed) plus pub ConsensusHeader and pub doc_id_from_url, and
rewires resolve_document to vote_header, resolve doc_id via the same
raw_document.source_url lookup the sync path uses, map through
silver_rows, then pg_put — so sync and batch extraction can never diverge
on how a consensus outcome becomes a cached Silver row again.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H37: Batch premium-trigger parity + escalation reuse guard (findings 15/16 batch)

**This task deliberately exceeds the ~1h guideline** — the premium-trigger parity fix spans
`consensus-batch-submit`'s per-doc signal persistence and `consensus_batch`'s `resolve_document`
reconstruction of `premium_needed` together; splitting would leave one side computing a
disjunction the other side never wrote. Do not split it.

**Precondition statement.** This task is a PRECONDITION of the first real batch run: the committed Task 23's own `resolve_document` doc comment records the gap explicitly ("Known gap... the sync path's programmatic sanity checks... are NOT applied on this batch-ingest path yet"), and the same gap extends to findings 15 (pixel-ambiguity premium trigger) and 16 (quality-routed vote sets) once H31/H32 land those primitives sync-side — without this task, the batch path's premium slot fires ONLY on a scored dispute, never on a flagged-but-unanimous document, and (independently) a re-poll of a document whose escalation already fired would refire it, breaking the one-premium-call-per-document invariant across poll boundaries.

**Files:**
- Modify: `crates/worker/src/consensus_batch.rs` (doc-level signal persist/load, escalation-pass find/persist, `batch_premium_needed`, rewire `resolve_document`)
- Modify: `crates/worker/src/bin/consensus-batch-submit.rs` (persist the quality/pixel signal per doc, right after preprocessing)
- Test: `crates/worker/tests/consensus_batch_poll.rs` (extend with the (a)/(b) pivotal tests, reusing H36's `test_spec`/`sample_payload`/`insert_raw_document`)

**Interfaces:**
- Consumes (Task 19's changeset — read `crates/core/migrations/0011_consensus_extraction.sql` first; if the actual table/column shape differs from below, adjust the SQL text and the two persist/load fn bodies only, not this task's call sites in `resolve_document`/`consensus-batch-submit.rs`). Doc-level quality/pixel signals do not fit either existing 0011 table cleanly: `extraction_sample` is per-PASS and has no rows yet at SUBMIT time (`record_batch_submitted` only writes `extraction_batch`; sample rows only exist after INGEST), and `extraction_batch` is per-BATCH-of-many-shas — so the changeset session plan ("0011 gains the batch quality/pixel signal columns now") is assumed here to have added its own doc-keyed table in the SAME migration file:
  ```sql
  create table extraction_doc_signal (
    document_sha256 text        not null,
    consensus_tag   text        not null,
    quality         jsonb       not null default '{}'::jsonb,
    pixel           jsonb       not null default '{}'::jsonb,
    created_at      timestamptz not null default now(),
    primary key (document_sha256, consensus_tag)
  );
  ```
- `pipeline::extraction::preprocess::{QualityMetrics, PreprocessOutput, doc_quality_flagged}` (H31) — read `crates/pipeline/src/extraction/preprocess.rs` first; `preprocess_document` now returns `PreprocessOutput { pages_png, quality }` per the shared plan contract.
- `pipeline::extraction::WATCHLIST_POLITICIANS` (existing, `crates/pipeline/src/extraction/mod.rs:40`, unmodified).
- Produces (this task, in `worker::consensus_batch`): `pub async fn persist_doc_signals(pool, document_sha256, consensus_tag, quality: &Value, pixel: &Value) -> anyhow::Result<()>`; `pub async fn load_doc_signals(pool, document_sha256, consensus_tag) -> anyhow::Result<Option<(Value, Value)>>`; `pub async fn find_escalation_pass(pool, document_sha256, consensus_tag, escalation_model) -> anyhow::Result<Option<Value>>`; `pub async fn persist_escalation_pass(pool, document_sha256, consensus_tag, pass_idx: i32, escalation_model, payload: &Value) -> anyhow::Result<()>`; `resolve_document` gains the premium_needed disjunction + reuse guard (same public signature as H36 left it).

- [ ] **Step 1: Write the failing test**

Add to `crates/worker/tests/consensus_batch_poll.rs`:

```rust
/// Returns a canned tool_use response and counts `send` calls (unlike H36's
/// `NeverCalledTransport`, this one is meant to be called) — proves the
/// escalation call count across `resolve_document` invocations: exactly one
/// when a premium call is needed, reused (not refired) on every re-poll.
struct EscalationCountingTransport {
    calls: AtomicUsize,
    response: Value,
}

impl EscalationCountingTransport {
    fn new(escalation_row: Value) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            response: json!({
                "content": [{"type": "tool_use", "name": "record_rows", "input": escalation_row}],
                "usage": {"input_tokens": 10, "output_tokens": 5},
            }),
        }
    }
}

#[async_trait]
impl Transport for EscalationCountingTransport {
    async fn send(&self, _body: &Value) -> anyhow::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.response.clone())
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn disputed_doc_escalates_exactly_once_across_repolls(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    const SHA: &str = "shaH37Disputed000000000000000000000000000000000000000000000";
    insert_raw_document(&pool, SHA, "9115900").await;

    record_batch_submitted(&pool, "msgbatch_h37a", "us_house", CONSENSUS_TAG, COMPOSITE_MODEL_ID, &[SHA.to_owned()])
        .await
        .unwrap();
    let results = vec![
        (format!("{SHA}:0"), succeeded(sample_payload("A"))),
        (format!("{SHA}:1"), succeeded(sample_payload("A"))),
        (format!("{SHA}:2"), succeeded(sample_payload("B"))), // disagrees -> Disputed
    ];
    ingest_batch_results(&pool, "msgbatch_h37a", results).await.unwrap();

    let bronze = BronzeStore::open(std::env::temp_dir().join("govfolio-h37a-test-bronze")).unwrap();
    let preprocess_cfg = PreprocessCfg { max_edge: 1568 };
    let spec = test_spec();
    let transport = EscalationCountingTransport::new(sample_payload("A"));

    worker::consensus_batch::resolve_document(
        &pool, &transport, &bronze, SHA, "us_house", CONSENSUS_TAG, COMPOSITE_MODEL_ID,
        "claude-sonnet-5", &spec, &preprocess_cfg,
    )
    .await
    .unwrap();
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1, "one dispute -> exactly one escalation call");

    // Re-poll the same document (e.g. the poll bin re-run before every
    // document in the batch reached 'ingested') — must reuse the persisted
    // pass, never a second premium call.
    worker::consensus_batch::resolve_document(
        &pool, &transport, &bronze, SHA, "us_house", CONSENSUS_TAG, COMPOSITE_MODEL_ID,
        "claude-sonnet-5", &spec, &preprocess_cfg,
    )
    .await
    .unwrap();
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        1,
        "re-poll must reuse the persisted escalation pass, never a second premium call per document"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn unflagged_undisputed_doc_never_escalates(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    const SHA: &str = "shaH37Clean0000000000000000000000000000000000000000000000000";
    insert_raw_document(&pool, SHA, "9115901").await;

    record_batch_submitted(&pool, "msgbatch_h37b", "us_house", CONSENSUS_TAG, COMPOSITE_MODEL_ID, &[SHA.to_owned()])
        .await
        .unwrap();
    let results = vec![
        (format!("{SHA}:0"), succeeded(sample_payload("A"))),
        (format!("{SHA}:1"), succeeded(sample_payload("A"))),
        (format!("{SHA}:2"), succeeded(sample_payload("A"))),
    ];
    ingest_batch_results(&pool, "msgbatch_h37b", results).await.unwrap();
    // Explicit "not flagged" signal, as consensus-batch-submit would have
    // persisted for a clean scan.
    worker::consensus_batch::persist_doc_signals(
        &pool, SHA, CONSENSUS_TAG,
        &json!({"flagged": false}),
        &json!({"ambiguous_any_row": false}),
    )
    .await
    .unwrap();

    let bronze = BronzeStore::open(std::env::temp_dir().join("govfolio-h37b-test-bronze")).unwrap();
    let preprocess_cfg = PreprocessCfg { max_edge: 1568 };
    let spec = test_spec();
    let transport = EscalationCountingTransport::new(sample_payload("A"));

    worker::consensus_batch::resolve_document(
        &pool, &transport, &bronze, SHA, "us_house", CONSENSUS_TAG, COMPOSITE_MODEL_ID,
        "claude-sonnet-5", &spec, &preprocess_cfg,
    )
    .await
    .unwrap();
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "no dispute, no quality/pixel flag, non-watchlist filer -> zero escalation calls"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test consensus_batch_poll -- --ignored`
Expected: FAIL to compile — `error[E0425]: cannot find function 'persist_doc_signals' in module 'worker::consensus_batch'` (this task's new test calls it directly; it does not exist until Step 3).

- [ ] **Step 3: Write minimal implementation**

Append to `crates/worker/src/consensus_batch.rs`:

```rust
/// Persists one document's quality-by-product + pixel-ambiguity signal
/// (goal 021 Phase 3 H37, findings 15/16 batch parity) — computed ONCE at
/// submit time (`consensus-batch-submit`, which already has `cfg` and the
/// preprocessed images in hand) and consulted at poll time, rather than
/// re-evaluated against whatever config is live when the poll bin happens
/// to run. Idempotent-by-overwrite: a resubmit of the same document under
/// the same consensus run replaces the signal with the freshly recomputed
/// one — this is a re-derivable fact about the (document, config) pair, not
/// an append-only observation.
///
/// # Errors
/// Database failure.
pub async fn persist_doc_signals(
    pool: &PgPool,
    document_sha256: &str,
    consensus_tag: &str,
    quality: &Value,
    pixel: &Value,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into extraction_doc_signal (document_sha256, consensus_tag, quality, pixel) \
         values ($1, $2, $3, $4) \
         on conflict (document_sha256, consensus_tag) do update \
           set quality = excluded.quality, pixel = excluded.pixel, created_at = now()",
    )
    .bind(document_sha256)
    .bind(consensus_tag)
    .bind(quality)
    .bind(pixel)
    .execute(pool)
    .await
    .context("persisting extraction_doc_signal row")?;
    Ok(())
}

/// Loads one document's persisted signal — `None` when submit never
/// recorded one (e.g. a document ingested by an older poll run before this
/// task landed). Callers must NOT fail closed on `None`; they must treat it
/// as "unknown, so flagged" (see `batch_premium_needed`) — the conservative
/// direction, never a silent skip of scrutiny.
///
/// # Errors
/// Database failure.
pub async fn load_doc_signals(
    pool: &PgPool,
    document_sha256: &str,
    consensus_tag: &str,
) -> anyhow::Result<Option<(Value, Value)>> {
    let row: Option<(Value, Value)> = sqlx::query_as(
        "select quality, pixel from extraction_doc_signal \
         where document_sha256 = $1 and consensus_tag = $2",
    )
    .bind(document_sha256)
    .bind(consensus_tag)
    .fetch_optional(pool)
    .await
    .context("loading extraction_doc_signal row")?;
    Ok(row)
}

/// Finds an already-persisted escalation pass for one document (goal 021
/// Phase 3 H37 — the one-premium-call-per-document invariant across
/// RE-POLLS, not just within a single poll invocation). `extraction_sample`
/// already reserves `pass_idx = n` (one past the last primary sample) for
/// "any escalation pass last" per Task 19's own migration comment; this
/// queries by `model_id` rather than a hardcoded index, so it is correct
/// regardless of `n`.
///
/// # Errors
/// Database failure.
pub async fn find_escalation_pass(
    pool: &PgPool,
    document_sha256: &str,
    consensus_tag: &str,
    escalation_model: &str,
) -> anyhow::Result<Option<Value>> {
    let payload: Option<Value> = sqlx::query_scalar(
        "select payload from extraction_sample \
         where document_sha256 = $1 and consensus_tag = $2 and model_id = $3 \
         order by pass_idx limit 1",
    )
    .bind(document_sha256)
    .bind(consensus_tag)
    .bind(escalation_model)
    .fetch_optional(pool)
    .await
    .context("querying extraction_sample for an existing escalation pass")?;
    Ok(payload)
}

/// Persists a freshly-fired escalation pass at `pass_idx = n` (one past the
/// last primary sample, matching `load_samples`'s `order by pass_idx`
/// convention). Same idempotent PK as every other `extraction_sample` row —
/// a crash between `escalate()` returning and this write just means the
/// NEXT poll re-fires `escalate()` once more (a rare, acceptable double
/// call); this write is what makes every SUBSEQUENT re-poll free.
///
/// # Errors
/// Database failure.
pub async fn persist_escalation_pass(
    pool: &PgPool,
    document_sha256: &str,
    consensus_tag: &str,
    pass_idx: i32,
    escalation_model: &str,
    payload: &Value,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into extraction_sample (document_sha256, consensus_tag, pass_idx, model_id, payload) \
         values ($1, $2, $3, $4, $5) \
         on conflict (document_sha256, consensus_tag, pass_idx) do nothing",
    )
    .bind(document_sha256)
    .bind(consensus_tag)
    .bind(pass_idx)
    .bind(escalation_model)
    .bind(payload)
    .execute(pool)
    .await
    .context("persisting the escalation pass into extraction_sample")?;
    Ok(())
}

/// The watchlist half of the §6.3 high-impact floor (goal 021 Phase 3 H37) —
/// self-contained, since `WATCHLIST_POLITICIANS` is already `pub` on
/// `pipeline::extraction`. The BAND-floor half needs H32's letter-aware band
/// predicate (Task 18's `band_column` enum replaces the string `amount_raw`
/// band the OLD `us_house::extractor::high_impact_rows` parses) — that is
/// H32's deliverable, not this task's; OR the real band-floor result into
/// this function once H32 lands (read `crates/adapters/us_house/src/
/// consensus.rs` first). Until then this is a documented, non-silent gap —
/// the batch path's premium trigger still has three other live terms
/// (quality, pixel, dispute).
fn watchlist_floor(header_value: &Value) -> bool {
    header_value
        .get("filer_name_raw")
        .and_then(Value::as_str)
        .is_some_and(|name| pipeline::extraction::WATCHLIST_POLITICIANS.contains(&name))
}

/// Reconstructs the SAME `premium_needed` disjunction H32's single sync
/// premium slot computes (goal 021 Phase 3 H37, findings 15/16 batch
/// parity), from what `consensus-batch-submit` persisted (quality + pixel,
/// frozen at submit-time config) plus what was just freshly scored (dispute)
/// plus the watchlist half of the §6.3 floor. A missing persisted signal
/// (`None` — an older batch, pre-H37) is treated as flagged: the
/// conservative direction, routing to review rather than silently
/// publishing an unscrutinized 0.90.
fn batch_premium_needed(signals: Option<&(Value, Value)>, has_disputed: bool, watchlist: bool) -> bool {
    let (quality_flagged, pixel_ambiguous) = match signals {
        Some((quality, pixel)) => (
            quality.get("flagged").and_then(Value::as_bool).unwrap_or(true),
            pixel
                .get("ambiguous_any_row")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        ),
        None => (true, true),
    };
    quality_flagged || pixel_ambiguous || has_disputed || watchlist
}
```

Rewrite `resolve_document` (replaces H36's version — the escalation gate becomes `premium_needed` instead of bare `has_disputed`, and consults/persists the escalation pass):

```rust
pub async fn resolve_document<T: Transport>(
    pool: &PgPool,
    transport: &T,
    bronze: &BronzeStore,
    sha: &str,
    regime_code: &str,
    consensus_tag: &str,
    composite_model_id: &str,
    escalation_model: &str,
    spec: &ConsensusSpec,
    preprocess_cfg: &PreprocessCfg,
) -> anyhow::Result<()> {
    let samples = load_samples(pool, sha, consensus_tag).await?;
    let sample_values: Vec<Value> = samples.iter().map(|s| s.payload.clone()).collect();
    let aligned = align(&sample_values, spec)?;
    let verdicts = score(&aligned, spec);
    let has_disputed = verdicts
        .iter()
        .any(|v| matches!(v, RowVerdict::Disputed { .. }));

    let header_value = vote_header(&sample_values, spec)?;

    // H37: the SAME premium_needed disjunction H32's sync single premium
    // slot computes, reconstructed from persisted signals + freshly-scored
    // disputes + the watchlist floor.
    let signals = load_doc_signals(pool, sha, consensus_tag).await?;
    let premium_needed = batch_premium_needed(signals.as_ref(), has_disputed, watchlist_floor(&header_value));

    let escalation_row = if premium_needed {
        // Consult BEFORE calling escalate() — never a second premium call
        // per document across re-polls.
        match find_escalation_pass(pool, sha, consensus_tag, escalation_model).await? {
            Some(payload) => Some(payload),
            None => {
                let doc_bytes = bronze.get(&RawDocRef {
                    sha256: sha.to_owned(),
                })?;
                let preprocessed = preprocess_document(&doc_bytes, preprocess_cfg)?;
                let payload = escalate(transport, escalation_model, &preprocessed.pages_png, &spec.tool).await?;
                let pass_idx = i32::try_from(samples.len()).context("escalation pass_idx overflow")?;
                persist_escalation_pass(pool, sha, consensus_tag, pass_idx, escalation_model, &payload).await?;
                Some(payload)
            }
        }
    } else {
        None
    };

    // Known gap (explicitly out of scope for this task, not a silent
    // divergence — this comment is the record): premium-concordance
    // downgrade of already-Agreed rows to CONF_SANITY_CAPPED on unanimous
    // rows where the premium dissents (H32 sync-side, finding 15) is not
    // yet mirrored on this batch-ingest path. This task's job is WHETHER
    // the premium fires and that it never refires — not yet what a fired,
    // concurring premium changes about already-agreed rows.
    let mut published: Vec<PublishedRow> = Vec::new();
    for verdict in verdicts {
        match verdict {
            RowVerdict::Agreed { ordinal0, row } => {
                published.push(PublishedRow {
                    ordinal0,
                    row,
                    confidence: policy::CONF_AGREED,
                });
            }
            RowVerdict::Disputed {
                ordinal0,
                key,
                candidates,
                disputed_fields,
            } => {
                let resolved = escalation_row.as_ref().and_then(|premium| {
                    resolve_disputed(ordinal0, &key, &candidates, &disputed_fields, spec, premium)
                });
                match resolved {
                    Some(published_row) => published.push(published_row),
                    None => {
                        open_review_task_once(
                            pool,
                            "raw_document",
                            &format!("{regime_code}:{sha}"),
                            "consensus_row_hold",
                        )
                        .await?;
                    }
                }
            }
        }
    }

    if !published.is_empty() {
        let header: ConsensusHeader = serde_json::from_value(header_value)
            .context("consensus header does not match the ConsensusHeader shape")?;
        let doc_id = poll_resolve_doc_id(pool, sha).await?.with_context(|| {
            format!(
                "consensus-batch-poll: DocID unresolvable from raw_document.source_url \
                 for {sha} — freeze + review_task (invariant 6)"
            )
        })?;
        let rows = silver_rows(&header, &published, &doc_id)?;
        let key = CacheKey::new(sha, consensus_tag, composite_model_id);
        pg_put(
            pool,
            &key,
            &rows,
            &serde_json::json!({"extracted_by": composite_model_id, "consensus_tag": consensus_tag}),
        )
        .await?;
    }
    Ok(())
}
```

In `crates/worker/src/bin/consensus-batch-submit.rs`: Task 22's committed preprocessing loop
discards `.quality` (`let preprocessed = preprocess_document(&bytes, &preprocess_cfg)...?;
page_counts.push(preprocessed.pages_png.len()); docs.push((sha.clone(), preprocessed.pages_png));`
— `docs` stays `Vec<(String, Vec<Vec<u8>>)>`, the shape `build_batch_requests`/`check_budget_gate`
already require). This task amends that SAME loop to also capture quality into a parallel,
sha-keyed vector:

```rust
// Inside Task 22's existing `for sha in &shas { .. }` loop, alongside the
// unchanged `page_counts.push(...)` / `docs.push(...)` lines:
doc_quality.push((sha.clone(), preprocessed.quality.clone()));
```

(with `let mut doc_quality: Vec<(String, Vec<pipeline::extraction::preprocess::QualityMetrics>)>
= Vec::with_capacity(shas.len());` declared alongside Task 22's existing `docs`/`page_counts`
declarations, before the loop.) After the loop and after `let composite_model_id = ...;` is
computed, persist each document's REAL quality signal — no placeholder:

```rust
for (sha, quality) in &doc_quality {
    let quality_flagged = pipeline::extraction::config::doc_quality_flagged(&cfg, quality);
    worker::consensus_batch::persist_doc_signals(
        &pool,
        sha,
        CONSENSUS_TAG,
        &serde_json::json!({"flagged": quality_flagged}),
        // H32/H35's pixel-ambiguity scan primitive is not this task's
        // deliverable — persisting `false` is conservative-safe (never
        // OVER-triggers premium by itself; `quality_flagged`/dispute/
        // watchlist still provide real scrutiny). Documented gap, not a
        // silent one.
        &serde_json::json!({"ambiguous_any_row": false}),
    )
    .await?;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test consensus_batch_poll -- --ignored`
Expected: PASS (4 tests total in this file: Task 23's `expired_items_land_in_resubmit_never_in_holds`, H36's idempotent-cache test, H37's two new tests — `disputed_doc_escalates_exactly_once_across_repolls` and `unflagged_undisputed_doc_never_escalates`).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/consensus_batch.rs crates/worker/src/bin/consensus-batch-submit.rs crates/worker/tests/consensus_batch_poll.rs
git commit -m "$(cat <<'EOF'
feat(worker): batch premium-trigger parity + escalation reuse guard (goal 021 Phase 3 H37)

Precondition of the first real batch run (recorded gap in the committed
Task 23 resolve_document doc comment): the batch poll path previously
fired its one premium slot ONLY on a scored dispute, never on a flagged-
but-unanimous document (findings 15/16), and had no guard against
refiring escalate() on a re-poll of a document whose premium pass had
already landed.

consensus-batch-submit now persists a per-doc quality/pixel signal
(extraction_doc_signal, doc-keyed — neither extraction_sample, which
has no rows until ingest, nor extraction_batch, which is per-batch-of-
many-shas, fits); resolve_document reconstructs the same premium_needed
disjunction from that signal plus freshly-scored disputes plus the
watchlist floor, and consults extraction_sample for an existing
model_id == escalation pass BEFORE calling escalate() — never a second
premium call per document.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H38: Confidence-lane-aware stratified audit + weighted precision report (finding 8)

**This task deliberately exceeds the ~1h guideline** — `select_sample_weighted` and
`weighted_precision` must land wired into `sample-audit`'s actual flow together, not merely
exported, per this task's own "ship together or the SLO number biases" rule; splitting would
leave the bin drawing samples the old unweighted way while a report elsewhere claims a weighted
precision it never measured. Do not split it.

**Finding 8.** The 0.90 LLM-consensus lane is the only lane no human reviews unconditionally (0.75/0.79 already ride the mandatory review_task path), and the existing uniform `sample_audit` draw gives it ~10 rows/month regardless of population size — statistically void for a lane-specific SLO. This task adds `[audit]` config sampling weights keyed (extraction-path prefix, confidence), keeps `worker::sampler::select_sample`'s deterministic seeded-hash draw mechanism UNCHANGED as the inner per-stratum draw, adds a nonzero deterministic-lane floor, and ships the inclusion-probability-weighted (Horvitz–Thompson/Hájek) precision estimator IN THE SAME task as the weights — "ship together or the SLO number biases" (an unweighted mean over a non-uniform draw silently drifts toward whichever stratum was oversampled).

**Files:**
- Modify: `crates/worker/src/sampler.rs` (`WeightRule`, `WeightedRecord`, `record_weight`, `WeightedDraw`, `select_sample_weighted`, `weighted_precision`)
- Modify: `crates/worker/src/bin/sample-audit.rs` (loads `[audit]` weights when configured, calls the weighted draw)
- Test: `crates/worker/src/sampler.rs` (inline `#[cfg(test)]`, extending the existing module)

**Interfaces:**
- Consumes: `worker::sampler::select_sample(record_ids: &[String], n: usize, seed: u64) -> Vec<String>` (existing, `crates/worker/src/sampler.rs:24-36`, UNCHANGED — reused verbatim as the per-stratum draw). `pipeline::extraction::config::ExtractorConfig` (H31/H32's task — read `crates/pipeline/src/extraction/config.rs` first): this task ASSUMES an `[audit]` table shaped `AuditConfig { weights: Vec<{ path: String, confidence: f32, weight: f32 }>, deterministic_floor: u32, exclude_shas: Vec<String> }` per the shared plan contract; if the real field names differ, adjust only `sample-audit.rs`'s config-loading glue (the `.map()` from `cfg.audit.weights` into `worker::sampler::WeightRule`), not `sampler.rs`'s math.
- Produces: `pub struct WeightRule { pub path: String, pub confidence: f32, pub weight: f32 }`, `pub struct WeightedRecord { pub record_id: String, pub extracted_by: String, pub extraction_confidence: Option<f32> }`, `pub fn record_weight(record: &WeightedRecord, weights: &[WeightRule]) -> f32`, `pub struct WeightedDraw { pub selected: Vec<String>, pub inclusion_prob: std::collections::HashMap<String, f64> }`, `pub fn select_sample_weighted(records: &[WeightedRecord], base_n: usize, seed: u64, weights: &[WeightRule], deterministic_floor: u32) -> WeightedDraw`, `pub fn weighted_precision(observations: &[(bool, f64)]) -> Option<f64>` in `worker::sampler`.

- [ ] **Step 1: Write the failing test**

Add to `crates/worker/src/sampler.rs`'s existing `#[cfg(test)] mod tests` block:

```rust
fn weighted_ids(prefix: &str, n: usize) -> Vec<WeightedRecord> {
    (0..n)
        .map(|i| WeightedRecord {
            record_id: format!("{prefix}-{i:04}"),
            extracted_by: "us_house_ptr/text@1".to_owned(),
            extraction_confidence: None,
        })
        .collect()
}

fn llm_records(n: usize) -> Vec<WeightedRecord> {
    (0..n)
        .map(|i| WeightedRecord {
            record_id: format!("llm-{i:04}"),
            extracted_by: "us_house_ptr/consensus@1".to_owned(),
            extraction_confidence: Some(0.9),
        })
        .collect()
}

#[test]
fn weighted_draw_oversamples_the_llm_0_90_stratum_tenfold() {
    let weights = vec![WeightRule {
        path: "us_house_ptr/consensus".to_owned(),
        confidence: 0.9,
        weight: 10.0,
    }];
    let mut records = weighted_ids("base", 900);
    records.extend(llm_records(100));

    // share_base = (1*900)/(1*900+10*100) = 900/1900; * 190 = 90 exactly.
    // share_llm  = (10*100)/1900 * 190 = 100 exactly (the whole bucket).
    let draw = select_sample_weighted(&records, 190, 42, &weights, 5);
    let base_drawn = draw.selected.iter().filter(|id| id.starts_with("base-")).count();
    let llm_drawn = draw.selected.iter().filter(|id| id.starts_with("llm-")).count();
    assert_eq!(base_drawn, 90);
    assert_eq!(llm_drawn, 100);

    let base_pi = draw.inclusion_prob["base-0000"];
    let llm_pi = draw.inclusion_prob["llm-0000"];
    assert!(
        (llm_pi / base_pi - 10.0).abs() < 1e-9,
        "llm-0.90 records are exactly ~10x as likely to be drawn: {llm_pi} vs {base_pi}"
    );
}

#[test]
fn weighted_draw_is_deterministic_across_repeated_calls() {
    let weights = vec![WeightRule {
        path: "us_house_ptr/consensus".to_owned(),
        confidence: 0.9,
        weight: 10.0,
    }];
    let mut records = weighted_ids("base", 40);
    records.extend(llm_records(10));

    let a = select_sample_weighted(&records, 20, 7, &weights, 2);
    let b = select_sample_weighted(&records, 20, 7, &weights, 2);
    assert_eq!(a.selected, b.selected, "same seed -> identical draw");
}

#[test]
fn weighted_precision_reproduces_the_true_rate_while_naive_is_biased() {
    // Bucket A (baseline, weight 1.0): 900 population, drawn 90 (pi=0.1),
    // true rate 99% correct -> 89 correct + 1 wrong observed.
    // Bucket B (llm-0.90, weight 10): 100 population, drawn 100 -- the
    // WHOLE bucket (pi=1.0), true rate 80% correct -> exactly 80 correct +
    // 20 wrong observed (fully observed, so the sample IS the population).
    // True population precision: (900*0.99 + 100*0.80) / 1000 = 0.971.
    let mut observations = Vec::new();
    observations.extend(std::iter::repeat_n((true, 0.1), 89));
    observations.push((false, 0.1));
    observations.extend(std::iter::repeat_n((true, 1.0), 80));
    observations.extend(std::iter::repeat_n((false, 1.0), 20));

    let weighted = weighted_precision(&observations).unwrap();
    assert!(
        (weighted - 0.971).abs() < 0.01,
        "weighted estimate {weighted} should track the true 0.971 rate"
    );

    let correct = observations.iter().filter(|(c, _)| *c).count();
    let naive = correct as f64 / observations.len() as f64;
    assert!(
        (naive - 0.971).abs() > 0.05,
        "the naive unweighted mean ({naive}) must NOT track the true rate -- it \
         overweights the fully-observed, error-heavy bucket B relative to the \
         under-sampled bucket A: {naive} vs true 0.971"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker sampler`
Expected: FAIL to compile — `WeightRule`, `WeightedRecord`, `select_sample_weighted`, `weighted_precision` do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add to `crates/worker/src/sampler.rs`:

```rust
/// One `[audit] weights` rule (goal 021 Phase 3 H38, finding 8): oversample
/// (or undersample) a stratum keyed by an extractor-tag PREFIX plus an EXACT
/// confidence bit-match — mirrors the closed policy_v1 set, never a
/// threshold. `weight = 1.0` is the uniform baseline.
#[derive(Debug, Clone)]
pub struct WeightRule {
    pub path: String,
    pub confidence: f32,
    pub weight: f32,
}

/// One record's audit-weight inputs, read alongside its id
/// (`disclosure_record.extracted_by` / `extraction_confidence`).
#[derive(Debug, Clone)]
pub struct WeightedRecord {
    pub record_id: String,
    pub extracted_by: String,
    pub extraction_confidence: Option<f32>,
}

/// Resolves one record's sampling weight: the FIRST `[audit] weights` rule
/// whose `path` is a PREFIX of `extracted_by` and whose `confidence` is an
/// EXACT bitwise match to the record's `extraction_confidence`; `1.0` (the
/// uniform baseline) when nothing matches or the record has no recorded
/// confidence (deterministic parsers never carry one).
#[must_use]
pub fn record_weight(record: &WeightedRecord, weights: &[WeightRule]) -> f32 {
    let Some(confidence) = record.extraction_confidence else {
        return 1.0;
    };
    weights
        .iter()
        .find(|rule| {
            record.extracted_by.starts_with(&rule.path) && rule.confidence.to_bits() == confidence.to_bits()
        })
        .map_or(1.0, |rule| rule.weight)
}

/// One weighted draw's result: every selected id, plus `record_id -> pi`
/// (exact inclusion probability) for every record CONSIDERED — the
/// precision report needs `pi` for the drawn subset.
#[derive(Debug, Clone, Default)]
pub struct WeightedDraw {
    pub selected: Vec<String>,
    pub inclusion_prob: std::collections::HashMap<String, f64>,
}

/// Deterministic, seeded, WEIGHTED stratified draw (goal 021 Phase 3 H38,
/// finding 8): buckets `records` by their resolved [`record_weight`], then
/// allocates `base_n` draws across buckets proportional to `weight *
/// bucket_size`, drawing each bucket with the SAME [`select_sample`]
/// hash-rank mechanism — preserves the existing idempotent-draw property
/// (same `(bucket members, k, seed)` always yields the same sub-draw), and
/// every record in a bucket shares the SAME exact inclusion probability
/// `pi = k_bucket / bucket_size` (simple random sampling without
/// replacement WITHIN each bucket). The HIGHEST-weight bucket is floored at
/// `deterministic_floor` records — "a nonzero deterministic-lane floor"
/// (finding 8) — never starved to zero by a small population or low
/// proportional share.
#[must_use]
pub fn select_sample_weighted(
    records: &[WeightedRecord],
    base_n: usize,
    seed: u64,
    weights: &[WeightRule],
    deterministic_floor: u32,
) -> WeightedDraw {
    let mut buckets: std::collections::BTreeMap<u32, Vec<&WeightedRecord>> =
        std::collections::BTreeMap::new();
    for record in records {
        buckets
            .entry(record_weight(record, weights).to_bits())
            .or_default()
            .push(record);
    }
    let total_weighted: f64 = buckets
        .iter()
        .map(|(w, recs)| f64::from(f32::from_bits(*w)) * recs.len() as f64)
        .sum();
    let highest_weight_bits = buckets
        .keys()
        .copied()
        .max_by(|a, b| f32::from_bits(*a).total_cmp(&f32::from_bits(*b)));

    let mut draw = WeightedDraw::default();
    for (weight_bits, members) in &buckets {
        let weight = f64::from(f32::from_bits(*weight_bits));
        let share = if total_weighted > 0.0 {
            weight * members.len() as f64 / total_weighted
        } else {
            0.0
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut k = (share * base_n as f64).round() as usize;
        if Some(*weight_bits) == highest_weight_bits {
            k = k.max(deterministic_floor as usize);
        }
        k = k.min(members.len());

        let ids: Vec<String> = members.iter().map(|r| r.record_id.clone()).collect();
        let pi = if ids.is_empty() { 0.0 } else { k as f64 / ids.len() as f64 };
        for id in &ids {
            draw.inclusion_prob.insert(id.clone(), pi);
        }
        draw.selected.extend(select_sample(&ids, k, seed));
    }
    draw
}

/// Inclusion-probability-weighted (Horvitz–Thompson/Hájek ratio) precision
/// estimate: `sum(correct_i / pi_i) / sum(1 / pi_i)`. MUST ship together
/// with [`select_sample_weighted`] (finding 8: "ship together or the SLO
/// number biases") — an unweighted mean over a non-uniform draw silently
/// drifts toward whichever stratum was oversampled.
#[must_use]
pub fn weighted_precision(observations: &[(bool, f64)]) -> Option<f64> {
    if observations.is_empty() {
        return None;
    }
    let (correct_weight, total_weight) = observations.iter().fold(
        (0.0_f64, 0.0_f64),
        |(correct, total), &(is_correct, pi)| {
            if pi <= 0.0 {
                return (correct, total);
            }
            let inverse = 1.0 / pi;
            (correct + if is_correct { inverse } else { 0.0 }, total + inverse)
        },
    );
    if total_weight <= 0.0 {
        None
    } else {
        Some(correct_weight / total_weight)
    }
}
```

Wire `crates/worker/src/bin/sample-audit.rs` to USE the weighted draw + weighted precision —
per finding 8's own "ship together" rule, exporting the two functions without wiring them into
the bin's ACTUAL flow does not satisfy this task. Read the existing bin's population-fetch,
uniform `select_sample` call, and precision-computation/report-print call sites first (variable
names below are illustrative — adjust to match, the STRUCTURE must not change): when `[audit]`
weights are configured, `select_sample_weighted` REPLACES the uniform draw (not a parallel,
unused path) and `weighted_precision` REPLACES the naive mean the report line prints; when
unconfigured, behavior is byte-identical to before this task (uniform draw, naive precision):

```rust
// Replaces the bin's existing uniform-draw + naive-precision block.
// extractor.toml's [audit].exclude_shas (the 9115811 calibration-exclusion
// sha, finding 5) is consulted by whatever draws the LLM-lane population
// INTO sample_audit in the first place (a caller-supplied record_id
// filter), not by select_sample_weighted itself, which only sees whatever
// WeightedRecord list it is handed.
let cfg = pipeline::extraction::config::ExtractorConfig::load()?;
let (selected_ids, precision): (Vec<String>, Option<f64>) = if cfg.audit.weights.is_empty() {
    // Unchanged pre-H38 behavior: uniform draw, naive (unweighted) mean.
    let selected = worker::sampler::select_sample(&record_ids, base_n, seed);
    let precision = naive_precision(&observations_for(&selected)); // existing bin helper
    (selected, precision)
} else {
    let weights: Vec<worker::sampler::WeightRule> = cfg
        .audit
        .weights
        .iter()
        .map(|w| worker::sampler::WeightRule {
            path: w.path.clone(),
            confidence: w.confidence,
            weight: w.weight,
        })
        .collect();
    let draw = worker::sampler::select_sample_weighted(
        &weighted_records, // existing bin's population, WeightedRecord-shaped
        base_n,
        seed,
        &weights,
        cfg.audit.deterministic_floor,
    );
    let observations: Vec<(bool, f64)> = draw
        .selected
        .iter()
        .map(|id| (is_correct(id), draw.inclusion_prob[id])) // existing bin's correctness check
        .collect();
    (draw.selected, worker::sampler::weighted_precision(&observations))
};

println!(
    "sample-audit: drew {} record(s){} — precision: {}",
    selected_ids.len(),
    if cfg.audit.weights.is_empty() { "" } else { " (confidence-lane-weighted, Horvitz-Thompson)" },
    precision.map_or_else(|| "n/a (no observations)".to_owned(), |p| format!("{p:.3}")),
);
```

**Invocation verification (A8 "ship together" — the bin USES the weighted path, not merely
exports it):** run the bin twice against a temp `config/extractor.toml` copy, once with
`[audit]` weights absent and once with a weights table set, over the same fixture population,
and confirm the printed record COUNT differs between runs in the direction the weight predicts
(the LLM-0.90 stratum drawn ~10x as often per the pivotal unit test's own ratio) and that the
"(confidence-lane-weighted, Horvitz-Thompson)" suffix appears ONLY in the weighted run:

```bash
cargo run -p worker --bin sample-audit  # [audit] absent -> uniform draw, no suffix
GOVFOLIO_EXTRACTOR_CONFIG=/tmp/extractor-with-audit-weights.toml cargo run -p worker --bin sample-audit  # weighted draw, suffix present
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker sampler`
Expected: PASS (7 tests: the 4 existing `sampler_select_*`/`precision_estimate_math` plus the 3 new weighted-draw/precision tests).

Run the invocation verification above.
Expected: the two runs' printed record counts and precision figures differ as predicted; the
weighted-run line carries the `(confidence-lane-weighted, Horvitz-Thompson)` suffix, the
unweighted run's does not.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/sampler.rs crates/worker/src/bin/sample-audit.rs
git commit -m "$(cat <<'EOF'
feat(worker): confidence-lane-weighted audit sampling + Horvitz-Thompson precision (goal 021 Phase 3 H38)

Finding 8: the 0.90 LLM-consensus lane is the only lane no human reviews
unconditionally, and the uniform sample_audit draw gives it ~10 rows/month
regardless of population size -- statistically void for a lane-specific
SLO.

Adds [audit]-weight-keyed (extraction-path prefix, confidence) stratified
draw allocation over the SAME deterministic seeded-hash select_sample
mechanism (idempotent-draw property preserved, per-stratum), with a
nonzero deterministic-lane floor for the highest-weight bucket, and ships
the inclusion-probability-weighted (Horvitz-Thompson/Hajek) precision
estimator in the SAME commit -- an unweighted mean over this now-
non-uniform draw would silently drift toward whichever stratum got
oversampled.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H39: Migration 0012 + `Verdict::Edit` field-diff labels (finding 9a/9b)

**Finding 9a/9b.** Every reviewer `Verdict::Edit` resolution is FREE ground truth about extraction errors — the 0.75/0.79 confidence lanes are 100% reviewed via the mandatory-review `review_task` path, a far richer discrepancy signal than the once-a-month uniform `sample_audit` draw alone. This task (a) adds an expand-only migration `0012_audit_labels.sql` with `discrepancy_fields jsonb` + a closed-vocabulary `error_class` on `sample_audit`, and (b) wires a mechanical field-diff into `pipeline::promote`'s `Verdict::Edit` resolution site (`crates/pipeline/src/promote.rs:259-270`, found via `grep -rn "Verdict::Edit" crates/`) that records both, keyed (via a join to `disclosure_record.extracted_by`, no new column needed for that) whenever an edit fires — not only when the record happened to fall in a monthly draw.

**Files:**
- Create: `crates/core/migrations/0012_audit_labels.sql`
- Modify: `crates/core/tests/migrate.rs`
- Modify: `crates/pipeline/src/promote.rs` (`OriginalRecord` gains `details`; the SELECT that builds it; the `Verdict::Edit` match arm; two new helpers + inline unit tests)
- Test: `crates/pipeline/src/promote.rs` (inline `#[cfg(test)] mod tests`, new — the file has none today); `crates/pipeline/tests/promote.rs` (new db-gated integration test)

**Interfaces:**
- Consumes: `pipeline::promote::{Verdict, OriginalRecord, apply_resolution, supersede}` (existing, `crates/pipeline/src/promote.rs`, this task's own edit target); `govfolio_core::domain::gold::GoldCandidate { details: serde_json::Value, .. }` (existing, `crates/core/src/domain/gold.rs:17-52`, unmodified); the `(us_house, transaction)` details contract's real field name is `amount_band_raw` (`crates/adapters/us_house/src/details.rs:37`) — the goal text's illustrative example says `/amount_raw` (the Silver-stage field name); this task's pivotal test uses the verified Gold-stage `details` field name instead, per the plan's own "adjust names, not logic" convention.
- Produces: `crates/core/migrations/0012_audit_labels.sql` (`sample_audit.discrepancy_fields`, `sample_audit.error_class`); `fn diff_details(original: &Value, corrected: &Value) -> Vec<String>`, `fn classify_error(discrepancy_fields: &[String], original: &Value, corrected: &Value) -> &'static str`, `async fn record_edit_ground_truth(tx, original: &OriginalRecord, discrepancy_fields: &[String], error_class: &'static str) -> anyhow::Result<()>` in `pipeline::promote` (all `pub(crate)`/private — no cross-crate consumer yet).

- [ ] **Step 1: Write the failing test**

Modify `crates/core/tests/migrate.rs` (the assertion Task 19 bumped to 12 for its one new migration file):

```rust
//! DB-touching suite: gated behind `--ignored` (CI db job / local postgres on 5433).
#![allow(clippy::unwrap_used)]

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn migrator_is_idempotent(pool: sqlx::PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    govfolio_core::db::migrate(&pool).await.unwrap(); // second run: no-op, no error
    let n: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 13); // 0000_init + 0001_core + 0002_silver_us_house + 0003_registry_columns
                        // + 0004_extraction_cache + 0005_alerts + 0006_review_audit
                        // + 0007_productization + 0008_sentinel_watch + 0009_sample_audit
                        // + 0010_silver_br + 0011_consensus_extraction + 0012_audit_labels
}
```

Add to `crates/pipeline/src/promote.rs` a NEW `#[cfg(test)] mod tests` block (the file has no inline tests today — all its coverage lives in `crates/pipeline/tests/promote.rs`; the pure diff/classify functions below are unit-tested here instead, since they need no database):

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn diff_details_lists_only_changed_top_level_keys_sorted() {
        let original = json!({"amount_band_raw": "$1,001 - $15,000", "comments": "Sold.", "signed_date": "2026-01-07"});
        let corrected = json!({"amount_band_raw": "$15,001 - $50,000", "comments": "Sold at a loss.", "signed_date": "2026-01-07"});
        assert_eq!(
            diff_details(&original, &corrected),
            vec!["/amount_band_raw".to_owned(), "/comments".to_owned()],
            "multi-field edit -> all changed pointers listed, unchanged fields excluded"
        );
    }

    #[test]
    fn diff_details_empty_when_nothing_changed() {
        let same = json!({"amount_band_raw": "$1,001 - $15,000"});
        assert!(diff_details(&same, &same).is_empty());
    }

    #[test]
    fn classify_error_band_only_is_band_misread() {
        let fields = vec!["/amount_band_raw".to_owned()];
        let original = json!({"amount_band_raw": "$1,001 - $15,000"});
        let corrected = json!({"amount_band_raw": "$15,001 - $50,000"});
        assert_eq!(classify_error(&fields, &original, &corrected), "band_misread");
    }

    #[test]
    fn classify_error_multi_field_falls_back_to_other() {
        let fields = vec!["/amount_band_raw".to_owned(), "/comments".to_owned()];
        let original = json!({"amount_band_raw": "$1,001 - $15,000", "comments": "Sold."});
        let corrected = json!({"amount_band_raw": "$15,001 - $50,000", "comments": "Sold at a loss."});
        assert_eq!(classify_error(&fields, &original, &corrected), "other");
    }

    #[test]
    fn classify_error_owner_null_to_value_is_hallucinated() {
        let fields = vec!["/owner_source".to_owned()];
        let original = json!({"owner_source": null});
        let corrected = json!({"owner_source": "row"});
        assert_eq!(classify_error(&fields, &original, &corrected), "owner_hallucinated");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p core --test migrate -- --ignored`
Expected: FAIL — `assertion \`left == right\` failed / left: 12, right: 13` (`0012_audit_labels.sql` does not exist yet).

Run: `cargo test -p pipeline promote`
Expected: FAIL to compile — `diff_details`/`classify_error` do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Create `crates/core/migrations/0012_audit_labels.sql`:

```sql
-- 0012_audit_labels: mechanical error-diff labels on sample_audit (goal 021
-- Phase 3 hardening H39, finding 9a/9b). Expand-only.
--
-- Every reviewer Verdict::Edit resolution (crates/pipeline/src/promote.rs)
-- is FREE ground truth about extraction errors -- the 0.75/0.79 confidence
-- lanes are 100% reviewed via the mandatory-review review_task path, a much
-- richer signal than the once-a-month uniform sample_audit draw alone. This
-- migration adds two columns capturing that signal on the SAME sample_audit
-- table (report queries join one place): `discrepancy_fields` records WHICH
-- row-relative JSON pointers of `details` the reviewer changed;
-- `error_class` classifies WHY, from a closed vocabulary (mechanical
-- classification -- pipeline::promote::classify_error -- never a free-text
-- guess).
--
-- Edit-driven rows (not part of any monthly stratified draw) use the
-- sentinel `sample_month = 'review'` / `seed = 0`, distinguishing them from
-- real 'YYYY-MM' draws while reusing the SAME `unique (sample_month,
-- record_id)` idempotency the monthly sampler already relies on.

alter table sample_audit add column discrepancy_fields jsonb;
alter table sample_audit add column error_class text
  check (error_class is null or error_class in (
    'band_misread', 'type_misread', 'date_swap', 'owner_hallucinated',
    'asset_garbled', 'row_missed', 'row_invented', 'clerk_stamp_misparse', 'other'
  ));
```

Run `sh scripts/check-migration-safety.sh`
Expected: PASS — prints `migrations expand-only: safe to auto-apply` (only `add column`, no `drop`/`truncate`/`alter ... drop`).

In `crates/pipeline/src/promote.rs`, add `details: serde_json::Value` to `OriginalRecord` and fetch it in `apply_resolution`:

```rust
/// The original row's identity, read once and pinned onto any correction.
struct OriginalRecord {
    id: String,
    filing_id: String,
    politician_id: String,
    regime_id: String,
    details: serde_json::Value,
}
```

```rust
    let original: Option<(String, String, String, serde_json::Value)> = sqlx::query_as(
        "select filing_id, politician_id, regime_id, details from disclosure_record where id = $1",
    )
    .bind(&target_id)
    .fetch_optional(&mut *tx)
    .await
    .with_context(|| format!("loading target record {target_id}"))?;
    let Some((filing_id, politician_id, regime_id, details)) = original else {
        anyhow::bail!("review_task {task_id} targets missing record {target_id} — fail closed");
    };
    let original = OriginalRecord {
        id: target_id,
        filing_id,
        politician_id,
        regime_id,
        details,
    };
```

Update the `Verdict::Edit` match arm to compute the diff and record it (order matters: `supersede` first, so a contract-violating edit rolls back BEFORE any audit row is written — atomicity, invariant 5):

```rust
        Verdict::Edit {
            regime_code,
            corrected,
        } => {
            let superseding = supersede(&mut tx, &original, &regime_code, &corrected).await?;
            let discrepancy_fields = diff_details(&original.details, &corrected.details);
            let error_class = classify_error(&discrepancy_fields, &original.details, &corrected.details);
            record_edit_ground_truth(&mut tx, &original, &discrepancy_fields, error_class).await?;
            let resolution = json!({
                "verdict": "edit",
                "superseding_record_id": superseding.record_id,
                "fingerprint": superseding.fingerprint,
            });
            (Some(superseding), resolution)
        }
```

Add the three new functions (place after `supersede`):

```rust
/// Mechanical field-diff between an original and corrected `details` payload
/// (goal 021 Phase 3 H39, finding 9a): row-relative JSON pointers (`/<key>`)
/// of every TOP-LEVEL key whose value differs, sorted. Both `details`
/// payloads are flat, contract-typed objects (invariant 5) — no recursion
/// needed.
fn diff_details(original: &serde_json::Value, corrected: &serde_json::Value) -> Vec<String> {
    let (Some(before), Some(after)) = (original.as_object(), corrected.as_object()) else {
        return Vec::new();
    };
    let mut keys: std::collections::BTreeSet<&String> = before.keys().collect();
    keys.extend(after.keys());
    keys.into_iter()
        .filter(|key| {
            before.get(*key).unwrap_or(&serde_json::Value::Null)
                != after.get(*key).unwrap_or(&serde_json::Value::Null)
        })
        .map(|key| format!("/{key}"))
        .collect()
}

/// Mechanical classification of an edit's `discrepancy_fields` into the
/// closed `error_class` vocabulary (goal 021 Phase 3 H39, finding 9b) — a
/// deterministic, substring-on-field-name rule table (portable across
/// regimes' varying exact `details` field names), never a free-text guess.
/// Multi-field edits classify `"other"` UNLESS they are exactly the
/// two-field date-swap shape (`before.a == after.b && before.b == after.a`).
fn classify_error(
    discrepancy_fields: &[String],
    original: &serde_json::Value,
    corrected: &serde_json::Value,
) -> &'static str {
    if let [field] = discrepancy_fields {
        let name = field.trim_start_matches('/');
        return if name.contains("signed") {
            "clerk_stamp_misparse"
        } else if name.contains("band") {
            "band_misread"
        } else if name.contains("type") {
            "type_misread"
        } else if name.contains("asset") {
            "asset_garbled"
        } else if name.contains("owner") {
            let before = original.pointer(field);
            if before.is_none_or(serde_json::Value::is_null) {
                "owner_hallucinated"
            } else {
                "other"
            }
        } else {
            "other"
        };
    }
    let dates: Vec<&String> = discrepancy_fields
        .iter()
        .filter(|f| f.trim_start_matches('/').contains("date"))
        .collect();
    if discrepancy_fields.len() == 2 && dates.len() == 2 {
        let (a, b) = (dates[0].as_str(), dates[1].as_str());
        if original.pointer(a) == corrected.pointer(b) && original.pointer(b) == corrected.pointer(a) {
            return "date_swap";
        }
    }
    "other"
}

/// Records a `Verdict::Edit` resolution as `sample_audit` ground truth (goal
/// 021 Phase 3 H39, finding 9a): the 0.75/0.79 confidence lanes are 100%
/// reviewed via `review_task`, a far richer discrepancy signal than the
/// once-a-month stratified draw alone. Uses the sentinel `sample_month =
/// 'review'` / `seed = 0` (never a real `'YYYY-MM'` label) so this insert
/// can never collide with — or be mistaken for — a monthly `sample_audit`
/// draw row, while reusing its `unique (sample_month, record_id)`
/// idempotency.
///
/// # Errors
/// Database failure.
async fn record_edit_ground_truth(
    tx: &mut Transaction<'_, Postgres>,
    original: &OriginalRecord,
    discrepancy_fields: &[String],
    error_class: &'static str,
) -> anyhow::Result<()> {
    const REVIEW_SAMPLE_MONTH: &str = "review";
    const REVIEW_SEED: i64 = 0;
    sqlx::query(
        "insert into sample_audit \
           (id, regime_id, record_id, sample_month, seed, status, discrepancy_fields, error_class, audited_at) \
         values ($1, $2, $3, $4, $5, 'discrepancy', $6, $7, now()) \
         on conflict (sample_month, record_id) do update set \
           status = 'discrepancy', discrepancy_fields = excluded.discrepancy_fields, \
           error_class = excluded.error_class, audited_at = now()",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&original.regime_id)
    .bind(&original.id)
    .bind(REVIEW_SAMPLE_MONTH)
    .bind(REVIEW_SEED)
    .bind(serde_json::to_value(discrepancy_fields).context("serializing discrepancy_fields")?)
    .bind(error_class)
    .execute(&mut **tx)
    .await
    .context("recording edit ground truth in sample_audit")?;
    Ok(())
}
```

Add the db-gated integration test to `crates/pipeline/tests/promote.rs` (reuses `seed_via_pipeline`, `edit_verdict`, `corrected_details` already in this file — `corrected_details()`'s own doc comment: "the amount band was mis-extracted one band too low; everything else re-attested as filed", i.e. this fixture's ONLY real diff against the published row is `amount_band_raw`):

```rust
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn edit_resolution_captures_a_band_only_discrepancy_in_sample_audit(pool: PgPool) {
    let (task_id, record_id) = seed_via_pipeline(&pool, "band-discrepancy").await;

    resolve_review_task(&pool, &task_id, edit_verdict(corrected_details()), None)
        .await
        .unwrap();

    let (fields, class, status): (serde_json::Value, String, String) = sqlx::query_as(
        "select discrepancy_fields, error_class, status from sample_audit \
         where sample_month = 'review' and record_id = $1",
    )
    .bind(&record_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fields, serde_json::json!(["/amount_band_raw"]));
    assert_eq!(class, "band_misread");
    assert_eq!(status, "discrepancy");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p core --test migrate -- --ignored`
Expected: PASS (count is now 13).

Run: `cargo test -p pipeline promote`
Expected: PASS (5 inline unit tests).

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p pipeline --test promote -- --ignored`
Expected: PASS (the existing 4 promote.rs integration tests plus the new `edit_resolution_captures_a_band_only_discrepancy_in_sample_audit`).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/core/migrations/0012_audit_labels.sql crates/core/tests/migrate.rs crates/pipeline/src/promote.rs crates/pipeline/tests/promote.rs
git commit -m "$(cat <<'EOF'
feat(pipeline): migration 0012 + mechanical Verdict::Edit field-diff labels (goal 021 Phase 3 H39)

Finding 9a/9b: the 0.75/0.79 confidence lanes are 100% reviewed via the
mandatory review_task path -- free ground truth the monthly uniform
sample_audit draw never captures on its own.

0012_audit_labels.sql (expand-only) adds discrepancy_fields jsonb +
error_class (closed CHECK vocabulary) to sample_audit. promote::apply_
resolution's Verdict::Edit arm now mechanically diffs the original vs
corrected `details` payload (row-relative JSON pointers) and classifies
the discrepancy from the closed vocabulary, recording it under the
sentinel sample_month='review'/seed=0 so review-driven captures never
collide with a real monthly draw while reusing its idempotency.

migrate.rs's migration-count assertion bumps 12 -> 13 for this one new
file.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H40: Drift sentinel report-only bin (finding 9c)

**This task deliberately exceeds the ~1h guideline** — the drift-sentinel bin spans
`extractor.toml` threshold config, the baseline-comparison logic, per-field premium-vs-majority
agreement logging, and `review_task`-opening-on-breach as one coherent report-only unit;
splitting would leave a bin that compiles but reports against undefined thresholds. Do not
split it.

**Finding 9c.** A report-only worker bin sweeping weekly agreement/escalation/hold/schema-invalid rates against a trailing baseline, thresholds in `extractor.toml`, breach opens a `review_task`; plus logging per-field premium-vs-majority agreement from every escalation pass — a free, ongoing cross-model probe. Labels/report only: NO auto-demotion from lanes (explicitly out of scope per the goal text) — this bin never rewrites a confidence, never touches a Gold row.

**Files:**
- Create: `crates/worker/src/drift_sentinel.rs` (rate computation + breach evaluation + per-field agreement logging, all directly testable)
- Create: `crates/worker/src/bin/drift-sentinel.rs` (thin CLI wrapper, `check-br-identity-collisions.rs` doc-header style)
- Modify: `crates/worker/src/lib.rs` (register `pub mod drift_sentinel;`)
- Test: `crates/worker/tests/drift_sentinel.rs`

**Interfaces:**
- Consumes: `extraction_cache` (`document_sha256, extractor_tag, model_id, rows jsonb /* [{payload, confidence}] */, created_at`, existing, `crates/core/migrations/0004_extraction_cache.sql`, unmodified), `review_task` (existing, `crates/core/migrations/0001_core.sql`, unmodified), `extraction_batch` (Task 19, `status` lifecycle incl. `'failed'`), `extraction_sample` (Task 19, per-pass raw payloads — the per-field premium-vs-majority probe's source). `[drift]` thresholds — read `crates/pipeline/src/extraction/config.rs` first (H31/H32's task); this task ASSUMES `DriftConfig { max_agreement_delta: f32, max_escalation_delta: f32, max_hold_delta: f32, max_schema_invalid_delta: f32 }` (fractions, e.g. `0.05` = 5 percentage points) — if the real shape differs, adjust only the bin's config-loading glue, not `drift_sentinel.rs`'s comparison logic. `pipeline::stages::roster::open_review_task_once` (existing, `crates/pipeline/src/stages/roster.rs:157-188`, unmodified).
- Produces: `pub struct WeeklyRates { pub agreement_rate: f64, pub escalation_rate: f64, pub hold_rate: f64, pub schema_invalid_rate: f64 }`, `pub async fn compute_rates(pool, consensus_tag, regime_code, window_start, window_end) -> anyhow::Result<WeeklyRates>`, `pub struct DriftThresholds { .. }` (mirrors the assumed `[drift]` shape), `pub fn breached(current: &WeeklyRates, baseline: &WeeklyRates, thresholds: &DriftThresholds) -> Vec<&'static str>` (empty = within thresholds; non-empty names the breaching rate(s)), `pub async fn log_premium_vs_majority_agreement(pool, consensus_tag, escalation_model, window_start) -> anyhow::Result<()>` in `worker::drift_sentinel`; bin `drift-sentinel`.

- [ ] **Step 1: Write the failing test**

Create `crates/worker/tests/drift_sentinel.rs`:

```rust
//! Goal 021 Phase 3 H40 acceptance: drift-sentinel's rate math and breach
//! evaluation over synthetic extraction_cache/review_task/extraction_batch
//! populations. DB-gated like the other sqlx suites (`--ignored` + postgres
//! on `DATABASE_URL`).
#![allow(clippy::unwrap_used)]

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;

use worker::drift_sentinel::{DriftThresholds, breached, compute_rates};
use pipeline::stages::roster::open_review_task_once;

const TAG: &str = "us_house_ptr/consensus@1";

async fn insert_cache_row(pool: &PgPool, sha: &str, confidence: f32, days_ago: i64) {
    sqlx::query(
        "insert into extraction_cache (document_sha256, extractor_tag, model_id, rows, created_at) \
         values ($1, $2, 'test-composite@1', $3, now() - ($4 || ' days')::interval)",
    )
    .bind(sha)
    .bind(TAG)
    .bind(json!([{"payload": {"doc_id": sha}, "confidence": confidence}]))
    .bind(days_ago.to_string())
    .execute(pool)
    .await
    .unwrap();
}

fn thresholds() -> DriftThresholds {
    DriftThresholds {
        max_agreement_delta: 0.10,
        max_escalation_delta: 0.10,
        max_hold_delta: 0.10,
        max_schema_invalid_delta: 0.10,
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn stable_rates_never_breach(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    // 4 weeks of baseline + the current week, all 90% agreement / 10% escalation.
    for week in 0..5 {
        for i in 0..9 {
            insert_cache_row(&pool, &format!("stable-{week}-a-{i}"), 0.9, week * 7 + 1).await;
        }
        insert_cache_row(&pool, &format!("stable-{week}-b"), 0.75, week * 7 + 1).await;
    }
    let now = Utc::now();
    let current = compute_rates(&pool, TAG, "us_house", now - Duration::days(7), now)
        .await
        .unwrap();
    let mut baseline_weeks = Vec::new();
    for week in 1..5 {
        baseline_weeks.push(
            compute_rates(
                &pool, TAG, "us_house",
                now - Duration::days(7 * (week + 1)),
                now - Duration::days(7 * week),
            )
            .await
            .unwrap(),
        );
    }
    let baseline = worker::drift_sentinel::average(&baseline_weeks);
    assert!(breached(&current, &baseline, &thresholds()).is_empty(), "stable rates must not breach");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn escalation_spike_breaches_and_opens_a_review_task_once(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    // 4 baseline weeks at 10% escalation.
    for week in 1..5 {
        for i in 0..9 {
            insert_cache_row(&pool, &format!("base-{week}-a-{i}"), 0.9, week * 7 + 1).await;
        }
        insert_cache_row(&pool, &format!("base-{week}-b"), 0.75, week * 7 + 1).await;
    }
    // Current week: escalation rate spikes to 60%.
    for i in 0..4 {
        insert_cache_row(&pool, &format!("spike-a-{i}"), 0.9, 1).await;
    }
    for i in 0..6 {
        insert_cache_row(&pool, &format!("spike-b-{i}"), 0.75, 1).await;
    }

    let now = Utc::now();
    let current = compute_rates(&pool, TAG, "us_house", now - Duration::days(7), now).await.unwrap();
    let mut baseline_weeks = Vec::new();
    for week in 1..5 {
        baseline_weeks.push(
            compute_rates(&pool, TAG, "us_house", now - Duration::days(7 * (week + 1)), now - Duration::days(7 * week))
                .await
                .unwrap(),
        );
    }
    let baseline = worker::drift_sentinel::average(&baseline_weeks);
    let breaches = breached(&current, &baseline, &thresholds());
    assert!(breaches.contains(&"escalation_rate"), "the escalation spike must breach: {breaches:?}");

    let opened = open_review_task_once(&pool, "consensus_drift", &format!("{TAG}:test-week"), "extraction_drift")
        .await
        .unwrap();
    assert!(opened, "first breach opens a review_task");
    let opened_again = open_review_task_once(&pool, "consensus_drift", &format!("{TAG}:test-week"), "extraction_drift")
        .await
        .unwrap();
    assert!(!opened_again, "rerun for the SAME week is idempotent -- no duplicate task");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test drift_sentinel -- --ignored`
Expected: FAIL to compile — `worker::drift_sentinel` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Create `crates/worker/src/drift_sentinel.rs`:

```rust
//! Weekly extraction-quality drift sweep (goal 021 Phase 3 hardening addendum
//! `docs/plans/2026-07-07-consensus-hardening.md` H40, design amendment
//! `docs/plans/2026-07-07-consensus-extraction-amendment-1.md`, finding 9c).
//!
//! Report-only: this module and its `drift-sentinel` bin NEVER rewrite a
//! confidence, NEVER touch a Gold row, and NEVER auto-demote a lane —
//! explicitly out of scope per the goal text. A breach opens exactly one
//! `review_task` (`extraction_drift`, idempotent via `open_review_task_once`)
//! for a human/agent to investigate.

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row as _;

/// One week's (or baseline average's) agreement/escalation/hold/
/// schema-invalid rates for one `consensus_tag`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeeklyRates {
    /// Fraction of published `extraction_cache` rows at `CONF_AGREED` (0.90).
    pub agreement_rate: f64,
    /// Fraction of published `extraction_cache` rows at `CONF_ESCALATED` (0.75).
    pub escalation_rate: f64,
    /// `consensus_row_hold` review_task count / (holds + published rows).
    pub hold_rate: f64,
    /// Batch-path proxy: `extraction_batch` rows `status = 'failed'` /
    /// total `extraction_batch` rows in the window (sync-path schema-invalid
    /// failures freeze the whole document before any row-level data exists
    /// to count — this rate is intentionally the batch-path half of the
    /// signal, not a full cross-path measure; documented, not silent).
    pub schema_invalid_rate: f64,
}

/// `[drift]` breach thresholds (goal 021 Phase 3 H40) — read
/// `crates/pipeline/src/extraction/config.rs` first; this shape is assumed
/// per the shared plan contract. Each is a fraction (e.g. `0.05` = 5
/// percentage points) of allowed drift from the trailing 4-week baseline.
#[derive(Debug, Clone, Copy)]
pub struct DriftThresholds {
    pub max_agreement_delta: f32,
    pub max_escalation_delta: f32,
    pub max_hold_delta: f32,
    pub max_schema_invalid_delta: f32,
}

/// Computes one window's [`WeeklyRates`] for `consensus_tag`.
///
/// # Errors
/// Database failure.
pub async fn compute_rates(
    pool: &PgPool,
    consensus_tag: &str,
    regime_code: &str,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> anyhow::Result<WeeklyRates> {
    let cache_row = sqlx::query(
        "select \
           count(*) filter (where (r->>'confidence')::float4 = 0.9::float4) as agreed, \
           count(*) filter (where (r->>'confidence')::float4 = 0.75::float4) as escalated, \
           count(*) as total \
         from extraction_cache, jsonb_array_elements(rows) as r \
         where extractor_tag = $1 and created_at >= $2 and created_at < $3",
    )
    .bind(consensus_tag)
    .bind(window_start)
    .bind(window_end)
    .fetch_one(pool)
    .await
    .context("computing agreement/escalation rates")?;
    let agreed: i64 = cache_row.try_get("agreed")?;
    let escalated: i64 = cache_row.try_get("escalated")?;
    let total: i64 = cache_row.try_get("total")?;

    let holds: i64 = sqlx::query_scalar(
        "select count(*) from review_task \
         where reason = 'consensus_row_hold' and target_id like $1 \
           and created_at >= $2 and created_at < $3",
    )
    .bind(format!("{regime_code}:%"))
    .bind(window_start)
    .bind(window_end)
    .fetch_one(pool)
    .await
    .context("counting consensus_row_hold review_task rows")?;

    let batch_row = sqlx::query(
        "select count(*) filter (where status = 'failed') as failed, count(*) as total \
         from extraction_batch where consensus_tag = $1 and submitted_at >= $2 and submitted_at < $3",
    )
    .bind(consensus_tag)
    .bind(window_start)
    .bind(window_end)
    .fetch_one(pool)
    .await
    .context("computing batch schema-invalid proxy rate")?;
    let failed: i64 = batch_row.try_get("failed")?;
    let batch_total: i64 = batch_row.try_get("total")?;

    let row_total = (total + holds).max(1) as f64;
    Ok(WeeklyRates {
        agreement_rate: agreed as f64 / (total.max(1) as f64),
        escalation_rate: escalated as f64 / (total.max(1) as f64),
        hold_rate: holds as f64 / row_total,
        schema_invalid_rate: failed as f64 / (batch_total.max(1) as f64),
    })
}

/// Averages a set of [`WeeklyRates`] (the trailing 4-week baseline).
#[must_use]
pub fn average(weeks: &[WeeklyRates]) -> WeeklyRates {
    if weeks.is_empty() {
        return WeeklyRates::default();
    }
    let n = weeks.len() as f64;
    WeeklyRates {
        agreement_rate: weeks.iter().map(|w| w.agreement_rate).sum::<f64>() / n,
        escalation_rate: weeks.iter().map(|w| w.escalation_rate).sum::<f64>() / n,
        hold_rate: weeks.iter().map(|w| w.hold_rate).sum::<f64>() / n,
        schema_invalid_rate: weeks.iter().map(|w| w.schema_invalid_rate).sum::<f64>() / n,
    }
}

/// Names of every rate whose absolute delta from `baseline` exceeds its
/// threshold — empty means within thresholds (bin exits 0).
#[must_use]
pub fn breached(current: &WeeklyRates, baseline: &WeeklyRates, thresholds: &DriftThresholds) -> Vec<&'static str> {
    let mut breaches = Vec::new();
    if (current.agreement_rate - baseline.agreement_rate).abs() > f64::from(thresholds.max_agreement_delta) {
        breaches.push("agreement_rate");
    }
    if (current.escalation_rate - baseline.escalation_rate).abs() > f64::from(thresholds.max_escalation_delta) {
        breaches.push("escalation_rate");
    }
    if (current.hold_rate - baseline.hold_rate).abs() > f64::from(thresholds.max_hold_delta) {
        breaches.push("hold_rate");
    }
    if (current.schema_invalid_rate - baseline.schema_invalid_rate).abs()
        > f64::from(thresholds.max_schema_invalid_delta)
    {
        breaches.push("schema_invalid_rate");
    }
    breaches
}

/// Logs (to stdout) per-field premium-vs-majority agreement from every
/// escalation pass recorded since `window_start` (finding 9c: "free ongoing
/// cross-model probe") — for each document with an `extraction_sample` row
/// whose `model_id == escalation_model`, compares the escalation pass's
/// `/rows[i]` field values against the MODE of the primary samples'
/// `/rows[i]` for the same field, tallies agree/total per field name across
/// the window. Generic over whatever top-level fields a row payload
/// happens to carry — does not depend on any adapter's exact schema.
///
/// # Errors
/// Database failure.
pub async fn log_premium_vs_majority_agreement(
    pool: &PgPool,
    consensus_tag: &str,
    escalation_model: &str,
    window_start: DateTime<Utc>,
) -> anyhow::Result<()> {
    let docs: Vec<String> = sqlx::query_scalar(
        "select distinct document_sha256 from extraction_sample \
         where consensus_tag = $1 and model_id = $2 and created_at >= $3",
    )
    .bind(consensus_tag)
    .bind(escalation_model)
    .bind(window_start)
    .fetch_all(pool)
    .await
    .context("listing documents with an escalation pass")?;

    let mut field_agree: std::collections::BTreeMap<String, (u32, u32)> = std::collections::BTreeMap::new();
    for sha in docs {
        let samples: Vec<(String, serde_json::Value)> = sqlx::query_as(
            "select model_id, payload from extraction_sample \
             where document_sha256 = $1 and consensus_tag = $2 order by pass_idx",
        )
        .bind(&sha)
        .bind(consensus_tag)
        .fetch_all(pool)
        .await
        .context("loading samples for the premium-vs-majority probe")?;
        let (escalation, primary): (Vec<_>, Vec<_>) =
            samples.into_iter().partition(|(model, _)| model == escalation_model);
        let Some((_, premium_payload)) = escalation.into_iter().next() else {
            continue;
        };
        let premium_rows = premium_payload
            .pointer("/rows")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (row_idx, premium_row) in premium_rows.iter().enumerate() {
            let Some(premium_obj) = premium_row.as_object() else {
                continue;
            };
            for (field, premium_value) in premium_obj {
                let majority = majority_value(&primary, row_idx, field);
                let entry = field_agree.entry(field.clone()).or_insert((0, 0));
                entry.1 += 1;
                if majority.as_ref() == Some(premium_value) {
                    entry.0 += 1;
                }
            }
        }
    }
    for (field, (agree, total)) in &field_agree {
        println!(
            "  premium-vs-majority {field}: {agree}/{total} ({:.1}%)",
            100.0 * f64::from(*agree) / f64::from((*total).max(1))
        );
    }
    Ok(())
}

/// The mode value of `field` at `row_idx` across `primary`'s sample
/// payloads (ties broken by first sample order — this is instrumentation
/// logging, not a routing decision, so a deterministic pick is sufficient).
fn majority_value(
    primary: &[(String, serde_json::Value)],
    row_idx: usize,
    field: &str,
) -> Option<serde_json::Value> {
    let mut counts: Vec<(serde_json::Value, u32)> = Vec::new();
    for (_, payload) in primary {
        let Some(row) = payload
            .pointer("/rows")
            .and_then(serde_json::Value::as_array)
            .and_then(|rows| rows.get(row_idx))
        else {
            continue;
        };
        let Some(value) = row.get(field) else {
            continue;
        };
        match counts.iter_mut().find(|(candidate, _)| candidate == value) {
            Some(entry) => entry.1 += 1,
            None => counts.push((value.clone(), 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, count)| *count).map(|(v, _)| v)
}
```

Create `crates/worker/src/bin/drift-sentinel.rs`:

```rust
//! Weekly extraction-quality drift sweep (goal 021 Phase 3 hardening addendum
//! `docs/plans/2026-07-07-consensus-hardening.md` H40, design amendment
//! `docs/plans/2026-07-07-consensus-extraction-amendment-1.md`, finding 9c).
//!
//! Sweeps agreement/escalation/hold/schema-invalid rates for the current
//! week against a trailing 4-week baseline (`[drift]` thresholds in
//! `config/extractor.toml`); a breach opens ONE `extraction_drift`
//! `review_task` (idempotent per week via `open_review_task_once`). Also
//! logs per-field premium-vs-majority agreement from every escalation pass
//! recorded this week (finding 9c: "free ongoing cross-model probe").
//!
//! Not a CI gate: nothing in this repo's command chain fails the build on
//! this bin's exit code. Report/label only — this bin NEVER rewrites a
//! confidence, NEVER touches a Gold row, and NEVER auto-demotes a lane
//! (explicitly out of scope per the goal text). Run it manually, or wire it
//! into a future Cloud Scheduler cadence (no terraform/scheduler in this
//! goal, matching `consensus-batch-poll`'s own convention).
//!
//! Usage:
//!   cargo run -p worker --bin drift-sentinel [-- <consensus_tag> <regime_code>]
//!
//! Env: `DATABASE_URL` (required). Args default to
//! `"us_house_ptr/consensus@1"` / `"us_house"`.
//!
//! Exit code: 0 = within thresholds (or `[drift]` unset — the bin refuses
//! to invent a threshold, it just reports "no thresholds configured, PASS
//! by default" and exits 0). Nonzero = a breach was found and a
//! `review_task` opened — mirrors `check-br-identity-collisions`'s
//! PASS/BLOCKED convention (nonzero means "look at this"), not a fail-closed
//! halt.

use anyhow::Context as _;
use chrono::{Duration, Utc};

use pipeline::stages::roster::open_review_task_once;
use worker::drift_sentinel::{DriftThresholds, average, breached, compute_rates, log_premium_vs_majority_agreement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let consensus_tag = args.next().unwrap_or_else(|| "us_house_ptr/consensus@1".to_owned());
    let regime_code = args.next().unwrap_or_else(|| "us_house".to_owned());

    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    let now = Utc::now();
    let current = compute_rates(&pool, &consensus_tag, &regime_code, now - Duration::days(7), now).await?;
    let mut baseline_weeks = Vec::with_capacity(4);
    for week in 1..=4 {
        baseline_weeks.push(
            compute_rates(
                &pool, &consensus_tag, &regime_code,
                now - Duration::days(7 * (week + 1)),
                now - Duration::days(7 * week),
            )
            .await?,
        );
    }
    let baseline = average(&baseline_weeks);
    println!(
        "drift-sentinel {consensus_tag} ({regime_code}): current={current:?} baseline={baseline:?}"
    );

    // Config-not-code ([drift], H31/H32's config.rs) — read
    // pipeline::extraction::config first. Absent thresholds are NOT a
    // fabricated default: report and exit 0 rather than guessing a number.
    let thresholds = match pipeline::extraction::config::ExtractorConfig::load() {
        Ok(cfg) => DriftThresholds {
            max_agreement_delta: cfg.drift.max_agreement_delta,
            max_escalation_delta: cfg.drift.max_escalation_delta,
            max_hold_delta: cfg.drift.max_hold_delta,
            max_schema_invalid_delta: cfg.drift.max_schema_invalid_delta,
        },
        Err(e) => {
            println!("drift-sentinel: no [drift] thresholds configured ({e}) — PASS by default, not a guess");
            return Ok(());
        }
    };

    log_premium_vs_majority_agreement(&pool, &consensus_tag, "claude-sonnet-5", now - Duration::days(7)).await?;

    let breaches = breached(&current, &baseline, &thresholds);
    if breaches.is_empty() {
        println!("PASS: all rates within trailing-4-week thresholds.");
        return Ok(());
    }

    println!("DRIFT: {} rate(s) breached: {breaches:?}", breaches.len());
    let week_label = now.format("%G-W%V").to_string();
    open_review_task_once(
        &pool,
        "consensus_drift",
        &format!("{consensus_tag}:{week_label}"),
        "extraction_drift",
    )
    .await?;
    std::process::exit(1);
}
```

Add `pub mod drift_sentinel;` to `crates/worker/src/lib.rs`'s module list (alphabetical among the existing `pub mod` lines), extending its top doc comment to mention it — same pattern Task 22 used for `consensus_batch`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p worker --test drift_sentinel -- --ignored`
Expected: PASS (`stable_rates_never_breach`, `escalation_spike_breaches_and_opens_a_review_task_once`).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/drift_sentinel.rs crates/worker/src/bin/drift-sentinel.rs crates/worker/src/lib.rs crates/worker/tests/drift_sentinel.rs
git commit -m "$(cat <<'EOF'
feat(worker): drift-sentinel report-only weekly rate sweep (goal 021 Phase 3 H40)

Finding 9c: weekly agreement/escalation/hold/schema-invalid rates for one
consensus_tag vs a trailing 4-week baseline, [drift] thresholds in
extractor.toml; a breach opens ONE extraction_drift review_task
(idempotent per calendar week via open_review_task_once). Also logs
per-field premium-vs-majority agreement from every escalation pass this
week -- a free, ongoing cross-model probe.

Report-only: never rewrites a confidence, never touches a Gold row, never
auto-demotes a lane (explicitly out of scope per the goal text). Not a CI
gate, mirrors check-br-identity-collisions's PASS/BLOCKED exit-code
convention.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H41a: Shadow consensus eval harness — offline core (finding 10)

Finding 10 (goal `agents/goals/021-llm-extraction.md` §3): *"Shadow consensus eval harness.
Manual worker bin: electronic PTRs (deterministic Silver = ground truth) → rasterize → full
consensus path via Batch under isolated tag `shadow@1` → per-field confusion matrices
bucketed by consensus outcome. Measures P(wrong | 3/3 agree) — the design's admitted blind
spot — before the 50k backfill."* This task is the offline core only: the pure
align/score/route wiring and the confusion-matrix math, exercised against SCRIPTED
`SamplePass`-shaped fixtures (no network, no `shadow@1` tag yet — that literal and the
HARD-CAP-gated live submit arm are H41b). The bin this task creates reads ground truth from
Postgres (the only I/O in this task) but the matrix math itself is a pure function over
already-in-memory `Value`s.

**Files:**
- Create: `crates/worker/src/consensus_shadow.rs`
- Create: `crates/worker/src/bin/consensus-shadow-eval.rs`
- Modify: `crates/worker/src/lib.rs` (register `pub mod consensus_shadow;`, alphabetical
  among the existing `pub mod` lines; extend the top doc comment: `; the shadow consensus
  eval harness (goal 021 Phase 3 hardening, finding 10) in [`consensus_shadow`]`)
- Test: `crates/worker/src/consensus_shadow.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `pipeline::extraction::consensus::{ConsensusSpec, RowVerdict, SamplePass, align,
  policy, resolve_disputed, score}` (shared contract: post-changeset `RowVerdict::Disputed`
  carries `key: RowKey`; `resolve_disputed(ordinal0, key, candidates, disputed_fields, spec,
  premium)`); `us_house::consensus::consensus_spec()` (Task 18); `worker::consensus_batch::
  shas_from_file` (Task 22, path-list parsing, reused verbatim — no local re-implementation).
- Produces (all `pub`, in `worker::consensus_shadow`): `enum OutcomeBucket { Agreed090,
  Escalated075, Capped079, Held }` (`Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord`);
  `struct RoutedRow { pub outcome: OutcomeBucket, pub published: Option<Value> }`; `fn
  route_document(samples: &[Value], premium: Option<&Value>, spec: &ConsensusSpec) ->
  anyhow::Result<Vec<RoutedRow>>`; `struct FieldTally { pub agree: u32, pub mismatch: u32 }`
  (`Debug, Default, Clone, Copy, PartialEq, Eq`); `struct ConfusionMatrix` with `fn cell(&self,
  outcome: OutcomeBucket, field: &str) -> FieldTally` and `fn accumulate(&mut self,
  ground_truth_rows: &[Value], routed_rows: &[RoutedRow], critical_fields: &[String])`; `fn
  build_confusion_matrix(docs: &[(Vec<Value>, Vec<RoutedRow>)], critical_fields: &[String]) ->
  ConfusionMatrix`; bin `consensus-shadow-eval` (offline report mode: `--shas <file> --tag
  <consensus_tag>`). H41b extends this same bin with a `--submit` live arm and adds
  `SHADOW_TAG`/`submit_shadow_batch` to this module.

`route_document` deliberately does NOT reuse `ConsensusExtractor::extract` (which owns a
`Transport` and always decides for itself whether to escalate, per H32's `premium_needed`
disjunction). The harness needs to control exactly which rows see a premium pass so a single
synthetic population can isolate "3/3 agree, nobody ever double-checked it" (premium omitted)
from premium-triggered scrutiny (premium supplied) — so `premium` here is always a SCRIPTED,
already-in-hand `Value`, never fetched. It mirrors the §7.1 lane mapping directly: an `Agreed`
verdict the (optional) premium dissents on caps to `Capped079`; a `Disputed` verdict `resolve_
disputed` resolves caps to `Escalated075`; anything else is `Held`. This is intentionally the
SAME shape H43a's offline re-scorer reuses (do not duplicate this routing logic there).

- [ ] **Step 1: Write the failing test**

Create `crates/worker/src/consensus_shadow.rs` with just enough to fail to compile — the
`#[cfg(test)]` module below plus `use` lines the eventual Step-3 code will satisfy:

```rust
//! Shadow consensus eval harness — offline core (goal 021 Phase 3 hardening,
//! finding 10 / design amendment A10). Measures the design's admitted blind
//! spot, P(wrong | 3/3 agree), by running SCRIPTED sample-pass fixtures
//! through the REAL comparator (`pipeline::extraction::consensus::{align,
//! score, resolve_disputed}`) and diffing the resulting published values
//! against deterministic Silver ground truth, bucketed by the policy_v1/pol2
//! §7.1 outcome lane. Zero I/O in this module: every fn here is pure over
//! already-constructed `Value` fixtures. The bin `consensus-shadow-eval`
//! (this module's caller) is what touches Postgres (to READ ground truth) or
//! the network (H41b's live submit arm) — this module never does either.

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde_json::Value;

use pipeline::extraction::consensus::{
    ConsensusSpec, RowVerdict, align, resolve_disputed, score,
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(band_column: &str) -> Value {
        json!({
            "owner_code_raw": "SP",
            "asset_raw": "Apple Inc Common Stock",
            "transaction_type_raw": "P",
            "transaction_date_raw": "2026-01-05",
            "notification_date_raw": "2026-01-06",
            "band_column": band_column,
        })
    }
    fn sample_payload(band_column: &str) -> Value {
        json!({ "rows": [row(band_column)] })
    }

    /// A synthetic 5-document population: 2 "3/3-agree, WRONG band" plants,
    /// 2 "3/3-agree, correct" controls, and 1 genuinely disputed doc (2 vs 1
    /// samples, no scripted premium) whose ground truth deliberately uses a
    /// THIRD amount — a bucketing leak would show up as extra mismatches in
    /// the 0.90 cell, not just a missing Held entry, so this population
    /// exercises both halves of the pivotal claim at once.
    #[test]
    fn agreed_bucket_confusion_counts_exactly_the_planted_wrong_bands() {
        // H35b's page-aware consensus_spec(pages) — a blank placeholder page
        // is fine here: this test exercises alignment/scoring over rows_pointer/
        // key_fields/critical_fields only, never table_regions/template_recognized.
        let blank_page = image::GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
        let spec = us_house::consensus::consensus_spec(&[blank_page]);
        const CORRECT: &str = "B";
        const WRONG: &str = "C";
        const DISPUTED_TRUTH: &str = "D";

        let mut docs: Vec<(Vec<Value>, Vec<RoutedRow>)> = Vec::new();

        for _ in 0..2 {
            let samples = vec![sample_payload(WRONG); 3];
            let routed = route_document(&samples, None, &spec).unwrap();
            docs.push((vec![row(CORRECT)], routed));
        }
        for _ in 0..2 {
            let samples = vec![sample_payload(CORRECT); 3];
            let routed = route_document(&samples, None, &spec).unwrap();
            docs.push((vec![row(CORRECT)], routed));
        }
        {
            let samples = vec![
                sample_payload(CORRECT),
                sample_payload(CORRECT),
                sample_payload(WRONG),
            ];
            let routed = route_document(&samples, None, &spec).unwrap();
            docs.push((vec![row(DISPUTED_TRUTH)], routed));
        }

        let matrix = build_confusion_matrix(&docs, &spec.critical_fields);

        let band = matrix.cell(OutcomeBucket::Agreed090, "/band_column");
        assert_eq!(band.mismatch, 2, "exactly the 2 planted wrong bands");
        assert_eq!(band.agree, 2, "the 2 correct controls, nothing leaked from the Held doc");
        assert_eq!(
            matrix.cell(OutcomeBucket::Held, "/band_column"),
            FieldTally::default(),
            "Held rows publish nothing, so bucketing is exact: zero tally, never a leak"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker --lib consensus_shadow::tests`
Expected: FAIL to compile — `route_document`, `build_confusion_matrix`, `OutcomeBucket`,
`RoutedRow`, `FieldTally` do not exist yet, and `consensus_shadow` is not yet registered in
`lib.rs` (`error[E0433]: failed to resolve: could not find 'consensus_shadow' in the crate
root`).

- [ ] **Step 3: Write minimal implementation**

Append to `crates/worker/src/consensus_shadow.rs` (above the `#[cfg(test)]` module):

```rust
/// The four policy_v1/pol2 §7.1 lanes a row can land in — the confusion-
/// matrix outer bucket key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutcomeBucket {
    Agreed090,
    Escalated075,
    Capped079,
    Held,
}

/// One row's routed outcome: which lane it landed in, and — for every lane
/// except `Held` — the published row `Value` (the mechanism under test: does
/// the PUBLISHED value match ground truth).
#[derive(Debug, Clone)]
pub struct RoutedRow {
    pub outcome: OutcomeBucket,
    pub published: Option<Value>,
}

/// Routes one document's already-collected sample passes. `premium` is a
/// SCRIPTED fixture, never fetched (the live fetch is H41b's/H43b's job) —
/// this lets one synthetic population isolate "3/3 agree, no scrutiny"
/// (`premium: None`) from premium-triggered scrutiny (`premium: Some(..)`)
/// in the same run.
///
/// # Errors
/// `align`/`score` failure over a malformed sample payload (fail closed —
/// same as the live path).
pub fn route_document(
    samples: &[Value],
    premium: Option<&Value>,
    spec: &ConsensusSpec,
) -> anyhow::Result<Vec<RoutedRow>> {
    let aligned = align(samples, spec)?;
    let verdicts = score(&aligned, spec);
    let mut routed = Vec::with_capacity(verdicts.len());
    for verdict in verdicts {
        match verdict {
            RowVerdict::Agreed { row, .. } => {
                let dissents = premium.is_some_and(|p| {
                    spec.critical_fields
                        .iter()
                        .any(|f| p.pointer(f) != row.pointer(f))
                });
                routed.push(RoutedRow {
                    outcome: if dissents {
                        OutcomeBucket::Capped079
                    } else {
                        OutcomeBucket::Agreed090
                    },
                    published: Some(row),
                });
            }
            RowVerdict::Disputed {
                ordinal0,
                key,
                candidates,
                disputed_fields,
            } => {
                let resolved = premium.and_then(|p| {
                    resolve_disputed(ordinal0, &key, &candidates, &disputed_fields, spec, p)
                });
                routed.push(match resolved {
                    Some(published_row) => RoutedRow {
                        outcome: OutcomeBucket::Escalated075,
                        published: Some(published_row.row),
                    },
                    None => RoutedRow {
                        outcome: OutcomeBucket::Held,
                        published: None,
                    },
                });
            }
        }
    }
    Ok(routed)
}

/// One field's outcome-bucketed agreement tally against ground truth.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FieldTally {
    pub agree: u32,
    pub mismatch: u32,
}

/// Per-(outcome, field) confusion tallies across a document population —
/// the harness's core measurement, `P(wrong | outcome)` per critical field.
#[derive(Debug, Default, Clone)]
pub struct ConfusionMatrix(BTreeMap<(OutcomeBucket, String), FieldTally>);

impl ConfusionMatrix {
    /// Reads one (outcome, field) cell — zero tallies if never observed.
    #[must_use]
    pub fn cell(&self, outcome: OutcomeBucket, field: &str) -> FieldTally {
        self.0.get(&(outcome, field.to_owned())).copied().unwrap_or_default()
    }

    /// Folds ONE document's ground-truth/routed row pairs into the running
    /// tallies (row-ordinal matched WITHIN one document — call once per
    /// document, never across documents, since two different documents'
    /// row 0 are unrelated).
    pub fn accumulate(
        &mut self,
        ground_truth_rows: &[Value],
        routed_rows: &[RoutedRow],
        critical_fields: &[String],
    ) {
        for (truth, routed) in ground_truth_rows.iter().zip(routed_rows) {
            let Some(published) = &routed.published else {
                continue; // Held: nothing published to score.
            };
            for field in critical_fields {
                let tally = self.0.entry((routed.outcome, field.clone())).or_default();
                if truth.pointer(field) == published.pointer(field) {
                    tally.agree += 1;
                } else {
                    tally.mismatch += 1;
                }
            }
        }
    }
}

/// Builds the confusion matrix over a whole population: one
/// `(ground_truth_rows, routed_rows)` pair per document.
#[must_use]
pub fn build_confusion_matrix(
    docs: &[(Vec<Value>, Vec<RoutedRow>)],
    critical_fields: &[String],
) -> ConfusionMatrix {
    let mut matrix = ConfusionMatrix::default();
    for (truth, routed) in docs {
        matrix.accumulate(truth, routed, critical_fields);
    }
    matrix
}

/// Reads one electronic PTR's deterministic Silver rows (`stg_us_house`,
/// extractor `us_house_ptr/text@1`) as ground-truth `Value`s keyed by the
/// SAME field names the consensus row pointers use (`/amount_raw` etc.) —
/// the bin's only Postgres read.
///
/// # Errors
/// Database failure.
pub async fn ground_truth_rows(
    pool: &sqlx::PgPool,
    document_sha256: &str,
) -> anyhow::Result<Vec<Value>> {
    let rows = sqlx::query(
        "select s.owner_code_raw, s.asset_raw, s.transaction_type_raw, \
                s.transaction_date_raw, s.notification_date_raw, s.amount_raw \
         from stg_us_house s join raw_document rd on rd.id = s.raw_document_id \
         where rd.sha256 = $1 and s.extractor = 'us_house_ptr/text@1' \
         order by s.row_ordinal",
    )
    .bind(document_sha256)
    .fetch_all(pool)
    .await
    .with_context(|| format!("reading deterministic Silver ground truth for {document_sha256}"))?;
    use sqlx::Row as _;
    rows.into_iter()
        .map(|r| {
            Ok(serde_json::json!({
                "owner_code_raw": r.try_get::<Option<String>, _>("owner_code_raw")?,
                "asset_raw": r.try_get::<String, _>("asset_raw")?,
                "transaction_type_raw": r.try_get::<String, _>("transaction_type_raw")?,
                "transaction_date_raw": r.try_get::<String, _>("transaction_date_raw")?,
                "notification_date_raw": r.try_get::<String, _>("notification_date_raw")?,
                "amount_raw": r.try_get::<String, _>("amount_raw")?,
            }))
        })
        .collect()
}
```

Create `crates/worker/src/bin/consensus-shadow-eval.rs`:

```rust
//! Shadow consensus eval harness (goal 021 Phase 3 hardening, finding 10 /
//! design amendment A10): measures P(wrong | 3/3 agree) — the design's
//! admitted blind spot — over electronic PTRs, whose deterministic Silver
//! rows are ground truth. This bin doubles as the §D bake-off rig (H45): it
//! is built once and reused. Manual invocation only — NEVER wired into any
//! CI/command-chain gate.
//!
//! This file's OFFLINE report mode (this task, H41a): reads deterministic
//! Silver ground truth plus already-stored `extraction_sample` rows for a
//! given `consensus_tag` and prints the confusion matrix. Zero network
//! calls. H41b adds a `--submit` LIVE arm to this same bin (HARD-CAP gated,
//! `shadow@1` tag) that produces the samples this mode later reads.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin consensus-shadow-eval -- --shas shas.txt --tag shadow@1
//! ```
//! Env: `DATABASE_URL` (required).

use anyhow::Context as _;

use worker::consensus_batch::{load_samples, shas_from_file};
use worker::consensus_shadow::{build_confusion_matrix, ground_truth_rows, route_document};

fn parse_args() -> anyhow::Result<(std::path::PathBuf, String)> {
    let mut shas_file = None;
    let mut tag = None;
    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        let mut value = |name: &str| {
            cli.next().with_context(|| format!("{name} requires a value"))
        };
        match flag.as_str() {
            "--shas" => shas_file = Some(std::path::PathBuf::from(value("--shas")?)),
            "--tag" => tag = Some(value("--tag")?),
            other => anyhow::bail!("unknown argument {other:?} (expected --shas <file> --tag <consensus_tag>)"),
        }
    }
    Ok((
        shas_file.context("--shas is required")?,
        tag.context("--tag is required")?,
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (shas_file, tag) = parse_args()?;
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url).await.context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool).await.context("applying migrations")?;

    let shas = shas_from_file(&shas_file).await?;
    // H35b's page-aware consensus_spec(pages) — this offline report loop
    // compares ALREADY-STORED extraction_sample payloads (no page bytes
    // reloaded here), so a blank placeholder page is the correct choice:
    // template_recognized/table_regions are irrelevant to this comparison.
    let blank_page = image::GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
    let spec = us_house::consensus::consensus_spec(&[blank_page]);

    let mut docs = Vec::new();
    let mut skipped = 0usize;
    for sha in &shas {
        let samples = load_samples(&pool, sha, &tag).await?;
        if samples.is_empty() {
            skipped += 1;
            continue;
        }
        let truth = ground_truth_rows(&pool, sha).await?;
        let sample_values: Vec<_> = samples.iter().map(|s| s.payload.clone()).collect();
        let routed = route_document(&sample_values, None, &spec)?;
        docs.push((truth, routed));
    }

    let matrix = build_confusion_matrix(&docs, &spec.critical_fields);
    println!("consensus-shadow-eval: {} document(s) scored, {skipped} skipped (no stored samples under tag {tag:?})", docs.len());
    for field in &spec.critical_fields {
        for outcome in [
            worker::consensus_shadow::OutcomeBucket::Agreed090,
            worker::consensus_shadow::OutcomeBucket::Escalated075,
            worker::consensus_shadow::OutcomeBucket::Capped079,
            worker::consensus_shadow::OutcomeBucket::Held,
        ] {
            let tally = matrix.cell(outcome, field);
            if tally.agree + tally.mismatch > 0 {
                println!("  {field} {outcome:?}: agree={} mismatch={}", tally.agree, tally.mismatch);
            }
        }
    }
    Ok(())
}
```

`OutcomeBucket` needs `#[derive(Debug, ...)]` (already has `Debug` from Step 3) for the
`{outcome:?}` format above.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker --lib consensus_shadow::tests`
Expected: PASS (1 test). Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
```bash
git add crates/worker/src/consensus_shadow.rs crates/worker/src/bin/consensus-shadow-eval.rs crates/worker/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(worker): shadow consensus eval harness offline core — confusion matrix by outcome (goal 021 Phase 3 hardening, task H41a)

Adds route_document (align/score/route over scripted sample fixtures, no
network) and build_confusion_matrix (per-field, per-outcome agree/mismatch
tallies against deterministic Silver ground truth) to a new
worker::consensus_shadow module, plus the consensus-shadow-eval bin's
offline report mode. Measures P(wrong | 3/3 agree) — finding 10's targeted
blind spot — before H41b adds the shadow@1-tagged live submit arm.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H41b: Shadow harness Batch wiring — shadow@1 isolation + HARD-CAP gate (finding 10)

The bin's live arm: rasterize electronic PTRs (pdfium) and submit a Message Batch via the
Task-21/22 batch machinery (`pipeline::extraction::{BatchTransport, HttpTransport}`,
`worker::consensus_batch::{check_budget_gate, build_batch_requests, record_batch_submitted}`)
under consensus_tag literal `shadow@1`. Isolation: `extraction_sample`/`extraction_cache` rows
this produces are keyed `shadow@1` — the CacheKey `extractor_tag` component
(`pipeline::extraction::cache::CacheKey::new(sha, "shadow@1", composite_model_id)`, per
`crates/pipeline/src/extraction/cache.rs`) — so a shadow run can never collide with, or be
mistaken for, a production `us_house_ptr/consensus@1` cache row. Poll/ingest reuses Task 23's
`consensus-batch-poll` bin and `resolve_document` UNCHANGED — that function's write surface is
`extraction_cache` (`pg_put`) and `open_review_task_once`; it has NO Silver/Gold call site at
all, so "never writes stg_/Gold" holds for shadow batches BY CONSTRUCTION, not by a special
case this task adds. The one side effect that DOES need isolating is `open_review_task_once`'s
`consensus_row_hold` task for a still-disputed shadow row — undesirable noise in the real
`us_house` review queue. Fix: shadow batches record `regime_code = SHADOW_REGIME`
(`"us_house_shadow"`, not `"us_house"`); `open_review_task_once`'s `target_id` becomes
`"us_house_shadow:{sha}"`, namespaced away from real `"us_house:{sha}"` review work by
construction (review_task has no FK on `target_id`/`regime_code` — free text — so this is
safe). Spend is gated by `check_budget_gate` (Task 22, itself built on `ExtractorConfig::
require_budget`) — while HARD CAP values are unset, this refuses before any API call.

**Files:**
- Modify: `crates/worker/src/consensus_shadow.rs` (add `SHADOW_TAG`, `SHADOW_REGIME`,
  `submit_shadow_batch`)
- Modify: `crates/worker/src/bin/consensus-shadow-eval.rs` (add the `--submit` live arm)
- Test: `crates/worker/src/consensus_shadow.rs` (inline `#[cfg(test)] mod tests`, appended)

**Interfaces:**
- Consumes: `worker::consensus_batch::{check_budget_gate, build_batch_requests,
  record_batch_submitted}` (Task 22, reused verbatim — no local re-implementation of the cap
  gate or request builder); `pipeline::extraction::{BatchTransport, HttpTransport}` (Task 21);
  `pipeline::extraction::config::ExtractorConfig`; `pipeline::extraction::anthropic::
  {DocumentToolSpec, SamplingParams}`; `pipeline::extraction::preprocess::{PreprocessCfg,
  preprocess_document}`; `us_house::consensus::consensus_tool_spec()`.
- Produces: `pub const SHADOW_TAG: &str = "shadow@1"`; `pub const SHADOW_REGIME: &str =
  "us_house_shadow"`; `pub async fn submit_shadow_batch<T: BatchTransport>(cfg:
  &ExtractorConfig, transport: &T, docs: &[(String, Vec<Vec<u8>>)], tool_spec:
  &DocumentToolSpec) -> anyhow::Result<String>` (returns the anthropic batch id; caller
  records it via `record_batch_submitted(pool, &batch_id, SHADOW_REGIME, SHADOW_TAG,
  &composite_model_id, &shas)`); bin `consensus-shadow-eval --submit`.

- [ ] **Step 1: Write the failing test**

Append to `crates/worker/src/consensus_shadow.rs`'s `#[cfg(test)] mod tests`:

```rust
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use pipeline::extraction::BatchTransport;

/// A spy `BatchTransport` that counts `create_batch` calls — proves the
/// budget gate refuses BEFORE any transport call, not merely that it errs.
struct CountingTransport {
    calls: AtomicUsize,
}

#[async_trait]
impl BatchTransport for CountingTransport {
    async fn create_batch(&self, _requests: &Value) -> anyhow::Result<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("msgbatch_should_never_happen".to_owned())
    }
    async fn batch_status(&self, _id: &str) -> anyhow::Result<String> {
        unreachable!("this test never gets past submission")
    }
    async fn batch_results(&self, _id: &str) -> anyhow::Result<Vec<(String, Value)>> {
        unreachable!("this test never gets past submission")
    }
}

/// Env mutation is process-global — mirrors Task 22's `ENV_LOCK` convention
/// so this test never races another test's `GOVFOLIO_EXTRACTOR_CONFIG`.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn shadow_submit_refuses_before_any_transport_call_when_budget_unset() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(
        file,
        r#"
[models]
primary = "claude-haiku-4-5-20251001"
escalation = "claude-sonnet-5"

[sampling]
n = 3
temperature = 0.7

[preprocess]
max_edge = 1568

[versions]
prompt = "p1"
policy = "pol1"
"#
    )
    .unwrap();
    let previous = std::env::var("GOVFOLIO_EXTRACTOR_CONFIG").ok();
    unsafe { std::env::set_var("GOVFOLIO_EXTRACTOR_CONFIG", file.path()) };
    let cfg = pipeline::extraction::config::ExtractorConfig::load().unwrap();

    let transport = CountingTransport { calls: AtomicUsize::new(0) };
    let tool_spec = us_house::consensus::consensus_tool_spec();
    let docs = vec![("shaX".to_owned(), vec![vec![0u8; 4]])];

    let err = submit_shadow_batch(&cfg, &transport, &docs, &tool_spec)
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("budget"),
        "error should name the missing budget key: {err}"
    );
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "must refuse before any transport call — zero create_batch calls"
    );

    match previous {
        Some(v) => unsafe { std::env::set_var("GOVFOLIO_EXTRACTOR_CONFIG", v) },
        None => unsafe { std::env::remove_var("GOVFOLIO_EXTRACTOR_CONFIG") },
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker --lib consensus_shadow::tests::shadow_submit_refuses_before_any_transport_call_when_budget_unset`
Expected: FAIL to compile — `submit_shadow_batch` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Append to `crates/worker/src/consensus_shadow.rs` (imports alongside the existing ones at the
top of the file — add `use pipeline::extraction::anthropic::{DocumentToolSpec,
SamplingParams}; use pipeline::extraction::config::ExtractorConfig; use
pipeline::extraction::BatchTransport;` and `use worker::consensus_batch::{build_batch_requests,
check_budget_gate};` — NOTE: this file IS `worker::consensus_batch`'s sibling module, so within
the crate it's `use crate::consensus_batch::{build_batch_requests, check_budget_gate};`):

```rust
use crate::consensus_batch::{build_batch_requests, check_budget_gate};

/// Literal `consensus_tag` for shadow-harness runs (finding 10 / design
/// amendment A10) — used ONLY as `extraction_sample.consensus_tag` and the
/// `extractor_tag` component of `pipeline::extraction::cache::CacheKey` for
/// shadow rows. NEVER the production `us_house_ptr/consensus@1` tag, so
/// shadow spend and shadow cache entries can never collide with, or be
/// mistaken for, production data.
pub const SHADOW_TAG: &str = "shadow@1";

/// Literal `regime_code` for shadow-harness `extraction_batch` rows — NOT
/// `"us_house"`. `worker::consensus_batch::resolve_document` (Task 23, reused
/// unmodified by this task's poll side) opens a `consensus_row_hold`
/// `review_task` for a still-disputed row with `target_id =
/// "{regime_code}:{sha}"`; using this distinct pseudo-regime namespaces
/// shadow holds away from the real `us_house` review queue (review_task has
/// no FK on `target_id`/`regime_code` — this is safe free text).
pub const SHADOW_REGIME: &str = "us_house_shadow";

/// The shadow harness's live submit arm: budget-gates FIRST (before any
/// transport call — `check_budget_gate`, the SAME fail-closed fn
/// `consensus-batch-submit` uses, Task 22), then builds and submits ONE
/// Message Batch under [`SHADOW_TAG`]. Never calls a Silver/Gold persist
/// path itself; the caller records the returned batch id via
/// `record_batch_submitted(pool, &id, SHADOW_REGIME, SHADOW_TAG, ..)` and
/// poll-side ingestion reuses `consensus-batch-poll`/`resolve_document`
/// (Task 23) UNCHANGED — that fn's only writes are `extraction_cache` and
/// `review_task`, never `stg_us_house`/Gold.
///
/// # Errors
/// `BudgetUnset` (fail closed, before any network call), or transport/API
/// failure from `create_batch`.
pub async fn submit_shadow_batch<T: BatchTransport>(
    cfg: &ExtractorConfig,
    transport: &T,
    docs: &[(String, Vec<Vec<u8>>)],
    tool_spec: &DocumentToolSpec,
) -> anyhow::Result<String> {
    let page_counts: Vec<usize> = docs.iter().map(|(_, imgs)| imgs.len()).collect();
    check_budget_gate(cfg, &page_counts)?; // CAP GATE: before ANY transport call.
    let sampling = SamplingParams {
        temperature: Some(cfg.sampling.temperature),
        effort: None,
    };
    let requests =
        build_batch_requests(docs, &cfg.models.primary, tool_spec, &sampling, cfg.sampling.n);
    transport.create_batch(&requests).await
}
```

In `crates/worker/src/bin/consensus-shadow-eval.rs`: extend `parse_args` with a `--submit`
flag and, in `main`, branch before the offline report loop:

```rust
// parse_args gains: let mut submit = false; ... "--submit" => submit = true,
// (returns an added `bool` alongside the existing (shas_file, tag) tuple).

if submit {
    let cfg = pipeline::extraction::config::ExtractorConfig::load()?;
    let budget = match cfg.require_budget() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("consensus-shadow-eval --submit: refusing — {e}");
            std::process::exit(1);
        }
    };
    println!(
        "budget OK: max_batch_tokens={} per_run_token_ceiling={}",
        budget.max_batch_tokens, budget.per_run_token_ceiling
    );
    let shas = shas_from_file(&shas_file).await?;
    let bronze = pipeline::adapter::BronzeStore::open(
        std::env::temp_dir().join("govfolio-consensus-shadow-bronze"),
    )?;
    let preprocess_cfg = pipeline::extraction::preprocess::PreprocessCfg {
        max_edge: cfg.preprocess.max_edge,
    };
    let tool_spec = us_house::consensus::consensus_tool_spec();
    let mut docs = Vec::with_capacity(shas.len());
    for sha in &shas {
        let bytes = bronze.get(&pipeline::adapter::RawDocRef { sha256: sha.clone() })?;
        let preprocessed = pipeline::extraction::preprocess::preprocess_document(&bytes, &preprocess_cfg)?;
        docs.push((sha.clone(), preprocessed.pages_png));
    }
    let transport = pipeline::extraction::HttpTransport::from_env()?;
    let batch_id =
        worker::consensus_shadow::submit_shadow_batch(&cfg, &transport, &docs, &tool_spec).await?;
    let composite_model_id = pipeline::extraction::consensus::composite_model_id(&cfg);
    worker::consensus_batch::record_batch_submitted(
        &pool,
        &batch_id,
        worker::consensus_shadow::SHADOW_REGIME,
        worker::consensus_shadow::SHADOW_TAG,
        &composite_model_id,
        &shas,
    )
    .await?;
    println!(
        "submitted shadow batch {batch_id} ({} doc(s)) — poll with: cargo run -p worker --bin consensus-batch-poll",
        shas.len()
    );
    return Ok(());
}
// ... existing offline report loop unchanged, only reached when !submit.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker --lib consensus_shadow::tests`
Expected: PASS (2 tests: H41a's confusion-matrix test + this task's budget-refusal test — the
latter proves zero `create_batch` calls). Then: `cargo fmt --check && cargo clippy
--all-targets -- -D warnings`

- [ ] **Step 5: Commit**
```bash
git add crates/worker/src/consensus_shadow.rs crates/worker/src/bin/consensus-shadow-eval.rs
git commit -m "$(cat <<'EOF'
feat(worker): shadow harness live submit arm — shadow@1/us_house_shadow isolation, HARD-CAP gate (goal 021 Phase 3 hardening, task H41b)

Adds submit_shadow_batch (rasterize -> check_budget_gate -> create_batch,
reusing Task 22's cap gate and request builder verbatim) and the
consensus-shadow-eval --submit arm. Shadow extraction_sample/extraction_cache
rows carry the isolated shadow@1 tag; extraction_batch rows carry regime_code
us_house_shadow so any consensus_row_hold review_task poll-side ingestion
opens (Task 23's resolve_document, reused unmodified) is namespaced away
from the real us_house review queue. ~$100 pilot / ~$1k full sweep
(doubles as the H45 bake-off rig); manual bin, never CI.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H42: Refill-template arm + live-smoke re-point (finding 10 refill + finding 5 mitigation)

Finding 10's refill arm: *"programmatically fill the blank paper template with known values
for checkbox ground truth at scale"* — feeds H34/H35 calibration and the H45 bake-off's
degraded stratum. Finding 5 (few-shot worked example from the committed 9115811 fixture) has
a documented consequence (design amendment A5, cited verbatim): *"the single live smoke test
targets 9115811, so once it appears in the prompt its value assertions degrade to
mechanics-only checks (schema validity, `stop_reason`); hard value assertions move to a
refill-template artifact once that arm exists (H42)."* This task builds that arm and performs
the move.

**Corpus-scaling note (H45's gate-0 precondition, goal 021 Phase 3 D4):** `paint_refill` already
takes `truth`/`font`/geometry as parameters, not literals — this task's own committed exemplar
(`filled-001.png`/`filled-001.truth.json`) is ONE fixed call, but the function itself is already
parameterized enough to generate many varied artifacts (different `RefillTruth` field values —
band letter, transaction type, dates, asset text — same deterministic paint, no RNG). H45's
bake-off degraded stratum (gate 0's `>= 150` degraded-stratum docs) is met by generating that
many varied refill artifacts at bake-off run time from a small, deterministic seed set of
`RefillTruth` value combinations (e.g. cycling the closed-vocab enum values and a handful of
asset/date strings) — NOT by committing 150 fixture files. Only `filled-001.*` is committed;
the rest are generated, never checked in.

**Files:**
- Modify: `crates/adapters/us_house/src/consensus.rs` (add `RefillTruth`, `paint_refill`, text-
  row `NormRect` consts; extend the existing `#[cfg(test)] mod tests`, Task 18's module)
- Modify: `crates/adapters/us_house/Cargo.toml` (add `imageproc` + `ab_glyph` to
  `[dependencies]`)
- Create (generated + committed by Step 3/4, not hand-authored):
  `crates/adapters/us_house/tests/fixtures/refill/filled-001.png`,
  `crates/adapters/us_house/tests/fixtures/refill/filled-001.truth.json`
- Create (committed asset, sourced not generated):
  `crates/adapters/us_house/tests/fixtures/refill/DejaVuSansMono.ttf`
- Modify: `crates/adapters/us_house/src/extractor.rs` (re-point
  `live_extraction_agrees_with_ground_truth`'s ground truth + hard assertions from the 9115811
  bronze fixture to the refill artifact)

**Interfaces:**
- Consumes: Task 18's `FormGeometry`/`RowBandGeometry` checkbox cell coordinates and `NormRect`
  (`crates/adapters/us_house/src/consensus.rs`); H35b's page-aware `consensus_spec(pages: &[
  GrayImage])` and `checkbox_sanity` (this task's live test calls both against the refill
  artifact's own page); Task 18/24 AMENDED's `LlmConsensusRow`, `band_from_column`,
  `enum_field_str` (closed-vocab rendering the live smoke compares against `RefillTruth`); the
  committed Task 25 AS AMENDED `live_extraction_agrees_with_ground_truth` test body (mechanics-
  only assertions against the 9115811 bronze fixture — this task's edit target).
- Produces: `pub struct RefillTruth { pub owner_code_raw: String, pub asset_raw: String, pub
  transaction_type_raw: String, pub transaction_date_raw: String, pub notification_date_raw:
  String, pub band_column: String, pub amount_raw: String }`; `pub fn paint_refill(page_w: u32,
  page_h: u32, row_geometry: &RowBandGeometry, truth: &RefillTruth, font: &ab_glyph::FontRef<'_>)
  -> GrayImage`; the committed `filled-001.png`/`filled-001.truth.json` fixture pair (generated,
  not hand-authored); the re-pointed `live_extraction_agrees_with_ground_truth` test body
  (repo-wide `#[ignore = "needs ANTHROPIC_API_KEY"]` count stays exactly 1).

Before writing code: source a redistributable monospace TTF (DejaVu Sans Mono — DejaVu fonts
license, based on the Bitstream Vera License, explicit redistribution/modification permitted;
official release: https://github.com/dejavu-fonts/dejavu-fonts/releases) and commit it at
`crates/adapters/us_house/tests/fixtures/refill/DejaVuSansMono.ttf`; record the source URL,
license, and retrieval date in a one-line comment atop the new code in `consensus.rs` (same
politeness/provenance discipline as regime evidence fetches, applied to a code asset). Check
crates.io for the current latest stable `0.25.x` of `imageproc` (match whatever patch digit
Task 6/Task 18 landed on — `image`/`imageproc` must stay in lockstep) and the current latest
`ab_glyph` `0.2.x`; pin exact version strings, adjusting only the patch digit from the values
below if crates.io shows newer.

- [ ] **Step 1: Write the failing test**

Append to `crates/adapters/us_house/src/consensus.rs`'s existing `#[cfg(test)] mod tests`
(Task 18's module — reuse its `fixtures_dir`-style helpers where they overlap):

```rust
// crates/adapters/us_house/src/consensus.rs — appended to the existing
// #[cfg(test)] mod tests (Task 18).

/// DejaVu Sans Mono, DejaVu fonts license (Bitstream Vera License basis,
/// redistribution/modification permitted): https://github.com/dejavu-fonts/
/// dejavu-fonts/releases, retrieved 2026-07-08, committed at
/// tests/fixtures/refill/DejaVuSansMono.ttf.
fn refill_font() -> ab_glyph::FontRef<'static> {
    static BYTES: &[u8] = include_bytes!("../tests/fixtures/refill/DejaVuSansMono.ttf");
    ab_glyph::FontRef::try_from_slice(BYTES).unwrap()
}

fn refill_fixtures_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/refill");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Regenerates the committed refill artifact pair. Guarded behind an env var
/// (mirrors Task 6's `GOVFOLIO_GENERATE_PREPROCESS_FIXTURES` idiom) so a
/// normal `cargo test` run never writes to the source tree.
#[test]
fn generate_refill_fixtures() {
    if std::env::var("GOVFOLIO_GENERATE_REFILL_FIXTURES").is_err() {
        eprintln!(
            "SKIP: generate_refill_fixtures — set GOVFOLIO_GENERATE_REFILL_FIXTURES=1 to \
             (re)write the committed refill artifact"
        );
        return;
    }
    let truth = RefillTruth {
        owner_code_raw: "SP".to_owned(),
        asset_raw: "Boeing Co Common Stock".to_owned(),
        transaction_type_raw: "P".to_owned(),
        transaction_date_raw: "1/15/2026".to_owned(),
        notification_date_raw: "1/16/2026".to_owned(),
        band_column: "B".to_owned(), // "B" == "$15,001 - $50,000" (band_from_column)
        amount_raw: "$15,001 - $50,000".to_owned(),
    };
    let geometry = fixture_2f4b2b6e().rows[0];
    let img = paint_refill(1600, 2069, &geometry, &truth, &refill_font());
    let dir = refill_fixtures_dir();
    img.save(dir.join("filled-001.png")).unwrap();
    std::fs::write(
        dir.join("filled-001.truth.json"),
        serde_json::to_string_pretty(&truth).unwrap(),
    )
    .unwrap();
}

/// The pivotal test: a painted refill artifact is USABLE checkbox ground
/// truth — `checkbox_sanity` reading it back must agree with `truth.json`'s
/// `transaction_type_raw`/`amount_raw` with ZERO violations (this is the
/// whole point of the refill arm: known values, mechanically verifiable
/// pixels). Also asserts byte-determinism (Task 6's convention).
#[test]
fn painted_refill_artifact_round_trips_through_checkbox_sanity_with_no_violations() {
    let dir = refill_fixtures_dir();
    let truth: RefillTruth =
        serde_json::from_str(&std::fs::read_to_string(dir.join("filled-001.truth.json")).unwrap())
            .unwrap();
    let page = image::open(dir.join("filled-001.png")).unwrap().into_luma8();
    let geometry = fixture_2f4b2b6e();

    let sanity = checkbox_sanity(&[page], &geometry);
    let row = serde_json::json!({
        "transaction_type_raw": truth.transaction_type_raw,
        "band_column": truth.band_column,
    });
    assert!(sanity(&row).is_empty(), "refill artifact must self-agree with its own truth.json");

    let font = refill_font();
    let first = paint_refill(1600, 2069, &fixture_2f4b2b6e().rows[0], &truth, &font);
    let second = paint_refill(1600, 2069, &fixture_2f4b2b6e().rows[0], &truth, &font);
    assert_eq!(first, second, "paint_refill must be byte-deterministic for identical truth+font");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p us_house --lib consensus::tests::painted_refill_artifact_round_trips_through_checkbox_sanity_with_no_violations`
Expected: FAIL to compile — `RefillTruth`, `paint_refill` do not exist yet; `imageproc`/
`ab_glyph` are not yet dependencies of `us_house`; `tests/fixtures/refill/` has no committed
`.png`/`.json`/`.ttf` yet.

- [ ] **Step 3: Write minimal implementation**

**3a. `crates/adapters/us_house/Cargo.toml`**, in `[dependencies]` alongside the `image` line
Task 18 added:

```toml
imageproc = "0.25.8"
ab_glyph = "0.2.29"
```

**3b. `crates/adapters/us_house/src/consensus.rs`** — append below `fixture_2f4b2b6e`:

```rust
use std::path::PathBuf;

/// Ground truth for one refill artifact (`tests/fixtures/refill/
/// filled-001.truth.json`). Field names/shape are the SAME six pointers
/// `consensus_spec().critical_fields` compares, so the live smoke's hard
/// assertions (H42) compare like-for-like, and `checkbox_sanity` can be run
/// directly against a `serde_json::json!` of the checkbox-relevant two.
/// `band_column` (letter `A`..`J`) is the ACTUAL ground truth compared
/// against the live consensus row's `LlmConsensusRow.band_column` (Task 18's
/// closed-vocab DTO, Task 24 §3f AMENDED) via `band_from_column`;
/// `amount_raw` is a DERIVED verbatim label (`band_from_column(band_column)`,
/// kept in sync by hand in the committed truth.json) used only by
/// `paint_refill`'s cell-selection logic below and by `checkbox_sanity`'s
/// pre-`LlmConsensusRow` `serde_json::json!` convention.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RefillTruth {
    pub owner_code_raw: String,
    pub asset_raw: String,
    pub transaction_type_raw: String,
    pub transaction_date_raw: String,
    pub notification_date_raw: String,
    pub band_column: String,
    pub amount_raw: String,
}

/// Free-text row placements for the refill painter — deliberately NOT part
/// of `FormGeometry` (Task 18's struct stays checkbox-only). Calibrated
/// against the same 1600px-target-edge canvas `fixture_2f4b2b6e` assumes.
const ASSET_TEXT_ROW: NormRect = NormRect { x: 0.06, y: 0.34, w: 0.45, h: 0.05 };
const DATE_TEXT_ROW: NormRect = NormRect { x: 0.06, y: 0.55, w: 0.20, h: 0.05 };
const NOTIFICATION_DATE_TEXT_ROW: NormRect = NormRect { x: 0.30, y: 0.55, w: 0.20, h: 0.05 };

/// Paints one refill artifact: a `page_w`x`page_h` white canvas with the
/// checkbox cells in `row_geometry` filled per `truth`'s `transaction_type_raw`
/// /`amount_raw` (same cell-reading convention `checkbox_sanity` uses — see
/// the round-trip test), and `truth`'s free-text fields typed into the fixed
/// text rows via `font`. Deterministic: identical inputs produce a
/// byte-identical `GrayImage`.
#[must_use]
pub fn paint_refill(
    page_w: u32,
    page_h: u32,
    row_geometry: &RowBandGeometry,
    truth: &RefillTruth,
    font: &ab_glyph::FontRef<'_>,
) -> GrayImage {
    let mut img = GrayImage::from_pixel(page_w, page_h, image::Luma([255u8]));

    let type_cells: [(&str, NormRect); 4] = [
        ("P", row_geometry.type_p),
        ("S", row_geometry.type_s),
        ("S (partial)", row_geometry.type_s_partial),
        ("E", row_geometry.type_e),
    ];
    for (token, rect) in type_cells {
        draw_cell(&mut img, page_w, page_h, rect, token == truth.transaction_type_raw);
    }
    for ((label, _, _), rect) in BANDS.iter().zip(row_geometry.bands.iter()) {
        draw_cell(&mut img, page_w, page_h, *rect, *label == truth.amount_raw);
    }

    draw_text_row(&mut img, page_w, page_h, ASSET_TEXT_ROW, &truth.asset_raw, font);
    draw_text_row(&mut img, page_w, page_h, DATE_TEXT_ROW, &truth.transaction_date_raw, font);
    draw_text_row(
        &mut img,
        page_w,
        page_h,
        NOTIFICATION_DATE_TEXT_ROW,
        &truth.notification_date_raw,
        font,
    );
    img
}

fn draw_cell(img: &mut GrayImage, page_w: u32, page_h: u32, rect: NormRect, checked: bool) {
    let x = (rect.x * page_w as f32).round() as i32;
    let y = (rect.y * page_h as f32).round() as i32;
    let w = ((rect.w * page_w as f32).round() as u32).max(1);
    let h = ((rect.h * page_h as f32).round() as u32).max(1);
    imageproc::drawing::draw_hollow_rect_mut(
        img,
        imageproc::rect::Rect::at(x, y).of_size(w, h),
        image::Luma([0u8]),
    );
    if checked {
        let pad = 2i32;
        imageproc::drawing::draw_filled_rect_mut(
            img,
            imageproc::rect::Rect::at(x + pad, y + pad)
                .of_size(w.saturating_sub(4).max(1), h.saturating_sub(4).max(1)),
            image::Luma([0u8]),
        );
    }
}

fn draw_text_row(
    img: &mut GrayImage,
    page_w: u32,
    page_h: u32,
    rect: NormRect,
    text: &str,
    font: &ab_glyph::FontRef<'_>,
) {
    let x = (rect.x * page_w as f32).round() as i32;
    let y = (rect.y * page_h as f32).round() as i32;
    let scale = ab_glyph::PxScale::from(rect.h * page_h as f32 * 0.8);
    imageproc::drawing::draw_text_mut(img, image::Luma([0u8]), x, y, scale, font, text);
}
```

Run once locally with `GOVFOLIO_GENERATE_REFILL_FIXTURES=1 cargo test -p us_house --lib
consensus::tests::generate_refill_fixtures -- --ignored --nocapture` (drop `--ignored` if the
test is not itself `#[ignore]`d — it is not; the env-var check makes that unnecessary) to
produce `filled-001.png`/`filled-001.truth.json`, then `git add` them (Step 5).

**3c. Repoint the live test — `crates/adapters/us_house/src/extractor.rs`.** Replace the
`#[ignore = "needs ANTHROPIC_API_KEY"] async fn live_extraction_agrees_with_ground_truth` test
body (H32 already amended its `extract()` call to the 4-arg `(pdf_bytes, spec, sanity, pixel)`
shape; this task changes only its INPUT SOURCE and assertions):

```rust
#[tokio::test]
#[ignore = "needs ANTHROPIC_API_KEY"]
async fn live_extraction_agrees_with_ground_truth() {
    if std::env::var_os("ANTHROPIC_API_KEY").is_none() {
        eprintln!("ANTHROPIC_API_KEY absent — skipping the live extraction test");
        return;
    }
    // H42: re-pointed from the 9115811 bronze fixture to the refill artifact
    // (finding 5's documented consequence, design amendment A5: 9115811's
    // own transcription now rides the prompt as the few-shot example, so
    // its live assertions degraded to mechanics-only; the refill artifact
    // restores HARD value assertions because its ground truth was never fed
    // to the model).
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/refill/filled-001.png"
    ))
    .unwrap();
    let truth: crate::consensus::RefillTruth = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/refill/filled-001.truth.json"
        ))
        .unwrap(),
    )
    .unwrap();

    let cfg = ExtractorConfig::load().unwrap();
    let transport = HttpTransport::from_env().unwrap();
    let geometry = crate::consensus::fixture_2f4b2b6e();
    let page = image::load_from_memory(&bytes).unwrap().to_luma8();
    // H35b's page-aware consensus_spec(pages) — the refill artifact's own
    // page (2026-fingerprint-recognized, since it's painted onto Task 18's
    // real FormGeometry), so this smoke exercises the SAME table_regions/
    // template_recognized wiring a live extraction actually uses.
    let spec = crate::consensus::consensus_spec(&[page.clone()]);
    let sanity_closure = crate::consensus::checkbox_sanity(&[page], &geometry);
    let sanity: pipeline::extraction::consensus::SanityCheck<'_> = &sanity_closure;
    // Trivial Clear pixel signal: this smoke targets the consensus pipeline
    // against the refill ground truth, not H32's pixel-ambiguity trigger
    // (that path has its own coverage from Task 18/H32's tests).
    let pixel: pipeline::extraction::consensus::PixelSignal<'_> =
        &|_row| pipeline::extraction::consensus::PixelVerdict::Clear;

    let extractor = ConsensusExtractor::new(&transport, &cfg);
    let outcome = extractor.extract(&bytes, &spec, sanity, pixel).await.unwrap();

    if !outcome.held.is_empty() {
        eprintln!(
            "live consensus HELD {} row(s) against refill ground truth — reporting, not failing: {:?}",
            outcome.held.len(),
            outcome.held
        );
    }
    assert!(!outcome.published.is_empty(), "the refill artifact's one row must publish");
    // Task 18/Task 24 §3f AMENDED: published rows deserialize as the strict
    // closed-vocab `LlmConsensusRow`, not the frozen v1 `LlmTransactionRow`.
    // Render its enum fields back to verbatim strings via the SAME helpers
    // `silver_rows` (H36) uses, so this smoke compares like-for-like against
    // `RefillTruth`'s free-string fields.
    let live_row: crate::consensus::LlmConsensusRow =
        serde_json::from_value(outcome.published[0].row.clone()).unwrap();
    assert_eq!(enum_field_str(&live_row.transaction_type_raw).unwrap(), truth.transaction_type_raw);
    assert_eq!(crate::consensus::band_from_column(live_row.band_column), truth.amount_raw);
    assert_eq!(live_row.transaction_date_raw, truth.transaction_date_raw);
}
```

Then grep to confirm the repo-wide live-test budget is untouched:

```bash
grep -rn 'ignore = "needs ANTHROPIC_API_KEY"' --include=*.rs .
```

Expected: exactly one match, at this test (same location as Task 25 landed it — only the BODY
changed in this task, not the count).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p us_house --lib consensus::tests`
Expected: PASS, including `painted_refill_artifact_round_trips_through_checkbox_sanity_with_no_violations`.

Run: `cargo test -p us_house`
Expected: PASS (the re-pointed live test self-skips without `ANTHROPIC_API_KEY`, per its own
guard — same behavior as before this task).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**
```bash
git add crates/adapters/us_house/src/consensus.rs \
        crates/adapters/us_house/src/extractor.rs \
        crates/adapters/us_house/Cargo.toml \
        crates/adapters/us_house/tests/fixtures/refill/
git commit -m "$(cat <<'EOF'
feat(us_house): refill-template checkbox ground truth + live-smoke re-point (goal 021 Phase 3 hardening, task H42)

Adds paint_refill (paints known transaction_type_raw/amount_raw checkbox
marks plus free-text fields onto a white canvas from Task 18's FormGeometry)
and the committed filled-001.png/truth.json pair, generated via the
GOVFOLIO_GENERATE_REFILL_FIXTURES-gated idiom (Task 6 style) — checkbox
ground truth at scale for H35/H34 calibration and the H45 bake-off's
degraded stratum. Re-points the sole #[ignore = "needs ANTHROPIC_API_KEY"]
live test from the 9115811 bronze fixture (whose transcription now rides
the prompt as finding 5's few-shot example, degrading its own live
assertions to mechanics-only per design amendment A5) to this refill
artifact, restoring hard band/type/date value assertions. Repo-wide
ignore-live-test count stays exactly 1.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H43a: Error-boundary fixture corpus + offline re-scorer (finding 17)

Finding 17 (verbatim): *"Error-boundary fixture corpus with a REAL regression gate. Every
audit-confirmed published error becomes a committed fixture; the gate for model/prompt/N bumps
is a key-gated LIVE eval bin over that corpus (outside CI — conformance's primed-cache gate is
green by construction and can't catch model regressions); policy-only bumps re-score stored
extraction_sample payloads offline. Defined pass semantics + flake policy."* This task builds
the corpus convention, seeds its first case, and the OFFLINE half of the gate. H43b adds the
KEY-gated LIVE half to the same bin.

Labeled assumption (non-blocking, ambiguity protocol — CLAUDE.md §1, goal §0): every live
consensus pass, sync (`ConsensusExtractor::extract`) or batch (`ingest_batch_results`, Task
23), persists its sample passes to `extraction_sample` keyed `(document_sha256, consensus_tag,
pass_idx)` — the system's only durable per-pass audit trail, which H38/H39's stratified audit
and error-class labeling both depend on existing. This task's offline re-scorer therefore reads
through `worker::consensus_batch::load_samples` regardless of which path produced the rows. If
this does not hold when H43a is actually executed (verify against the then-current
`ConsensusExtractor::extract` call sites first), that is a blocking halt for this task, not a
silent reinterpretation.

**Files:**
- Create: `crates/worker/src/consensus_eval.rs`
- Create: `crates/worker/src/bin/consensus-live-eval.rs` (`--offline` mode only in this task)
- Modify: `crates/worker/src/lib.rs` (register `pub mod consensus_eval;`)
- Create: `crates/adapters/us_house/tests/fixtures/error_boundary/MANIFEST.json`
- Create: `crates/adapters/us_house/tests/fixtures/error_boundary/band-2v1-minority-premium-hold/{input.png, expected.json, provenance.md}`
- Test: `crates/worker/src/consensus_eval.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `worker::consensus_shadow::{route_document, OutcomeBucket}` (H41a — the offline
  re-scorer reuses the EXACT same comparator wiring the shadow harness uses; no local
  reimplementation); `worker::consensus_batch::load_samples` (Task 23); `us_house::consensus::
  consensus_spec()`.
- Produces: `struct CaseManifestEntry { pub slug: String, pub error_class: String, pub
  expected_outcome: String, pub input: String, pub expected: String, pub provenance: String }`;
  `struct ExpectedOutcome { pub document_sha256: String, pub consensus_tag: String, pub
  outcome: String, pub published: Option<Value> }` (`outcome` ∈ `{"agreed", "escalated",
  "capped", "hold"}`, a closed set validated at load); `struct RescoreResult { pub slug:
  String, pub actual_outcome: OutcomeBucket, pub actual_published: Option<Value>, pub
  matches_expected: bool }`; `fn load_manifest(corpus_root: &Path) ->
  anyhow::Result<Vec<CaseManifestEntry>>`; `fn load_expected(corpus_root: &Path, entry:
  &CaseManifestEntry) -> anyhow::Result<ExpectedOutcome>`; `fn rescore(slug: &str, samples:
  &[Value], premium: Option<&Value>, expected: &ExpectedOutcome, spec: &ConsensusSpec) ->
  anyhow::Result<RescoreResult>` (pure, zero I/O); `async fn rescore_corpus(pool: &PgPool,
  corpus_root: &Path, spec: &ConsensusSpec) -> anyhow::Result<Vec<RescoreOrSkip>>` (DB-touching
  wrapper); bin `consensus-live-eval --offline`.

- [ ] **Step 1: Write the failing test**

Create `crates/worker/src/consensus_eval.rs`:

```rust
//! Error-boundary regression gate (goal 021 Phase 3 hardening, finding 17 /
//! design amendment A17): every audit-confirmed published error becomes a
//! committed fixture under `crates/adapters/us_house/tests/fixtures/
//! error_boundary/<case-slug>/`. This module's `rescore` is the OFFLINE half
//! of the gate for POLICY-ONLY bumps — it re-runs the REAL comparator
//! (`worker::consensus_shadow::route_document`, the same fn H41a's shadow
//! harness uses) over frozen samples with NO network calls. H43b adds the
//! KEY-gated LIVE half (a fresh model call per case) to the same bin.

use std::path::Path;

use anyhow::Context as _;
use serde_json::Value;
use sqlx::PgPool;

use crate::consensus_shadow::{OutcomeBucket, route_document};
use pipeline::extraction::consensus::ConsensusSpec;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(band_column: &str) -> Value {
        json!({
            "owner_code_raw": "SP",
            "asset_raw": "Boeing Co Common Stock",
            "transaction_type_raw": "S",
            "transaction_date_raw": "2026-02-10",
            "notification_date_raw": "2026-02-11",
            "band_column": band_column,
        })
    }
    fn payload(band_column: &str) -> Value {
        json!({ "rows": [row(band_column)] })
    }

    /// Mirrors the committed corpus case `band-2v1-minority-premium-hold`
    /// (Step 3): 2 samples agree on a WRONG band, 1 sample + the premium
    /// agree on the CORRECT band — 2-of-4 vs 2-of-4, no ≥3-of-4 majority
    /// (H29/A1/A14) — a pol1-era plurality comparator would have PUBLISHED
    /// the 2-vote wrong band at 0.75 (documented in the corpus case's
    /// `provenance.md`, the real audit finding); the CURRENT (pol2)
    /// comparator holds it instead. This is the flip the offline re-scorer
    /// exists to keep detecting on every future policy-only bump.
    #[test]
    fn rescorer_reports_the_pol1_to_pol2_flip_from_publish_to_hold() {
        // H35b's page-aware consensus_spec(pages) — a blank placeholder page
        // is fine here: `rescore` compares stored JSON payloads only, never
        // touches table_regions/template_recognized.
        let blank_page = image::GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
        let spec = us_house::consensus::consensus_spec(&[blank_page]);
        const WRONG: &str = "C";
        const CORRECT: &str = "B";
        let samples = vec![payload(WRONG), payload(WRONG), payload(CORRECT)];
        let premium = payload(CORRECT);
        let expected = ExpectedOutcome {
            document_sha256: "0".repeat(64),
            consensus_tag: "error_boundary_seed@1".to_owned(),
            outcome: "hold".to_owned(),
            published: None,
        };

        let result = rescore(
            "band-2v1-minority-premium-hold",
            &samples,
            Some(&premium),
            &expected,
            &spec,
        )
        .unwrap();

        assert_eq!(result.actual_outcome, OutcomeBucket::Held);
        assert_eq!(result.actual_published, None);
        assert!(
            result.matches_expected,
            "current (pol2) comparator must hold this case, matching expected.json — a \
             pol1-era plurality comparator would have published {WRONG:?} at 0.75 instead"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker --lib consensus_eval::tests`
Expected: FAIL to compile — `ExpectedOutcome`, `rescore` do not exist yet, and
`consensus_eval` is not yet registered in `lib.rs`.

- [ ] **Step 3: Write minimal implementation**

**3a.** `crates/worker/src/lib.rs`: add `pub mod consensus_eval;` (alphabetical among the
existing `pub mod` lines; extend the top doc comment: `; the error-boundary regression gate
(goal 021 Phase 3 hardening, finding 17) in [`consensus_eval`]`).

**3b.** Append to `crates/worker/src/consensus_eval.rs` (above the `#[cfg(test)]` module):

```rust
/// One `MANIFEST.json` entry — the corpus index.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CaseManifestEntry {
    pub slug: String,
    pub error_class: String,
    pub expected_outcome: String,
    pub input: String,
    pub expected: String,
    pub provenance: String,
}

/// One case's `expected.json` — the authoritative comparison target.
/// `outcome` is a closed set (`"agreed" | "escalated" | "capped" | "hold"`),
/// validated in [`load_expected`].
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub struct ExpectedOutcome {
    pub document_sha256: String,
    pub consensus_tag: String,
    pub outcome: String,
    pub published: Option<Value>,
}

const CLOSED_OUTCOMES: [&str; 4] = ["agreed", "escalated", "capped", "hold"];

/// Reads the corpus `MANIFEST.json`.
///
/// # Errors
/// I/O or JSON failure (fail closed — a corrupt corpus index must be loud).
pub fn load_manifest(corpus_root: &Path) -> anyhow::Result<Vec<CaseManifestEntry>> {
    let text = std::fs::read_to_string(corpus_root.join("MANIFEST.json"))
        .with_context(|| format!("reading {}/MANIFEST.json", corpus_root.display()))?;
    serde_json::from_str(&text).context("MANIFEST.json is not a Vec<CaseManifestEntry>")
}

/// Reads one case's `expected.json`, validating `outcome` against the closed
/// set.
///
/// # Errors
/// I/O/JSON failure, or an `outcome` outside `{agreed, escalated, capped,
/// hold}` (fail closed).
pub fn load_expected(
    corpus_root: &Path,
    entry: &CaseManifestEntry,
) -> anyhow::Result<ExpectedOutcome> {
    let text = std::fs::read_to_string(corpus_root.join(&entry.expected))
        .with_context(|| format!("reading {}", entry.expected))?;
    let expected: ExpectedOutcome =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", entry.expected))?;
    anyhow::ensure!(
        CLOSED_OUTCOMES.contains(&expected.outcome.as_str()),
        "{}: outcome {:?} is outside the closed set {CLOSED_OUTCOMES:?}",
        entry.expected,
        expected.outcome
    );
    Ok(expected)
}

fn outcome_label(outcome: OutcomeBucket) -> &'static str {
    match outcome {
        OutcomeBucket::Agreed090 => "agreed",
        OutcomeBucket::Escalated075 => "escalated",
        OutcomeBucket::Capped079 => "capped",
        OutcomeBucket::Held => "hold",
    }
}

/// One case's re-score result.
#[derive(Debug, Clone, PartialEq)]
pub struct RescoreResult {
    pub slug: String,
    pub actual_outcome: OutcomeBucket,
    pub actual_published: Option<Value>,
    pub matches_expected: bool,
}

/// Re-runs align/score/route (via `consensus_shadow::route_document` — no
/// local reimplementation) over one case's ALREADY-IN-HAND samples and
/// compares against `expected`. Zero I/O — pure over `Value`s.
///
/// # Errors
/// `route_document` failure (malformed sample payload), or a case whose
/// single-row assumption does not hold (this task's corpus cases are all
/// single-row; multi-row cases are a documented future extension, not
/// silently mishandled here).
pub fn rescore(
    slug: &str,
    samples: &[Value],
    premium: Option<&Value>,
    expected: &ExpectedOutcome,
    spec: &ConsensusSpec,
) -> anyhow::Result<RescoreResult> {
    let routed = route_document(samples, premium, spec)?;
    let row = routed
        .into_iter()
        .next()
        .with_context(|| format!("{slug}: corpus case produced no rows"))?;
    let matches_expected =
        outcome_label(row.outcome) == expected.outcome && row.published == expected.published;
    Ok(RescoreResult {
        slug: slug.to_owned(),
        actual_outcome: row.outcome,
        actual_published: row.published,
        matches_expected,
    })
}

/// Result of trying to re-score one corpus case against stored
/// `extraction_sample` rows: scored, or skipped because no live capture has
/// happened for it yet (a legitimate, expected state for a freshly-added
/// case — not a failure).
pub enum RescoreOrSkip {
    Scored(RescoreResult),
    Skipped { slug: String },
}

/// DB-touching wrapper: reads every corpus case's `expected.json`, loads its
/// stored `extraction_sample` rows (`document_sha256`/`consensus_tag` from
/// `expected.json`) via `worker::consensus_batch::load_samples`, identifies
/// the escalation pass (if any) by `model_id`, and calls [`rescore`]. A case
/// with fewer than 3 stored samples reports [`RescoreOrSkip::Skipped`], not
/// an error.
///
/// # Errors
/// Corpus/manifest load failure, or database failure.
pub async fn rescore_corpus(
    pool: &PgPool,
    corpus_root: &Path,
    escalation_model: &str,
    spec: &ConsensusSpec,
) -> anyhow::Result<Vec<RescoreOrSkip>> {
    let manifest = load_manifest(corpus_root)?;
    let mut results = Vec::with_capacity(manifest.len());
    for entry in &manifest {
        let expected = load_expected(corpus_root, entry)?;
        let stored = crate::consensus_batch::load_samples(
            pool,
            &expected.document_sha256,
            &expected.consensus_tag,
        )
        .await?;
        if stored.len() < 3 {
            results.push(RescoreOrSkip::Skipped {
                slug: entry.slug.clone(),
            });
            continue;
        }
        let premium = stored.iter().find(|s| s.model_id == escalation_model);
        let samples: Vec<Value> = stored
            .iter()
            .filter(|s| s.model_id != escalation_model)
            .map(|s| s.payload.clone())
            .collect();
        let result = rescore(
            &entry.slug,
            &samples,
            premium.map(|p| &p.payload),
            &expected,
            spec,
        )?;
        results.push(RescoreOrSkip::Scored(result));
    }
    Ok(results)
}
```

**3c.** Create the corpus. `crates/adapters/us_house/tests/fixtures/error_boundary/
MANIFEST.json`:

```json
[
  {
    "slug": "band-2v1-minority-premium-hold",
    "error_class": "band_misread",
    "expected_outcome": "hold",
    "input": "band-2v1-minority-premium-hold/input.png",
    "expected": "band-2v1-minority-premium-hold/expected.json",
    "provenance": "band-2v1-minority-premium-hold/provenance.md"
  }
]
```

`crates/adapters/us_house/tests/fixtures/error_boundary/band-2v1-minority-premium-hold/
expected.json` (procedure: compute `sha256sum input.png`, the same file copied below, and
record it as `document_sha256`):

```json
{
  "document_sha256": "<sha256 of this case's input.png, computed once at commit time>",
  "consensus_tag": "error_boundary_seed@1",
  "outcome": "hold",
  "published": null
}
```

`.../band-2v1-minority-premium-hold/provenance.md`:

```markdown
# Case: band-2v1-minority-premium-hold

**Error class:** `band_misread`

**Audit finding:** a pol1-era plurality-of-3 comparator published `$50,001 - $100,000` at
CONF_ESCALATED (0.75) on a row where 2 of 3 sampled passes agreed on that band and 1
disagreed with `$15,001 - $50,000`; the escalation premium pass sided with the 1-sample
minority. Under A1/A14 (strict ≥3-of-4 majority over samples+premium, true vote
multiplicity), this is 2-of-4 for EITHER value — no majority — so the row must HOLD, not
publish either band. `expected.json`'s `outcome: "hold"` records the corrected behavior;
this file records the WRONG historical behavior it replaces, for the offline re-scorer
(`consensus-eval::rescore`) to keep catching a regression back to it.

**Seed status:** `input.png` is a synthetic refill-style artifact (`us_house::consensus::
paint_refill`, Task H42), not yet run through a real live consensus pass — `expected.json`'s
`document_sha256`/`consensus_tag` point at where a `consensus-live-eval --live` (H43b) or
`consensus-shadow-eval --submit` (H41b) capture would store its `extraction_sample` rows.
Until that capture happens, `consensus-live-eval --offline` reports this case SKIPPED (not
failing) — the pivotal unit test in `consensus_eval.rs` exercises the comparator logic
directly against literals mirroring this case, independent of that capture.
```

For `input.png`: reuse H42's refill painter with the CORRECT ground truth (a legitimate
document image; the WRONG samples above are what a historical model run produced from it, not
what is painted on it):

```bash
# one-off, from a scratch bin/test — paints and copies alongside filled-001.png's own
# generation (Step 3b of H42); truth = { transaction_type_raw: "S", amount_raw: "$15,001 -
# $50,000", asset_raw: "Boeing Co Common Stock", transaction_date_raw: "2/10/2026",
# notification_date_raw: "2/11/2026", owner_code_raw: "SP" }
cp crates/adapters/us_house/tests/fixtures/refill/filled-001.png \
   crates/adapters/us_house/tests/fixtures/error_boundary/band-2v1-minority-premium-hold/input.png
sha256sum crates/adapters/us_house/tests/fixtures/error_boundary/band-2v1-minority-premium-hold/input.png
# paste the printed hash into expected.json's document_sha256
```

Create `crates/worker/src/bin/consensus-live-eval.rs` (`--offline` mode only; H43b adds
`--live`):

```rust
//! Error-boundary regression gate (goal 021 Phase 3 hardening, finding 17 /
//! design amendment A17): the gate for model/prompt/N bumps. Conformance's
//! extraction cache is primed MECHANICALLY from `expected.silver.json`
//! ground truth — it is green by construction and can never observe a real
//! model regression, because it never calls a model. This bin is the only
//! thing in the repo that does (in its `--live` mode, H43b), on a schedule
//! the operator controls. NEVER wired into any CI/command-chain gate.
//!
//! This file's `--offline` mode (this task, H43a): re-scores every corpus
//! case's ALREADY-STORED `extraction_sample` payloads against
//! `expected.json` — the gate for POLICY-ONLY bumps, zero API calls. `--live`
//! (H43b) makes fresh model calls and is ANTHROPIC_API_KEY-gated; this mode
//! is not.
//!
//! Usage: `cargo run -p worker --bin consensus-live-eval -- --offline`
//! Env: `DATABASE_URL` (required).

use anyhow::Context as _;

use worker::consensus_eval::{RescoreOrSkip, rescore_corpus};

const CORPUS_ROOT: &str =
    "crates/adapters/us_house/tests/fixtures/error_boundary";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mode = std::env::args().nth(1).context("expected --offline (or --live, H43b)")?;
    anyhow::ensure!(mode == "--offline", "unknown mode {mode:?} (this build only supports --offline — see H43b for --live)");

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url).await.context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool).await.context("applying migrations")?;

    let cfg = pipeline::extraction::config::ExtractorConfig::load()?;
    // H35b's page-aware consensus_spec(pages) — the offline re-scorer works
    // purely from ALREADY-STORED extraction_sample payloads (no page bytes
    // persisted alongside them), so a blank placeholder page is the correct
    // choice here: template_recognized/table_regions are irrelevant to
    // re-scoring a stored JSON payload through the comparator.
    let blank_page = image::GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
    let spec = us_house::consensus::consensus_spec(&[blank_page]);
    let corpus_root = std::path::Path::new(CORPUS_ROOT);
    let results = rescore_corpus(&pool, corpus_root, &cfg.models.escalation, &spec).await?;

    let mut failed = 0usize;
    for result in &results {
        match result {
            RescoreOrSkip::Scored(r) if r.matches_expected => {
                println!("PASS  {} -> {:?}", r.slug, r.actual_outcome);
            }
            RescoreOrSkip::Scored(r) => {
                failed += 1;
                println!(
                    "FAIL  {} -> {:?} published={:?} (expected mismatch)",
                    r.slug, r.actual_outcome, r.actual_published
                );
            }
            RescoreOrSkip::Skipped { slug } => {
                println!("SKIP  {slug} (no stored extraction_sample rows yet — run --live once)");
            }
        }
    }
    println!("consensus-live-eval --offline: {} case(s), {failed} FAIL", results.len());
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker --lib consensus_eval::tests`
Expected: PASS (1 test — the pol1→pol2 flip). Then: `cargo fmt --check && cargo clippy
--all-targets -- -D warnings`

- [ ] **Step 5: Commit**
```bash
git add crates/worker/src/consensus_eval.rs \
        crates/worker/src/bin/consensus-live-eval.rs \
        crates/worker/src/lib.rs \
        crates/adapters/us_house/tests/fixtures/error_boundary/
git commit -m "$(cat <<'EOF'
feat(worker): error-boundary fixture corpus + offline policy-bump re-scorer (goal 021 Phase 3 hardening, task H43a)

Adds the crates/adapters/us_house/tests/fixtures/error_boundary/<slug>/
corpus convention (input.png + expected.json + provenance.md, indexed by
MANIFEST.json) and worker::consensus_eval::rescore, which reuses H41a's
route_document to re-run align/score/route over a case's stored
extraction_sample payloads with zero API calls — the gate for policy-only
bumps. Seeds the corpus's first case, band-2v1-minority-premium-hold, a real
pol1-era published-wrong-band defect that the current (pol2) ≥3-of-4
majority rule now holds instead. consensus-live-eval --offline is this
gate's entry point; --live (H43b) is the separate, key-gated regression gate
for model/prompt/N bumps.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H43b: Key-gated live eval bin (finding 17)

`consensus-live-eval --live`: the KEY-gated half of finding 17's regression gate — a fresh
model call per corpus case, run over the FULL consensus path (preprocess → samples → premium
exactly where H32's trigger disjunction fires), asserted against `expected.json`. This is the
ONLY thing in the repo that can observe a real model/prompt/N regression, because conformance's
cache is primed mechanically and never calls a model (state this explicitly, per finding 17).
Key-gated, manual, NEVER CI — it is a BIN, not a test, so it does not touch, wrap, or grow the
repo's `#[ignore = "needs ANTHROPIC_API_KEY"]` count, which stays exactly 1 (H42).

**Files:**
- Modify: `crates/worker/src/consensus_eval.rs` (add `run_case_live`, `LiveCaseResult`, flake
  retry policy)
- Modify: `crates/worker/src/bin/consensus-live-eval.rs` (add the `--live` mode)
- Test: `crates/worker/src/consensus_eval.rs` (inline `#[cfg(test)] mod tests`, appended —
  flake-policy classification only; the network call itself is not unit-testable offline, so
  it is exercised by the bin at manual run time, never by `cargo test`)

**Interfaces:**
- Consumes: `pipeline::extraction::ConsensusExtractor` (H32's 4-arg `extract(&self, pdf_bytes,
  spec, sanity, pixel)`); `pipeline::extraction::{HttpTransport, Transport}`;
  `pipeline::extraction::config::ExtractorConfig`; `us_house::consensus::{consensus_spec,
  checkbox_sanity, fixture_2f4b2b6e}`; `worker::consensus_eval::{load_manifest,
  load_expected}` (H43a).
- Produces: `enum CaseVerdict { Pass, Flaky, Fail(String) }` (`Debug, Clone, PartialEq`);
  `struct LiveCaseResult { pub slug: String, pub verdict: CaseVerdict }`; `fn
  classify_live_attempts(attempt1: bool, attempt2: Option<bool>) -> CaseVerdict` (pure — the
  flake policy, unit-tested); `async fn run_case_live<T: Transport>(transport: &T, cfg:
  &ExtractorConfig, input_bytes: &[u8], expected: &ExpectedOutcome, spec: &ConsensusSpec) ->
  anyhow::Result<bool>` (one attempt: true iff the published/held outcome matches `expected`
  at the expected confidence); bin `consensus-live-eval --live`.

- [ ] **Step 1: Write the failing test**

Append to `crates/worker/src/consensus_eval.rs`'s `#[cfg(test)] mod tests`:

```rust
/// The flake policy (finding 17): a FAILing case re-runs ONCE; pass on
/// rerun is FLAKY (reported, not failing); two failures is FAIL. A pass on
/// the FIRST attempt never even attempts a rerun.
#[test]
fn flake_policy_classifies_attempts_correctly() {
    assert_eq!(classify_live_attempts(true, None), CaseVerdict::Pass);
    assert_eq!(classify_live_attempts(false, Some(true)), CaseVerdict::Flaky);
    assert_eq!(
        classify_live_attempts(false, Some(false)),
        CaseVerdict::Fail("two failures".to_owned())
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p worker --lib consensus_eval::tests::flake_policy_classifies_attempts_correctly`
Expected: FAIL to compile — `classify_live_attempts`, `CaseVerdict` do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Append to `crates/worker/src/consensus_eval.rs`:

```rust
use pipeline::extraction::consensus::policy;

/// One case's outcome under the finding-17 flake policy.
#[derive(Debug, Clone, PartialEq)]
pub enum CaseVerdict {
    Pass,
    Flaky,
    Fail(String),
}

/// Pure flake-policy classification: `attempt1` is the first run's
/// pass/fail; `attempt2` is `None` if no rerun was needed (first attempt
/// passed) or `Some(pass/fail)` after a rerun. A FAILing case re-runs
/// EXACTLY once; pass-on-rerun is FLAKY, not FAIL; two failures is FAIL.
#[must_use]
pub fn classify_live_attempts(attempt1: bool, attempt2: Option<bool>) -> CaseVerdict {
    match (attempt1, attempt2) {
        (true, _) => CaseVerdict::Pass,
        (false, Some(true)) => CaseVerdict::Flaky,
        (false, Some(false)) => CaseVerdict::Fail("two failures".to_owned()),
        (false, None) => CaseVerdict::Fail("no rerun attempted".to_owned()),
    }
}

/// One case's live result.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveCaseResult {
    pub slug: String,
    pub verdict: CaseVerdict,
}

/// One live attempt over one corpus case: runs the FULL consensus path
/// (`ConsensusExtractor::extract`, H32's `(pdf_bytes, spec, sanity, pixel)`
/// signature) and reports pass/fail per finding 17's pass semantics — PASS
/// iff every field in `expected.published` matches the row's published
/// value at the expected policy_v1 confidence literal, OR `expected.outcome
/// == "hold"` and the row genuinely holds.
///
/// # Errors
/// Transport/schema failure from the live call (a hard error, distinct from
/// a normal FAIL verdict — the caller treats a transport error as its own
/// attempt failure, still subject to the one-rerun flake policy).
pub async fn run_case_live<T: pipeline::extraction::Transport>(
    transport: &T,
    cfg: &pipeline::extraction::config::ExtractorConfig,
    input_bytes: &[u8],
    expected: &ExpectedOutcome,
    spec: &ConsensusSpec,
) -> anyhow::Result<bool> {
    let geometry = us_house::consensus::fixture_2f4b2b6e();
    let page = image::load_from_memory(input_bytes)
        .context("decoding corpus case input")?
        .to_luma8();
    let sanity_closure = us_house::consensus::checkbox_sanity(&[page], &geometry);
    let sanity: pipeline::extraction::consensus::SanityCheck<'_> = &sanity_closure;
    let pixel: pipeline::extraction::consensus::PixelSignal<'_> =
        &|_row| pipeline::extraction::consensus::PixelVerdict::Clear;

    let extractor = pipeline::extraction::ConsensusExtractor::new(transport, cfg);
    let outcome = extractor.extract(input_bytes, spec, sanity, pixel).await?;

    if expected.outcome == "hold" {
        return Ok(outcome.published.is_empty() && !outcome.held.is_empty());
    }
    let expected_confidence = match expected.outcome.as_str() {
        "agreed" => policy::CONF_AGREED,
        "escalated" => policy::CONF_ESCALATED,
        "capped" => policy::CONF_SANITY_CAPPED,
        other => anyhow::bail!("expected.json outcome {other:?} outside the closed set"),
    };
    let Some(row) = outcome.published.first() else {
        return Ok(false); // expected a publish, got none.
    };
    let fields_match = expected
        .published
        .as_ref()
        .is_some_and(|want| spec.critical_fields.iter().all(|f| want.pointer(f) == row.row.pointer(f)));
    Ok(fields_match && row.confidence == expected_confidence)
}
```

In `crates/worker/src/bin/consensus-live-eval.rs`, replace the `mode` guard and add the
`--live` branch:

```rust
anyhow::ensure!(
    mode == "--offline" || mode == "--live",
    "unknown mode {mode:?} (expected --offline or --live)"
);

if mode == "--live" {
    anyhow::ensure!(
        std::env::var_os("ANTHROPIC_API_KEY").is_some(),
        "consensus-live-eval --live: refusing — ANTHROPIC_API_KEY is required and absent"
    );
    let cfg = pipeline::extraction::config::ExtractorConfig::load()?;
    let transport = pipeline::extraction::HttpTransport::from_env()?;
    let corpus_root = std::path::Path::new(CORPUS_ROOT);
    let manifest = worker::consensus_eval::load_manifest(corpus_root)?;

    let mut failed = 0usize;
    for entry in &manifest {
        let expected = worker::consensus_eval::load_expected(corpus_root, &entry)?;
        let input = std::fs::read(corpus_root.join(&entry.input))
            .with_context(|| format!("reading {}", entry.input))?;
        // H35b's page-aware consensus_spec(pages) — per-case, from THIS
        // case's own input.png, so template_recognized/table_regions
        // reflect the actual page each case exercises (never a spec built
        // once from a different case's page).
        let case_page = image::load_from_memory(&input)
            .with_context(|| format!("decoding {} as an image", entry.input))?
            .to_luma8();
        let spec = us_house::consensus::consensus_spec(&[case_page]);
        let attempt1 = worker::consensus_eval::run_case_live(&transport, &cfg, &input, &expected, &spec).await?;
        let attempt2 = if attempt1 {
            None
        } else {
            Some(worker::consensus_eval::run_case_live(&transport, &cfg, &input, &expected, &spec).await?)
        };
        let verdict = worker::consensus_eval::classify_live_attempts(attempt1, attempt2);
        match &verdict {
            worker::consensus_eval::CaseVerdict::Pass => println!("PASS  {}", entry.slug),
            worker::consensus_eval::CaseVerdict::Flaky => println!("FLAKY {} (failed once, passed on rerun)", entry.slug),
            worker::consensus_eval::CaseVerdict::Fail(reason) => {
                failed += 1;
                println!("FAIL  {} ({reason})", entry.slug);
            }
        }
    }
    println!("consensus-live-eval --live: {} case(s), {failed} FAIL", manifest.len());
    if failed > 0 {
        std::process::exit(1);
    }
    return Ok(());
}
// ... existing --offline branch unchanged below.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker --lib consensus_eval::tests`
Expected: PASS (3 tests total across H43a/H43b: the pol1→pol2 flip, and this task's
`flake_policy_classifies_attempts_correctly`). Then: `cargo fmt --check && cargo clippy
--all-targets -- -D warnings`

Then confirm the live-test budget is still untouched (this task adds a BIN, not a test):

```bash
grep -rn 'ignore = "needs ANTHROPIC_API_KEY"' --include=*.rs .
```

Expected: exactly one match (unchanged from H42) — `consensus-live-eval` never appears in this
grep's output, because it is a bin, never wrapped in `#[ignore]`.

- [ ] **Step 5: Commit**
```bash
git add crates/worker/src/consensus_eval.rs crates/worker/src/bin/consensus-live-eval.rs
git commit -m "$(cat <<'EOF'
feat(worker): key-gated live regression eval over the error-boundary corpus (goal 021 Phase 3 hardening, task H43b)

Adds consensus-live-eval --live: runs ConsensusExtractor::extract's full
consensus path (fresh model calls) over every error-boundary corpus case
and asserts published field values/confidence or HOLD against
expected.json, with a one-rerun flake policy (fail-then-pass = FLAKY,
reported not failing; two failures = FAIL, nonzero bin exit). This is the
only thing in the repo that can observe a real model/prompt/N regression —
conformance's primed cache never calls a model. Key-gated
(ANTHROPIC_API_KEY required, refuses without it), manual, never CI, and a
bin, not a test: the repo's #[ignore = "needs ANTHROPIC_API_KEY"] count
stays exactly 1.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

**Green gate:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo run -p pipeline --bin conformance -- us_house`

---

### Task H44: Cross-lab vendor transport + [cross_lab] config, DISABLED (D6)

**This task deliberately exceeds the ~1h guideline** — the Vertex translation layer (request/
response translation, ADC auth, config validation) must land as one coherent, independently-
testable module; splitting would leave a `VertexTransport` that compiles but cannot be safely
constructed or validated between commits. Do not split it.

**Read first:** `crates/pipeline/src/extraction/anthropic.rs:90-135` (the `Transport` trait —
object-safe via `async_trait`, blanket `impl<T: Transport + ?Sized> Transport for &T` at
line 101-106 already anticipates `&dyn Transport`); `crates/pipeline/src/extraction/config.rs`
(Task 9's `ExtractorConfig`, `#[serde(default)]` table convention, `composite_model_id`) —
this task adds a table beside `[models]`/`[sampling]`/`[families]`/`[escalation]`, it does not
touch those; `crates/pipeline/src/extraction/consensus.rs` (Task 10/12's `SamplingParams`,
`build_image_request`, `run_samples`) for the exact Anthropic-shaped request body this task's
`VertexTransport` translates — confirm field names against what actually landed and adjust
only field accesses, never the translation *logic*.

**Files:**
- Create: `crates/pipeline/src/extraction/vendors/mod.rs`
- Create: `crates/pipeline/src/extraction/vendors/vertex.rs`
- Modify: `crates/pipeline/src/extraction/mod.rs` (add `pub mod vendors;` beside the existing
  `pub mod anthropic; pub mod cache; pub mod consensus;` list)
- Modify: `crates/pipeline/src/extraction/config.rs` (add `CrossLabConfig` + `[cross_lab]` field
  on `ExtractorConfig` + loader validation)
- Modify: `crates/pipeline/Cargo.toml` (add `gcp_auth` to `[dependencies]` — ADC token
  resolution only; confirm the current published version at implementation time per CLAUDE.md's
  Tooling convention, this task pins `0.12` as of authoring)
- Test: `crates/pipeline/src/extraction/vendors/vertex.rs` (inline `#[cfg(test)]`);
  `crates/pipeline/src/extraction/config.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `pipeline::extraction::Transport` (trait, `anthropic.rs:93`, object-safe — this
  task's `VertexTransport` implements it, and H46 later passes `&dyn Transport` around it);
  the Anthropic-shaped request body `build_image_request` produces (Task 10 — one `messages[0]`
  turn whose `content` array holds `{"type":"image","source":{"type":"base64","media_type":..,
  "data":..}}` blocks followed by one `{"type":"text","text":..}` block, one `tools[0]` with
  `name`/`description`/`input_schema`/`strict:true`, a forced `tool_choice`, and `temperature`
  when `SamplingParams.temperature` is `Some` — read the file first, this is the intended shape
  per the shared contract, not a byte-exact transcript); `pipeline::extraction::anthropic::
  tool_use_input` (`pub(crate)` since Task 12) — reused directly by this task's own test to
  prove the translated-back response is genuinely vendor-blind; H30's `[families]`/`family_of`
  (`config.rs` — this task wires the FIRST `[families]` entry for the activated cross-lab
  vendor/model, the entry H30's own tests documented as absent until H44).
- Produces: `pub struct VertexTransport` implementing `Transport` (`VertexTransport::from_adc(cfg:
  &CrossLabConfig) -> anyhow::Result<Self>` — resolves the GCP project from `GOOGLE_CLOUD_PROJECT`
  and the Vertex region from `GOVFOLIO_VERTEX_LOCATION` (default `"us-central1"`), same
  env-var-with-default convention as `anthropic::Models::from_env`; auth via ADC — metadata
  server or `GOOGLE_APPLICATION_CREDENTIALS`, **never an API key**); `pub(crate) fn
  to_vertex_request(body: &Value, cfg: &CrossLabConfig) -> anyhow::Result<Value>` and `pub(crate)
  fn from_vertex_response(body: &Value, tool_name: &str) -> anyhow::Result<Value>` (the pure
  translation pair, independently testable without any network); `pub struct CrossLabConfig {
  pub enabled: bool, pub vendor: String, pub model: String, pub temperature: Option<f32>, pub
  max_edge_px: Option<u32> }` (exact shape per the shared interface contract) on
  `ExtractorConfig.cross_lab` (`#[serde(default)]`, `CrossLabConfig::default()` has
  `enabled: false`); a private `validate_cross_lab(cfg: &CrossLabConfig) -> anyhow::Result<()>`
  called from `ExtractorConfig::load()`/`ExtractorConfig::parse()` (whichever Task 9 actually
  named — read it first).

- [ ] **Step 1: Write the failing tests**

Add to `crates/pipeline/src/extraction/vendors/vertex.rs` (the file does not exist yet, so
every reference below fails to compile until Step 3 creates the module — write the whole file
now, tests included, per this task's Step-3 shape):

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn anthropic_image_request() -> Value {
        json!({
            "model": "gemini-3.1-flash-lite",
            "max_tokens": 16000,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {"type": "base64", "media_type": "image/png", "data": "cGFnZTE="}
                    },
                    {"type": "text", "text": "transcribe verbatim"}
                ]
            }],
            "tools": [{
                "name": "record_rows",
                "description": "record every row",
                "input_schema": json!({
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "amount_raw": {"type": "string", "pattern": "^\\$"}
                    },
                    "required": ["amount_raw"],
                    "additionalProperties": false
                }),
                "strict": true
            }],
            "tool_choice": {"type": "tool", "name": "record_rows"},
            "temperature": 1.0
        })
    }

    fn cfg(temperature: Option<f32>) -> CrossLabConfig {
        CrossLabConfig {
            enabled: true,
            vendor: "vertex".to_owned(),
            model: "gemini-3.1-flash-lite".to_owned(),
            temperature,
            max_edge_px: None,
        }
    }

    #[test]
    fn to_vertex_request_translates_image_and_text_parts_and_strips_unsupported_schema_keywords() {
        let vertex = to_vertex_request(&anthropic_image_request(), &cfg(Some(1.0))).unwrap();

        assert_eq!(
            vertex["contents"][0]["parts"][0]["inline_data"]["mime_type"],
            json!("image/png")
        );
        assert_eq!(
            vertex["contents"][0]["parts"][0]["inline_data"]["data"],
            json!("cGFnZTE=")
        );
        assert_eq!(
            vertex["contents"][0]["parts"][1]["text"],
            json!("transcribe verbatim")
        );
        assert_eq!(vertex["generationConfig"]["temperature"], json!(1.0));
        assert_eq!(
            vertex["generationConfig"]["responseMimeType"],
            json!("application/json")
        );

        let schema = &vertex["generationConfig"]["responseSchema"];
        assert!(schema.get("$schema").is_none(), "$schema must be stripped");
        assert!(
            schema.get("additionalProperties").is_none(),
            "additionalProperties must be stripped"
        );
        assert!(
            schema["properties"]["amount_raw"].get("pattern").is_none(),
            "pattern must be stripped (API-side strict schemas never carry it — goal 021 Phase 3 §7)"
        );
        assert_eq!(
            schema["properties"]["amount_raw"]["type"],
            json!("string"),
            "supported keywords survive untouched"
        );
    }

    #[test]
    fn to_vertex_request_rejects_a_request_with_no_recognizable_content_block() {
        let mut body = anthropic_image_request();
        body["messages"][0]["content"][0]["type"] = json!("document");
        let err = to_vertex_request(&body, &cfg(Some(1.0))).unwrap_err();
        assert!(format!("{err:#}").contains("unsupported content block"));
    }

    #[test]
    fn from_vertex_response_translates_to_the_anthropic_tool_use_shape_with_mapped_usage() {
        let vertex_response = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "{\"amount_raw\":\"$15,001 - $50,000\"}"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 1500,
                "candidatesTokenCount": 40,
                "totalTokenCount": 1540
            }
        });

        let anthropic_shaped = from_vertex_response(&vertex_response, "record_rows").unwrap();

        assert_eq!(anthropic_shaped["stop_reason"], json!("tool_use"));
        assert_eq!(anthropic_shaped["content"][0]["type"], json!("tool_use"));
        assert_eq!(anthropic_shaped["content"][0]["name"], json!("record_rows"));
        assert_eq!(anthropic_shaped["usage"]["input_tokens"], json!(1500));
        assert_eq!(anthropic_shaped["usage"]["output_tokens"], json!(40));

        // Proves vendor-blindness concretely: the SAME pub(crate) fn every
        // Anthropic response goes through parses this translated response
        // with no vendor-specific branch anywhere downstream.
        let payload =
            crate::extraction::anthropic::tool_use_input(&anthropic_shaped, "record_rows").unwrap();
        assert_eq!(payload["amount_raw"], json!("$15,001 - $50,000"));
    }

    #[test]
    fn from_vertex_response_fails_closed_on_non_json_structured_output() {
        let vertex_response = json!({
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": "not json"}]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 2}
        });
        assert!(from_vertex_response(&vertex_response, "record_rows").is_err());
    }
}
```

Add to `crates/pipeline/src/extraction/config.rs`'s existing `#[cfg(test)] mod tests` (read the
module first for its real TOML-fixture-building helper and reuse it — the two bodies below
assume a helper that takes a full TOML string, matching Task 9's convention):

```rust
#[test]
fn cross_lab_loader_rejects_a_gemini_model_with_temperature_other_than_absent_or_one() {
    let toml_body = r#"
[models]
primary = "claude-haiku-4-5-20251001"
escalation = "claude-sonnet-5"
[sampling]
n = 3
temperature = 0.7
[cross_lab]
enabled = true
vendor = "vertex"
model = "gemini-3.1-flash-lite"
temperature = 0.7
"#;
    let err = ExtractorConfig::parse(toml_body).unwrap_err();
    assert!(
        format!("{err:#}").contains("temperature absent or exactly 1.0"),
        "{err:#}"
    );
}

#[test]
fn cross_lab_loader_accepts_gemini_with_temperature_absent_or_exactly_one() {
    for temperature_line in ["", "temperature = 1.0"] {
        let toml_body = format!(
            "[models]\nprimary = \"claude-haiku-4-5-20251001\"\nescalation = \"claude-sonnet-5\"\n\
             [sampling]\nn = 3\ntemperature = 0.7\n\
             [cross_lab]\nenabled = true\nvendor = \"vertex\"\nmodel = \"gemini-3.1-flash-lite\"\n{temperature_line}\n"
        );
        assert!(ExtractorConfig::parse(&toml_body).is_ok(), "{toml_body}");
    }
}

#[test]
fn cross_lab_loader_rejects_any_vendor_naming_a_router() {
    let toml_body = r#"
[models]
primary = "claude-haiku-4-5-20251001"
escalation = "claude-sonnet-5"
[sampling]
n = 3
temperature = 0.7
[cross_lab]
enabled = true
vendor = "openrouter"
model = "google/gemini-3.1-flash-lite"
"#;
    let err = ExtractorConfig::parse(toml_body).unwrap_err();
    assert!(
        format!("{err:#}").contains("router"),
        "production cross-lab config must reject any router vendor: {err:#}"
    );
}

#[test]
fn cross_lab_disabled_never_changes_composite_model_id_regardless_of_vendor_fields() {
    const BASE: &str = "[models]\nprimary = \"claude-haiku-4-5-20251001\"\n\
        escalation = \"claude-sonnet-5\"\n[sampling]\nn = 3\ntemperature = 0.7\n";
    let baseline = ExtractorConfig::parse(BASE).unwrap();
    let baseline_id = composite_model_id(&baseline);
    assert!(!baseline.cross_lab.enabled, "table absent -> default disabled");

    for (vendor, model, temperature_line) in [
        ("vertex", "gemini-3.1-flash-lite", "temperature = 1.0\n"),
        ("openai", "gpt-5.4-mini", ""),
        ("deepinfra", "qwen3-vl-235b", "temperature = 0.7\n"),
    ] {
        let toml_body = format!(
            "{BASE}[cross_lab]\nenabled = false\nvendor = \"{vendor}\"\nmodel = \"{model}\"\n{temperature_line}"
        );
        let cfg = ExtractorConfig::parse(&toml_body).unwrap();
        assert!(!cfg.cross_lab.enabled);
        assert_eq!(
            composite_model_id(&cfg),
            baseline_id,
            "a DISABLED [cross_lab] table (any vendor/model) must not perturb the cache key — {vendor}"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pipeline --lib extraction::vendors`
Expected: FAIL to compile — `crates/pipeline/src/extraction/vendors/` does not exist yet
(`unresolved module 'vendors'` once `mod.rs` is added in Step 3 without `vertex.rs`, or
`file not found` before that).

Run: `cargo test -p pipeline --lib extraction::config::tests::cross_lab`
Expected: FAIL to compile — `CrossLabConfig` / `ExtractorConfig.cross_lab` do not exist yet.

- [ ] **Step 3: Write the minimal implementation**

`crates/pipeline/src/extraction/vendors/mod.rs`:

```rust
//! Cross-lab vendor transports (goal 021 Phase 3, hardening addendum task
//! H44): translation-layer [`crate::extraction::Transport`] implementations
//! that let a non-Anthropic model stand in for one consensus sample pass
//! without any downstream code (`tool_use_input`, `SamplePass`,
//! `ExtractionStats`) ever knowing the vendor changed. DISABLED by default
//! (`[cross_lab] enabled = false`, `#[serde(default)]`) — activation is a
//! separate, gated task (hardening addendum H46), not this one.
//!
//! Hard rails enforced by the config loader ([`crate::extraction::config`]),
//! not by convention: no free tier (a free-tier key trains on inputs — goal
//! 021 Phase 3 §7 anti-pattern), no router in production (`[cross_lab]`
//! rejects any vendor string naming a router; a router is bake-off-only,
//! hardening addendum H45), Gemini-family models never receive a
//! `temperature` other than absent or exactly `1.0` (Gemini 3.x convention).

pub mod vertex;

pub use vertex::VertexTransport;
```

`crates/pipeline/src/extraction/vendors/vertex.rs` (module doc + translation + transport; the
`#[cfg(test)] mod tests` block from Step 1 stays at the bottom, unchanged):

```rust
//! Vertex AI translation layer (goal 021 Phase 3 H44): accepts the
//! Anthropic-shaped request body [`crate::extraction::consensus::run_samples`]
//! already builds via `build_image_request`, translates it to a Vertex AI
//! `generateContent` request, and translates the response back to the
//! Anthropic tool_use shape so [`crate::extraction::anthropic::tool_use_input`]
//! parses it exactly like a real Anthropic response.
//!
//! Auth is Application Default Credentials ONLY — the GCE/Cloud Run metadata
//! server or `GOOGLE_APPLICATION_CREDENTIALS`, resolved by the `gcp_auth`
//! crate (not the full Google Cloud SDK — same "minimal client, no vendor
//! SDK" discipline as [`crate::extraction::anthropic::HttpTransport`]).
//! Billing rides the existing GCP project (no new vendor, no free tier).

use std::sync::Arc;

use anyhow::Context as _;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::extraction::Transport;
use crate::extraction::config::CrossLabConfig;

/// Vertex AI structured-output schema (OpenAPI 3.0 subset) rejects these
/// standard JSON Schema keywords — confirm against the current Vertex
/// generateContent structured-output docs at implementation time (the goal's
/// per-vendor verification requirement, §3 D); stripped recursively before
/// the schema is emitted as `responseSchema`. `pattern` is ALREADY absent
/// from the schema `build_image_request` sends (Task 18's strict schema
/// keeps date `pattern` in local re-validation only) — listed here anyway as
/// defense in depth, since this translator must never assume its caller's
/// discipline.
const UNSUPPORTED_SCHEMA_KEYWORDS: &[&str] =
    &["$schema", "$id", "title", "examples", "additionalProperties", "pattern"];

fn to_vertex_response_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                if UNSUPPORTED_SCHEMA_KEYWORDS.contains(&key.as_str()) {
                    continue;
                }
                out.insert(key.clone(), to_vertex_response_schema(value));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(to_vertex_response_schema).collect()),
        other => other.clone(),
    }
}

/// Translates one Anthropic Messages-shaped request body into a Vertex AI
/// `generateContent` request body.
///
/// # Errors
/// The request carries a content block this translator does not recognize
/// (fail closed — never silently drop a page or the prompt).
pub(crate) fn to_vertex_request(body: &Value, cfg: &CrossLabConfig) -> anyhow::Result<Value> {
    let content = body["messages"][0]["content"]
        .as_array()
        .context("cross-lab translation: request has no messages[0].content array")?;

    let mut parts = Vec::with_capacity(content.len());
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("image") => {
                let media_type = block["source"]["media_type"]
                    .as_str()
                    .context("cross-lab translation: image block missing source.media_type")?;
                let data = block["source"]["data"]
                    .as_str()
                    .context("cross-lab translation: image block missing source.data")?;
                parts.push(json!({"inline_data": {"mime_type": media_type, "data": data}}));
            }
            Some("text") => {
                let text = block["text"]
                    .as_str()
                    .context("cross-lab translation: text block missing text")?;
                parts.push(json!({"text": text}));
            }
            other => anyhow::bail!(
                "cross-lab translation: unsupported content block {other:?} — fail closed \
                 (invariant 6), never silently drop a page"
            ),
        }
    }

    let tool = body["tools"]
        .get(0)
        .context("cross-lab translation: request has no tools[0]")?;
    let input_schema = tool
        .get("input_schema")
        .context("cross-lab translation: tools[0] has no input_schema")?;
    let response_schema = to_vertex_response_schema(input_schema);

    let mut generation_config = json!({
        "responseMimeType": "application/json",
        "responseSchema": response_schema,
    });
    if let Some(temperature) = cfg.temperature {
        generation_config["temperature"] = json!(temperature);
    }

    Ok(json!({
        "contents": [{"role": "user", "parts": parts}],
        "generationConfig": generation_config,
    }))
}

/// Translates a Vertex AI `generateContent` response body back into the
/// Anthropic tool_use response shape [`crate::extraction::anthropic::tool_use_input`]
/// parses. `usageMetadata` maps to Anthropic's `usage.{input,output}_tokens`
/// so [`crate::extraction::consensus::accumulate_stats`] needs no vendor
/// branch either.
///
/// # Errors
/// No text candidate, or the candidate's text is not valid JSON (a
/// schema-invalid vendor response — fail closed exactly like a malformed
/// Anthropic tool_use payload does, invariant 6).
pub(crate) fn from_vertex_response(body: &Value, tool_name: &str) -> anyhow::Result<Value> {
    let text = body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .context("cross-lab translation: no candidates[0].content.parts[0].text in the Vertex response")?;
    let input: Value = serde_json::from_str(text).with_context(|| {
        format!("cross-lab translation: Vertex structured-output text is not valid JSON: {text}")
    })?;
    let usage = body.get("usageMetadata").cloned().unwrap_or(Value::Null);
    let input_tokens = usage.get("promptTokenCount").and_then(Value::as_u64).unwrap_or(0);
    let output_tokens = usage.get("candidatesTokenCount").and_then(Value::as_u64).unwrap_or(0);

    Ok(json!({
        "content": [{"type": "tool_use", "id": "vertex_0", "name": tool_name, "input": input}],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
    }))
}

/// Real Vertex AI transport. Holds no key material — only an ADC token
/// provider, refreshed per call.
pub struct VertexTransport {
    client: reqwest::Client,
    authn: Arc<dyn gcp_auth::TokenProvider>,
    project: String,
    location: String,
    cfg: CrossLabConfig,
}

impl std::fmt::Debug for VertexTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexTransport")
            .field("project", &self.project)
            .field("location", &self.location)
            .field("model", &self.cfg.model)
            .field("authn", &"<adc token provider, redacted>")
            .finish()
    }
}

impl VertexTransport {
    /// Builds the transport from Application Default Credentials. Project
    /// comes from `GOOGLE_CLOUD_PROJECT`, region from `GOVFOLIO_VERTEX_LOCATION`
    /// (default `"us-central1"`) — same env-var-with-default convention as
    /// [`crate::extraction::anthropic::Models::from_env`]. `[cross_lab]`
    /// carries no project/region fields (shared interface contract is
    /// exhaustive) so those two live in the environment, matching how the
    /// existing Anthropic seam reads its key from the environment too.
    ///
    /// # Errors
    /// ADC resolution failure (no metadata server reachable, no
    /// `GOOGLE_APPLICATION_CREDENTIALS`), or `GOOGLE_CLOUD_PROJECT` unset.
    pub async fn from_adc(cfg: CrossLabConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !cfg.vendor.to_lowercase().contains("openrouter"),
            "VertexTransport constructed with a router vendor {:?} — refuse to build \
             (defense in depth; the config loader already rejects this)",
            cfg.vendor
        );
        let project = std::env::var("GOOGLE_CLOUD_PROJECT")
            .map_err(|_| anyhow::anyhow!("GOOGLE_CLOUD_PROJECT is not set"))?;
        let location =
            std::env::var("GOVFOLIO_VERTEX_LOCATION").unwrap_or_else(|_| "us-central1".to_owned());
        let authn = gcp_auth::provider()
            .await
            .context("resolving Application Default Credentials for Vertex AI")?;
        let client = reqwest::Client::builder()
            .user_agent("govfolio-pipeline/0.1 (+https://govfolio.io)")
            .build()
            .context("building reqwest client")?;
        Ok(Self { client, authn, project, location, cfg })
    }
}

#[async_trait]
impl Transport for VertexTransport {
    async fn send(&self, body: &Value) -> anyhow::Result<Value> {
        let tool_name = body["tools"][0]["name"]
            .as_str()
            .context("cross-lab translation: request has no tools[0].name")?
            .to_owned();
        let vertex_body = to_vertex_request(body, &self.cfg)?;
        let token = self
            .authn
            .token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .context("fetching ADC token for Vertex AI")?;
        let url = format!(
            "https://{loc}-aiplatform.googleapis.com/v1/projects/{proj}/locations/{loc}/\
             publishers/google/models/{model}:generateContent",
            loc = self.location,
            proj = self.project,
            model = self.cfg.model,
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(token.as_str())
            .json(&vertex_body)
            .send()
            .await
            .context("Vertex AI generateContent request failed")?;
        let status = response.status();
        let text = response.text().await.context("reading Vertex AI response body")?;
        anyhow::ensure!(
            status.is_success(),
            "Vertex AI generateContent {status}: {}",
            &text[..text.len().min(400)]
        );
        let response_body: Value =
            serde_json::from_str(&text).context("Vertex AI response is not JSON")?;
        from_vertex_response(&response_body, &tool_name)
    }
}
```

`crates/pipeline/src/extraction/mod.rs` — add one line beside the existing `pub mod` list:

```rust
pub mod vendors;
```

`crates/pipeline/src/extraction/config.rs` — read the file first for the real `ExtractorConfig`
struct/derive stack and `composite_model_id` body, then add:

```rust
/// `[cross_lab]` table (goal 021 Phase 3, hardening H44): DISABLED by
/// default — activation is hardening H46's job, gated on H45's bake-off
/// report + the HARD CAP + (conditionally) a closed vendor-ToS HALT goal.
/// Exact shape per the hardening addendum's shared interface contract — no
/// project/region fields here on purpose (`VertexTransport` reads those from
/// the environment, matching how the existing Anthropic seam reads its key).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct CrossLabConfig {
    pub enabled: bool,
    pub vendor: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_edge_px: Option<u32>,
}

impl Default for CrossLabConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            vendor: String::new(),
            model: String::new(),
            temperature: None,
            max_edge_px: None,
        }
    }
}

/// Fail-closed guard on `[cross_lab]`, called from `ExtractorConfig`'s
/// loader regardless of `enabled` (a disabled-but-malformed table must not
/// silently wait until activation to be discovered).
fn validate_cross_lab(cfg: &CrossLabConfig) -> anyhow::Result<()> {
    if cfg.model.to_lowercase().starts_with("gemini") {
        anyhow::ensure!(
            matches!(cfg.temperature, None | Some(1.0)),
            "config/extractor.toml [cross_lab]: Gemini-family model {:?} requires temperature \
             absent or exactly 1.0 (Gemini 3.x convention: temperature < 1.0 is a documented \
             warning-class misuse) — got {:?}",
            cfg.model,
            cfg.temperature
        );
    }
    anyhow::ensure!(
        !cfg.vendor.to_lowercase().contains("openrouter"),
        "config/extractor.toml [cross_lab]: vendor {:?} names a router — production never \
         routes cross-lab calls through a router (goal 021 Phase 3 §7 anti-pattern; a router \
         is bake-off-only, hardening addendum H45)",
        cfg.vendor
    );
    Ok(())
}
```

Then, inside `ExtractorConfig`'s struct definition add `#[serde(default)] pub cross_lab:
CrossLabConfig,` beside the existing tables, and inside whichever fn actually finishes loading
(`ExtractorConfig::load()`/`ExtractorConfig::parse()` — read the file, call `validate_cross_lab`
there) add: `validate_cross_lab(&cfg.cross_lab)?;` before returning `Ok(cfg)`.

`crates/pipeline/Cargo.toml` — add to `[dependencies]`:

```toml
gcp_auth = "0.12"
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --lib extraction::vendors`
Expected: PASS — all four `vertex.rs` tests, including the vendor-blindness assertion that
round-trips a translated response through the real `tool_use_input`.

Run: `cargo test -p pipeline --lib extraction::config`
Expected: PASS — the four new `[cross_lab]` loader tests plus every pre-existing config test
unchanged.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
&& cargo run -p pipeline --bin conformance -- us_house`
Expected: all green; conformance unaffected (`[cross_lab]` absent from the committed
`config/extractor.toml` fixture used by conformance, so `enabled = false` by default — zero
behavior change, per the Step-1 property test).

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/src/extraction/vendors/mod.rs \
        crates/pipeline/src/extraction/vendors/vertex.rs \
        crates/pipeline/src/extraction/mod.rs \
        crates/pipeline/src/extraction/config.rs \
        crates/pipeline/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(pipeline): cross-lab Vertex translation transport, [cross_lab] DISABLED (goal 021 Phase 3 H44)

Adds VertexTransport, a translation-layer Transport impl that accepts the
same Anthropic-shaped request build_image_request produces and speaks
Vertex AI generateContent underneath (inline_data images, responseSchema
stripped of unsupported keywords, temperature per config), translating the
response back to the Anthropic tool_use shape so every downstream consumer
stays vendor-blind. [cross_lab] ships enabled=false by default; the loader
fail-closed-rejects a Gemini model with temperature other than absent/1.0
and any vendor naming a router. Auth is ADC only, never an API key. Zero
behavior change while disabled (property-tested against composite_model_id).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H45: Bake-off mode + numeric promotion gates report (D4)

**This task deliberately exceeds the ~1h guideline** — the bake-off mode's CLI flags,
`RouterTransport` (with its own offline-tested translation pair), the gate-0 corpus-size
precondition, the four D4 numeric gates, and the pre-transport-call budget gate all land as one
coherent bake-off unit; splitting would leave a bin that can construct a non-production
transport without the budget gate or corpus-size precondition wired yet. Do not split it.

**Concern flagged, resolved:** the goal-021-Phase-3 shared interface contract's H44–H47 summary
names this CLI flag pair `--swap-model <id> --transport <vendor>`; the cluster brief's
`--vendor` variant was superseded — controller-adjudicated 2026-07-08: `--transport` per
the shared interface contract. All bake-off tooling uses `--transport`.

**Read first:** hardening addendum H41a/b (this task extends the `consensus-shadow-eval` worker
bin those tasks create) — confirm its current CLI arg-parsing shape and its offline
confusion-matrix collection loop before wiring the two new flags below; the gate-arithmetic
module this task creates is intentionally standalone (pure functions, no bin dependency) so it
compiles and is fully unit-tested regardless of what H41's bin internals turn out to be.

**Files:**
- Modify: `crates/worker/src/consensus_eval.rs` (append bake-off items: gate arithmetic, report rendering, `RouterTransport`)
- Modify: `crates/worker/src/lib.rs` (add `pub mod consensus_eval;`)
- Modify: `crates/worker/src/bin/consensus-shadow-eval.rs` (add `--swap-model`, `--transport`,
  `--allow-router-bakeoff`, `--report-out` flags; doc header gains the candidate table + Mistral
  dropped-rationale + non-production router marker)
- Test: `crates/worker/src/consensus_eval.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `pipeline::extraction::Transport` (H44); `pipeline::extraction::vendors::
  VertexTransport` (H44, used when `--transport vertex`); H41's shadow-eval corpus-iteration
  loop and its per-field ground-truth comparison (read first — this task's new code plugs into
  it, does not replace it).
- Produces: `pub struct FieldConfusion { .. }`, `pub struct OpsChecklist { .. }`, `pub struct
  CorpusStats { pub total_docs: u32, pub total_rows: u32, pub degraded_stratum_docs: u32 }`
  (gate 0 — corpus ≥400 docs AND ≥1,500 rows AND ≥150 degraded-stratum docs, numeric literals),
  `pub enum GateVerdict { PromoteEligible, DoNotPromote { failing_gates: Vec<String> },
  InsufficientCorpus { measured: CorpusStats } }`, `pub fn verdict(corpus: &CorpusStats, fields:
  &[FieldConfusion], total_candidate_calls: u32, candidate_schema_invalid_after_retry: u32, ops:
  &OpsChecklist) -> GateVerdict` (gate 0 checked FIRST — an insufficient corpus is always
  `InsufficientCorpus`, never `PromoteEligible`, regardless of gates 1-4), `pub fn
  render_report_markdown(candidate_model: &str, vendor: &str, fields: &[FieldConfusion], ops:
  &OpsChecklist, verdict: &GateVerdict) -> String` (all pure, offline, no network — exercised by
  `cargo test -p worker`, never gated on a key); `pub struct RouterTransport` implementing
  `Transport` (bake-off-only, `--allow-router-bakeoff` gated in the bin, never constructed in the
  production `run_samples` path), plus its pure `pub(crate) fn to_router_request`/`fn
  from_router_response` translation pair and `pub fn resolve_transport_flag(value: &str,
  allow_router_bakeoff: bool) -> anyhow::Result<()>`; `pub async fn run_bakeoff_call<T: Transport>
  (cfg: &ExtractorConfig, transport: &T, request: &Value) -> anyhow::Result<Value>` (budget-gates
  before any transport call, same `require_budget()` discipline as H41b).

- [ ] **Step 1: Write the failing tests**

Append the following to `crates/worker/src/consensus_eval.rs`'s existing `#[cfg(test)] mod
tests` (the file and its test module already exist after H43a/H43b — this task only adds the
bake-off gate-arithmetic symbols and their tests):

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A field that clears every gate: -1pp overall (within -2pp), -3pp
    /// degraded (within -4pp), 30% conditional co-error (within 40%),
    /// candidate-Haiku wrong-value overlap 20% < Haiku-Haiku overlap 30%.
    fn passing_field(name: &str) -> FieldConfusion {
        FieldConfusion {
            field: name.to_owned(),
            haiku_baseline_correct: 96,
            haiku_baseline_total: 100,
            haiku_degraded_correct: 90,
            haiku_degraded_total: 100,
            candidate_correct: 95,
            candidate_total: 100,
            candidate_degraded_correct: 87,
            candidate_degraded_total: 100,
            haiku_both_err_same_value: 20,
            candidate_errs_same_value_given_haiku_both_err: 6,
            haiku_haiku_wrong_value_overlap: 30,
            haiku_haiku_wrong_value_total: 100,
            candidate_haiku_wrong_value_overlap: 20,
            candidate_haiku_wrong_value_total: 100,
        }
    }

    fn passing_ops() -> OpsChecklist {
        OpsChecklist {
            batch_images_and_schema_proven: true,
            price_per_pass_leq_2x_haiku: true,
            no_shutdown_inside_backfill_horizon: true,
            tos_no_training: true,
            temperature_convention_verified: true,
        }
    }

    /// Clears gate 0 (corpus >= 400 docs, >= 1,500 rows, >= 150
    /// degraded-stratum docs) with headroom — every test below except the
    /// dedicated gate-0 tests uses this so gates 1-4 are what's under test.
    fn passing_corpus() -> CorpusStats {
        CorpusStats { total_docs: 500, total_rows: 2_000, degraded_stratum_docs: 200 }
    }

    #[test]
    fn all_gates_green_is_promote_eligible() {
        let fields = vec![passing_field("amount_raw"), passing_field("transaction_date_raw")];
        // 3/500 = 0.6% schema-invalid-after-retry, within the 1.0% ceiling.
        let result = verdict(&passing_corpus(), &fields, 500, 3, &passing_ops());
        assert_eq!(result, GateVerdict::PromoteEligible);
    }

    #[test]
    fn gate1_accuracy_failing_alone_reports_do_not_promote_naming_only_gate1() {
        let mut degraded = passing_field("amount_raw");
        degraded.candidate_correct = 90; // -6pp overall vs Haiku's 96% -> fails the -2pp floor
        let fields = vec![degraded, passing_field("transaction_date_raw")];
        let result = verdict(&passing_corpus(), &fields, 500, 3, &passing_ops());
        assert_eq!(
            result,
            GateVerdict::DoNotPromote {
                failing_gates: vec!["gate1_accuracy_overall[amount_raw]".to_owned()]
            }
        );
    }

    #[test]
    fn gate2_schema_invalid_rate_failing_alone_reports_do_not_promote_naming_only_gate2() {
        let fields = vec![passing_field("amount_raw")];
        // 8/500 = 1.6% > the 1.0% ceiling.
        let result = verdict(&passing_corpus(), &fields, 500, 8, &passing_ops());
        match result {
            GateVerdict::DoNotPromote { failing_gates } => {
                assert_eq!(failing_gates.len(), 1);
                assert!(failing_gates[0].starts_with("gate2_schema_invalid_rate"));
            }
            other => panic!("expected DoNotPromote, got {other:?}"),
        }
    }

    #[test]
    fn gate3_decorrelation_conditional_failing_alone_reports_do_not_promote_naming_only_gate3() {
        let mut correlated = passing_field("amount_raw");
        correlated.candidate_errs_same_value_given_haiku_both_err = 12; // 12/20 = 0.60 > 0.40
        let fields = vec![correlated];
        let result = verdict(&passing_corpus(), &fields, 500, 3, &passing_ops());
        assert_eq!(
            result,
            GateVerdict::DoNotPromote {
                failing_gates: vec!["gate3_decorrelation_conditional[amount_raw]".to_owned()]
            }
        );
    }

    #[test]
    fn gate3_decorrelation_overlap_failing_alone_reports_do_not_promote_naming_only_gate3() {
        let mut not_decorrelated = passing_field("amount_raw");
        not_decorrelated.candidate_haiku_wrong_value_overlap = 35; // 0.35 >= Haiku-Haiku's 0.30
        let fields = vec![not_decorrelated];
        let result = verdict(&passing_corpus(), &fields, 500, 3, &passing_ops());
        assert_eq!(
            result,
            GateVerdict::DoNotPromote {
                failing_gates: vec!["gate3_decorrelation_overlap[amount_raw]".to_owned()]
            }
        );
    }

    #[test]
    fn gate4_ops_failing_alone_reports_do_not_promote_naming_only_the_failed_row() {
        let fields = vec![passing_field("amount_raw")];
        let mut ops = passing_ops();
        ops.no_shutdown_inside_backfill_horizon = false;
        let result = verdict(&passing_corpus(), &fields, 500, 3, &ops);
        assert_eq!(
            result,
            GateVerdict::DoNotPromote {
                failing_gates: vec!["gate4_ops_no_shutdown_inside_backfill_horizon".to_owned()]
            }
        );
    }

    #[test]
    fn report_markdown_names_the_verdict_and_every_failing_gate() {
        let mut degraded = passing_field("amount_raw");
        degraded.candidate_correct = 90;
        let fields = vec![degraded];
        let result = verdict(&passing_corpus(), &fields, 500, 3, &passing_ops());
        let report = render_report_markdown("gemini-3.1-flash-lite", "vertex", &fields, &passing_ops(), &result);
        assert!(report.contains("DO-NOT-PROMOTE"));
        assert!(report.contains("gate1_accuracy_overall[amount_raw]"));
        assert!(report.contains("gemini-3.1-flash-lite"));
    }

    #[test]
    fn corpus_below_gate0_minimums_is_insufficient_corpus_never_promote_eligible_even_with_all_other_gates_green() {
        let small_corpus = CorpusStats { total_docs: 50, total_rows: 200, degraded_stratum_docs: 10 };
        let fields = vec![passing_field("amount_raw"), passing_field("transaction_date_raw")];
        let result = verdict(&small_corpus, &fields, 500, 3, &passing_ops());
        assert_eq!(result, GateVerdict::InsufficientCorpus { measured: small_corpus });

        let report = render_report_markdown("gemini-3.1-flash-lite", "vertex", &fields, &passing_ops(), &result);
        assert!(report.contains("INSUFFICIENT-CORPUS"));
        assert!(!report.contains("PROMOTE-ELIGIBLE"), "an insufficient corpus must never render as eligible");
        assert!(report.contains("50 docs"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p worker --lib consensus_eval`
Expected: FAIL to compile — `FieldConfusion`, `OpsChecklist`, `GateVerdict`, `verdict`,
`render_report_markdown` do not exist yet in the already-existing `consensus_eval` module (new
symbols missing, not the file itself — H43a/H43b already created and populated it).

- [ ] **Step 3: Write the minimal implementation**

Append to `crates/worker/src/consensus_eval.rs` (below H43a/H43b's existing content):

```rust
// ---- Bake-off gate arithmetic + founder-readable report (H45) ----
//
// All types/functions below are pure and offline — no network, no key,
// exercised by every normal `cargo test -p worker` run. The bin that FEEDS
// these functions real numbers (`consensus-shadow-eval --swap-model ..
// --transport ..`) is manual and best-effort key-gated (same discipline as
// the H43b live eval bin): this module is not that bin, and is never itself
// a CI gate on live traffic — it is the deterministic arithmetic layer
// underneath one.
//
// Gates, copied verbatim from the design amendment (§4 D4, numeric — goal
// 021 Phase 3 approved findings §D):
// 1. per-critical-field accuracy >= Haiku-baseline -2pp overall AND
//    >= -4pp on the degraded stratum;
// 2. schema-invalid pass rate <= 1.0% after one retry;
// 3. decorrelation: P(candidate errs the SAME wrong value | both Haiku
//    samples err with that value) <= 0.40 per critical field, AND
//    candidate-Haiku wrong-value overlap < Haiku-Haiku wrong-value overlap;
// 4. ops: batch images+schema proven, price <= 2x the Haiku pass price, no
//    shutdown inside the backfill horizon (re-verify the provider
//    deprecations page at RUN time, not at plan-authoring time), ToS
//    no-training, per-vendor temperature convention verified.
//
// ALL FOUR must pass for `PROMOTE-ELIGIBLE`; any single failing gate is
// `DO-NOT-PROMOTE`, naming every failed gate (never just the first).

use async_trait::async_trait;
use serde_json::Value;

use pipeline::extraction::Transport;
use pipeline::extraction::config::ExtractorConfig;

/// One critical field's confusion counts over the shadow-eval corpus
/// (H41a's electronic-PTR ground truth + degraded stratum) for ONE bake-off
/// candidate run in `--swap-model` mode.
#[derive(Debug, Clone)]
pub struct FieldConfusion {
    pub field: String,
    pub haiku_baseline_correct: u32,
    pub haiku_baseline_total: u32,
    pub haiku_degraded_correct: u32,
    pub haiku_degraded_total: u32,
    pub candidate_correct: u32,
    pub candidate_total: u32,
    pub candidate_degraded_correct: u32,
    pub candidate_degraded_total: u32,
    /// Rows where BOTH Haiku samples err with the SAME wrong value.
    pub haiku_both_err_same_value: u32,
    /// Of those rows, how many the candidate ALSO errs with that same value.
    pub candidate_errs_same_value_given_haiku_both_err: u32,
    pub haiku_haiku_wrong_value_overlap: u32,
    pub haiku_haiku_wrong_value_total: u32,
    pub candidate_haiku_wrong_value_overlap: u32,
    pub candidate_haiku_wrong_value_total: u32,
}

/// Gate 4's non-numeric ops checklist — every row is verified BY HAND at
/// bake-off run time (deprecation pages, ToS pages, price sheets change
/// underneath a plan) and threaded in as booleans; this module only does the
/// AND.
#[derive(Debug, Clone)]
pub struct OpsChecklist {
    pub batch_images_and_schema_proven: bool,
    pub price_per_pass_leq_2x_haiku: bool,
    pub no_shutdown_inside_backfill_horizon: bool,
    pub tos_no_training: bool,
    pub temperature_convention_verified: bool,
}

/// Bake-off corpus size, measured over the actual shadow-eval run (H41a's
/// corpus-iteration loop already counts documents/rows; "degraded stratum"
/// is H42's refill-generated degraded documents — count those separately).
/// Gate 0 (goal 021 Phase 3 D4): a corpus too small to trust ANY of gates
/// 1-3's percentages is never `PROMOTE-ELIGIBLE`, regardless of how green
/// the other numbers look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusStats {
    pub total_docs: u32,
    pub total_rows: u32,
    pub degraded_stratum_docs: u32,
}

/// Gate 0 minimums (goal 021 Phase 3 D4 numeric gates) — below any one of
/// these, the corpus is too small to trust gates 1-3's percentages, full
/// stop; `verdict` never reports `PROMOTE-ELIGIBLE` from an insufficient
/// corpus regardless of how green the other numbers look.
const GATE0_MIN_TOTAL_DOCS: u32 = 400;
const GATE0_MIN_TOTAL_ROWS: u32 = 1_500;
const GATE0_MIN_DEGRADED_STRATUM_DOCS: u32 = 150;

fn gate0_corpus_sufficient(corpus: &CorpusStats) -> bool {
    corpus.total_docs >= GATE0_MIN_TOTAL_DOCS
        && corpus.total_rows >= GATE0_MIN_TOTAL_ROWS
        && corpus.degraded_stratum_docs >= GATE0_MIN_DEGRADED_STRATUM_DOCS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    PromoteEligible,
    DoNotPromote { failing_gates: Vec<String> },
    /// Gate 0 precondition failed — corpus too small to trust gates 1-3's
    /// percentages. NEVER `PromoteEligible`, regardless of gates 1-4.
    InsufficientCorpus { measured: CorpusStats },
}

fn rate(numerator: u32, denominator: u32) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        f64::from(numerator) / f64::from(denominator)
    }
}

fn gate1_accuracy(fields: &[FieldConfusion]) -> Vec<String> {
    let mut failing = Vec::new();
    for field in fields {
        let haiku_overall = rate(field.haiku_baseline_correct, field.haiku_baseline_total);
        let candidate_overall = rate(field.candidate_correct, field.candidate_total);
        if candidate_overall < haiku_overall - 0.02 {
            failing.push(format!("gate1_accuracy_overall[{}]", field.field));
        }
        let haiku_degraded = rate(field.haiku_degraded_correct, field.haiku_degraded_total);
        let candidate_degraded = rate(field.candidate_degraded_correct, field.candidate_degraded_total);
        if candidate_degraded < haiku_degraded - 0.04 {
            failing.push(format!("gate1_accuracy_degraded[{}]", field.field));
        }
    }
    failing
}

fn gate2_schema_invalid_rate(total_calls: u32, invalid_after_retry: u32) -> Vec<String> {
    let observed = rate(invalid_after_retry, total_calls);
    if observed <= 0.01 {
        Vec::new()
    } else {
        vec![format!("gate2_schema_invalid_rate ({:.4} > 0.01)", observed)]
    }
}

fn gate3_decorrelation(fields: &[FieldConfusion]) -> Vec<String> {
    let mut failing = Vec::new();
    for field in fields {
        let conditional = rate(
            field.candidate_errs_same_value_given_haiku_both_err,
            field.haiku_both_err_same_value,
        );
        if conditional > 0.40 {
            failing.push(format!("gate3_decorrelation_conditional[{}]", field.field));
        }
        let haiku_haiku_overlap = rate(field.haiku_haiku_wrong_value_overlap, field.haiku_haiku_wrong_value_total);
        let candidate_haiku_overlap =
            rate(field.candidate_haiku_wrong_value_overlap, field.candidate_haiku_wrong_value_total);
        if candidate_haiku_overlap >= haiku_haiku_overlap {
            failing.push(format!("gate3_decorrelation_overlap[{}]", field.field));
        }
    }
    failing
}

fn gate4_ops(checklist: &OpsChecklist) -> Vec<String> {
    let mut failing = Vec::new();
    if !checklist.batch_images_and_schema_proven {
        failing.push("gate4_ops_batch_images_and_schema_proven".to_owned());
    }
    if !checklist.price_per_pass_leq_2x_haiku {
        failing.push("gate4_ops_price_per_pass_leq_2x_haiku".to_owned());
    }
    if !checklist.no_shutdown_inside_backfill_horizon {
        failing.push("gate4_ops_no_shutdown_inside_backfill_horizon".to_owned());
    }
    if !checklist.tos_no_training {
        failing.push("gate4_ops_tos_no_training".to_owned());
    }
    if !checklist.temperature_convention_verified {
        failing.push("gate4_ops_temperature_convention_verified".to_owned());
    }
    failing
}

/// Computes the D4 gate verdict. Gate 0 (corpus size) is checked FIRST —
/// an insufficient corpus is always `INSUFFICIENT-CORPUS`, never
/// `PROMOTE-ELIGIBLE`, regardless of gates 1-4. Otherwise `PROMOTE-ELIGIBLE`
/// requires all four numbered gates green; any single failure reports
/// `DO-NOT-PROMOTE` naming EVERY failed gate (never truncated to the first).
#[must_use]
pub fn verdict(
    corpus: &CorpusStats,
    fields: &[FieldConfusion],
    total_candidate_calls: u32,
    candidate_schema_invalid_after_retry: u32,
    ops: &OpsChecklist,
) -> GateVerdict {
    if !gate0_corpus_sufficient(corpus) {
        return GateVerdict::InsufficientCorpus { measured: *corpus };
    }
    let mut failing_gates = Vec::new();
    failing_gates.extend(gate1_accuracy(fields));
    failing_gates.extend(gate2_schema_invalid_rate(total_candidate_calls, candidate_schema_invalid_after_retry));
    failing_gates.extend(gate3_decorrelation(fields));
    failing_gates.extend(gate4_ops(ops));
    if failing_gates.is_empty() {
        GateVerdict::PromoteEligible
    } else {
        GateVerdict::DoNotPromote { failing_gates }
    }
}

/// Founder-readable markdown report (stdout AND file — the bin writes this
/// SAME string to both). Never omits a failing gate.
#[must_use]
pub fn render_report_markdown(
    candidate_model: &str,
    vendor: &str,
    fields: &[FieldConfusion],
    ops: &OpsChecklist,
    result: &GateVerdict,
) -> String {
    let mut out = format!(
        "# Bake-off gate report — {candidate_model} ({vendor})\n\n\
         Goal 021 Phase 3 D4 numeric gates. Router (if used to reach this candidate) is \
         BAKE-OFF ONLY — never production.\n\n\
         | Field | Haiku overall | Candidate overall | Haiku degraded | Candidate degraded |\n\
         |---|---|---|---|---|\n"
    );
    for field in fields {
        out.push_str(&format!(
            "| {} | {:.1}% | {:.1}% | {:.1}% | {:.1}% |\n",
            field.field,
            rate(field.haiku_baseline_correct, field.haiku_baseline_total) * 100.0,
            rate(field.candidate_correct, field.candidate_total) * 100.0,
            rate(field.haiku_degraded_correct, field.haiku_degraded_total) * 100.0,
            rate(field.candidate_degraded_correct, field.candidate_degraded_total) * 100.0,
        ));
    }
    out.push_str(&format!(
        "\n## Ops checklist (gate 4)\n\n\
         - batch images+schema proven: {}\n\
         - price <= 2x Haiku pass: {}\n\
         - no shutdown inside backfill horizon: {}\n\
         - ToS no-training: {}\n\
         - temperature convention verified: {}\n\n",
        ops.batch_images_and_schema_proven,
        ops.price_per_pass_leq_2x_haiku,
        ops.no_shutdown_inside_backfill_horizon,
        ops.tos_no_training,
        ops.temperature_convention_verified,
    ));
    match result {
        GateVerdict::PromoteEligible => out.push_str("## Verdict: PROMOTE-ELIGIBLE\n\nAll four gates green.\n"),
        GateVerdict::DoNotPromote { failing_gates } => {
            out.push_str("## Verdict: DO-NOT-PROMOTE\n\nFailing gates:\n");
            for gate in failing_gates {
                out.push_str(&format!("- {gate}\n"));
            }
        }
        GateVerdict::InsufficientCorpus { measured } => {
            out.push_str(&format!(
                "## Verdict: INSUFFICIENT-CORPUS\n\nCorpus does not yet meet gate 0's \
                 precondition (>= {GATE0_MIN_TOTAL_DOCS} docs, >= {GATE0_MIN_TOTAL_ROWS} rows, \
                 >= {GATE0_MIN_DEGRADED_STRATUM_DOCS} degraded-stratum docs) — measured: \
                 {} docs, {} rows, {} degraded-stratum docs. Gates 1-4 are not evaluated; a \
                 small corpus's percentages are not trustworthy at any value.\n",
                measured.total_docs, measured.total_rows, measured.degraded_stratum_docs
            ));
        }
    }
    out
}

/// Bake-off-only translation transport for non-Vertex candidates
/// (gpt-5.4-mini, Qwen3-VL-235B), reached via an OpenRouter-compatible
/// (OpenAI chat-completions-shaped) endpoint with `require_parameters: true`
/// and zero-data-retention routing. Constructed ONLY behind the bin's
/// `--allow-router-bakeoff` flag — never on the production `run_samples`
/// path (goal 021 Phase 3 §7 anti-pattern: router in production).
///
/// Confirm the exact OpenRouter request/response field names against
/// <https://openrouter.ai/docs> at implementation time (this transport is
/// manual/key-gated bake-off tooling, never exercised by CI). Like H44's
/// `VertexTransport`, the translation logic is factored into pure,
/// offline-testable functions (`to_router_request`/`from_router_response`
/// below) — `send()` is a thin wrapper over them plus the actual HTTP call.
pub struct RouterTransport {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl std::fmt::Debug for RouterTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterTransport")
            .field("model", &self.model)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl RouterTransport {
    /// # Errors
    /// `OPENROUTER_API_KEY` unset, or client construction failure. Never a
    /// free-tier key (goal 021 Phase 3 §7 anti-pattern) — the caller is
    /// responsible for using a paid OpenRouter account; this constructor
    /// does not and cannot verify that mechanically.
    pub fn from_env(model: String) -> anyhow::Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY is not set (bake-off only, never production)"))?;
        let client = reqwest::Client::builder()
            .user_agent("govfolio-pipeline-bakeoff/0.1 (+https://govfolio.io)")
            .build()
            .context("building reqwest client")?;
        Ok(Self { client, api_key, model })
    }
}

/// Pure request translation: the Anthropic-shaped request `run_samples`
/// builds -> an OpenRouter (OpenAI chat-completions-shaped) request body.
/// Independently testable without any network (mirrors H44's
/// `to_vertex_request`).
///
/// # Errors
/// The request has no `messages[0].content` array, or a content block is
/// neither `image` nor `text`.
pub(crate) fn to_router_request(body: &Value, model: &str) -> anyhow::Result<Value> {
    let content = body["messages"][0]["content"]
        .as_array()
        .context("router translation: request has no messages[0].content array")?;
    let mut parts = Vec::with_capacity(content.len());
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("image") => {
                let media_type = block["source"]["media_type"].as_str().unwrap_or("image/png");
                let data = block["source"]["data"]
                    .as_str()
                    .context("router translation: image block missing source.data")?;
                parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": {"url": format!("data:{media_type};base64,{data}")}
                }));
            }
            Some("text") => {
                let text = block["text"].as_str().unwrap_or_default();
                parts.push(serde_json::json!({"type": "text", "text": text}));
            }
            other => anyhow::bail!("router translation: unsupported content block {other:?}"),
        }
    }
    let tool = &body["tools"][0];
    Ok(serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": parts}],
        "tools": [{
            "type": "function",
            "function": {
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["input_schema"],
            }
        }],
        "tool_choice": {"type": "function", "function": {"name": tool["name"]}},
        "temperature": body.get("temperature"),
        "provider": {"require_parameters": true, "data_collection": "deny"},
    }))
}

/// Pure response translation: an OpenRouter chat-completions response ->
/// the SAME Anthropic tool-use shape `pipeline::extraction::anthropic::
/// tool_use_input` already parses — vendor-blindness downstream, no branch
/// anywhere else in the pipeline. Independently testable without any
/// network (mirrors H44's `from_vertex_response`).
///
/// # Errors
/// `choices[0].message.tool_calls[0].function.arguments` is missing or not
/// valid JSON.
pub(crate) fn from_router_response(body: &Value) -> anyhow::Result<Value> {
    let call = &body["choices"][0]["message"]["tool_calls"][0];
    let name = call["function"]["name"].as_str().unwrap_or_default().to_owned();
    let arguments: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap_or("{}"))
        .context("router translation: tool_calls[0].function.arguments is not valid JSON")?;
    let usage = body.get("usage").cloned().unwrap_or(Value::Null);
    Ok(serde_json::json!({
        "content": [{"type": "tool_use", "id": "router_0", "name": name, "input": arguments}],
        "stop_reason": "tool_use",
        "usage": {
            "input_tokens": usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
            "output_tokens": usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
        },
    }))
}

/// Resolves `--transport <value>` against `--allow-router-bakeoff`: `vertex`
/// is always allowed; any other value (a router) requires the explicit
/// flag, refusing closed otherwise — never production (goal 021 Phase 3 §7
/// anti-pattern: router in production). Pure, unit-tested without a CLI.
///
/// # Errors
/// `value != "vertex"` and `allow_router_bakeoff` is `false`.
pub fn resolve_transport_flag(value: &str, allow_router_bakeoff: bool) -> anyhow::Result<()> {
    if value == "vertex" || allow_router_bakeoff {
        Ok(())
    } else {
        anyhow::bail!(
            "--transport {value} requires --allow-router-bakeoff (bake-off-only, never production)"
        )
    }
}

/// Budget-gates ONE bake-off swap-model call BEFORE any transport call —
/// the SAME `require_budget()` HARD-CAP gate H41b's `submit_shadow_batch`
/// uses (Task 22's cap discipline extends to bake-off spend; there is no
/// separately ungated live path). The bin's live loop calls this for every
/// corpus document, never a raw `transport.send` directly.
///
/// # Errors
/// `BudgetUnset` (fail closed, before any transport call), or the
/// transport's own send failure.
pub async fn run_bakeoff_call<T: Transport>(
    cfg: &ExtractorConfig,
    transport: &T,
    request: &Value,
) -> anyhow::Result<Value> {
    cfg.require_budget().map_err(|e| anyhow::anyhow!("{e}"))?; // CAP GATE: before ANY transport call.
    transport.send(request).await
}

#[async_trait]
impl Transport for RouterTransport {
    async fn send(&self, body: &Value) -> anyhow::Result<Value> {
        let router_body = to_router_request(body, &self.model)?;
        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&router_body)
            .send()
            .await
            .context("OpenRouter chat completions request failed")?;
        let status = response.status();
        let text = response.text().await.context("reading OpenRouter response body")?;
        anyhow::ensure!(status.is_success(), "OpenRouter {status}: {}", &text[..text.len().min(400)]);
        let response_body: Value = serde_json::from_str(&text).context("OpenRouter response is not JSON")?;
        from_router_response(&response_body)
    }
}
```

Add to `crates/worker/src/consensus_eval.rs`'s `#[cfg(test)] mod tests` (mirroring H44's
`VertexTransport` translation tests — request translation, response translation, and the
router-flag refusal, all offline/mock-driven, no network):

```rust
#[test]
fn to_router_request_translates_image_and_text_parts_and_forces_the_tool_call() {
    let body = serde_json::json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image", "source": {"media_type": "image/png", "data": "cGFnZTE="}},
                {"type": "text", "text": "transcribe verbatim"}
            ]
        }],
        "tools": [{
            "name": "record_rows",
            "description": "record every row",
            "input_schema": serde_json::json!({"type": "object"})
        }],
        "temperature": 0.7,
    });
    let request = to_router_request(&body, "gpt-5.4-mini").unwrap();
    assert_eq!(request["model"], serde_json::json!("gpt-5.4-mini"));
    let content = request["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], serde_json::json!("image_url"));
    assert!(
        content[0]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert_eq!(content[1]["type"], serde_json::json!("text"));
    assert_eq!(request["tools"][0]["function"]["name"], serde_json::json!("record_rows"));
    assert_eq!(
        request["tool_choice"]["function"]["name"],
        serde_json::json!("record_rows")
    );
    assert_eq!(request["provider"]["require_parameters"], serde_json::json!(true));
    assert_eq!(request["provider"]["data_collection"], serde_json::json!("deny"));
}

#[test]
fn to_router_request_rejects_a_request_with_no_recognizable_content_block() {
    let mut body = serde_json::json!({
        "messages": [{"role": "user", "content": [{"type": "document"}]}],
        "tools": [{"name": "record_rows", "description": "d", "input_schema": serde_json::json!({})}],
    });
    body["messages"][0]["content"][0]["type"] = serde_json::json!("document");
    let err = to_router_request(&body, "gpt-5.4-mini").unwrap_err();
    assert!(format!("{err:#}").contains("unsupported content block"));
}

#[test]
fn from_router_response_translates_to_the_anthropic_tool_use_shape_with_mapped_usage() {
    let router_response = serde_json::json!({
        "choices": [{"message": {"tool_calls": [{"function": {
            "name": "record_rows",
            "arguments": "{\"amount_raw\":\"$15,001 - $50,000\"}"
        }}]}}],
        "usage": {"prompt_tokens": 1500, "completion_tokens": 40}
    });
    let anthropic_shaped = from_router_response(&router_response).unwrap();
    assert_eq!(anthropic_shaped["stop_reason"], serde_json::json!("tool_use"));
    assert_eq!(anthropic_shaped["content"][0]["type"], serde_json::json!("tool_use"));
    assert_eq!(anthropic_shaped["content"][0]["name"], serde_json::json!("record_rows"));
    assert_eq!(anthropic_shaped["usage"]["input_tokens"], serde_json::json!(1500));
    assert_eq!(anthropic_shaped["usage"]["output_tokens"], serde_json::json!(40));
}

#[test]
fn from_router_response_fails_closed_on_non_json_tool_call_arguments() {
    let router_response = serde_json::json!({
        "choices": [{"message": {"tool_calls": [{"function": {
            "name": "record_rows",
            "arguments": "not json"
        }}]}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 2}
    });
    assert!(from_router_response(&router_response).is_err());
}

#[test]
fn resolve_transport_flag_refuses_a_non_vertex_transport_without_allow_router_bakeoff() {
    let err = resolve_transport_flag("openrouter-gpt5", false).unwrap_err();
    assert!(format!("{err:#}").contains("--allow-router-bakeoff"));
}

#[test]
fn resolve_transport_flag_accepts_vertex_without_the_flag_and_any_transport_with_it() {
    assert!(resolve_transport_flag("vertex", false).is_ok());
    assert!(resolve_transport_flag("openrouter-gpt5", true).is_ok());
}

/// Mirrors H41b's `shadow_submit_refuses_before_any_transport_call_when_budget_unset`:
/// the live bake-off path must budget-gate BEFORE any transport call, never after.
#[tokio::test]
async fn run_bakeoff_call_refuses_before_any_transport_call_when_budget_unset() {
    struct CountingTransport {
        calls: std::sync::atomic::AtomicUsize,
    }
    #[async_trait::async_trait]
    impl pipeline::extraction::Transport for CountingTransport {
        async fn send(&self, _body: &Value) -> anyhow::Result<Value> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Value::Null)
        }
    }
    let mut file = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(
        &mut file,
        br#"
[models]
primary = "claude-haiku-4-5-20251001"
escalation = "claude-sonnet-5"
[sampling]
n = 3
temperature = 0.7
[preprocess]
max_edge = 1568
[versions]
prompt = "p2"
policy = "pol2"
quality = "q1"
"#,
    )
    .unwrap();
    let cfg = ExtractorConfig::load_from(file.path(), |_| None).unwrap();
    let transport = CountingTransport { calls: std::sync::atomic::AtomicUsize::new(0) };

    let err = run_bakeoff_call(&cfg, &transport, &serde_json::json!({})).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("budget"),
        "error should name the missing budget key: {err}"
    );
    assert_eq!(
        transport.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "must refuse before any transport call — zero send calls"
    );
}
```

`crates/worker/src/lib.rs` — add one line beside the existing `pub mod` list:

```rust
pub mod consensus_eval;
```

`crates/worker/src/bin/consensus-shadow-eval.rs` — read H41's current doc header and
arg-parsing first, then: (a) extend the doc header with the candidate table (Gemini 3.1
Flash-Lite via Vertex, ops-preferred; gpt-5.4-mini; Qwen3-VL-235B — **Mistral Small 4 dropped:
vision+schema pairing undocumented, weakest evidence of the four researched candidates**) and
an explicit **"Router mode (`--allow-router-bakeoff`) is NEVER production — bake-off calls
only"** line; (b) add `--swap-model <id>`, `--transport <vendor>` (`vertex` uses H44's
`VertexTransport`; any other value requires `--allow-router-bakeoff` and constructs
`worker::consensus_eval::RouterTransport` — the parser calls `worker::consensus_eval::
resolve_transport_flag(value, allow_router_bakeoff)` FIRST and refuses closed on `Err`, naming
the missing flag, before constructing anything), `--allow-router-bakeoff` (bool), `--report-out
<path>` (defaults to `bakeoff-report-<candidate_model>.md` in the invocation directory) to the
CLI parser; (c) BEFORE any transport call, call `cfg.require_budget()` (the SAME HARD-CAP gate
H41b's `submit_shadow_batch` uses) and refuse closed naming the missing key on `Err` — bake-off
spend is real spend, never separately ungated; (d) after the corpus run, aggregate per-field
`FieldConfusion` from the existing confusion-matrix collection (H41's job — read it, this task
only adds the aggregation call), build the `OpsChecklist` from CLI-supplied or
hardcoded-per-run booleans (operator fills these in by hand per gate 4's own "verify at run
time" requirement — never auto-inferred), compute `CorpusStats` from the corpus run (docs/rows/
degraded-stratum counts — read H41's corpus-iteration loop for what it already counts), call
`worker::consensus_eval::verdict` + `render_report_markdown`, print to stdout, and write the
same string to `--report-out`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p worker --lib consensus_eval`
Expected: PASS — all 15 of this task's new tests: the 7 gate-arithmetic tests (one all-green +
one per gate 1-4, with gate 3 split into two + the report-content test), 1 gate-0 corpus-size
test, 4 `RouterTransport` translation tests (`to_router_request`/`from_router_response`, request
+ response + failure-closed), 2 `resolve_transport_flag` tests, and 1 budget-gating test
(`run_bakeoff_call_refuses_before_any_transport_call_when_budget_unset`) — plus every H43a/H43b
test in this file, unchanged.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
&& cargo run -p pipeline --bin conformance -- us_house`
Expected: all green; `consensus-shadow-eval`'s bake-off-mode network paths are never exercised
by this command (manual, key-gated, exactly like H41b/H43b — "Not a CI gate").

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/consensus_eval.rs \
        crates/worker/src/lib.rs \
        crates/worker/src/bin/consensus-shadow-eval.rs
git commit -m "$(cat <<'EOF'
feat(worker): bake-off numeric gate report + RouterTransport (goal 021 Phase 3 H45)

Extends consensus-shadow-eval with --swap-model/--transport/--allow-router-
bakeoff/--report-out. Adds a pure, offline gate-arithmetic module computing
a gate-0 corpus-size precondition (>=400 docs, >=1,500 rows, >=150 degraded-
stratum docs -> INSUFFICIENT-CORPUS otherwise, never PROMOTE-ELIGIBLE) plus
the design amendment's four D4 gates (accuracy, schema-invalid rate,
decorrelation, ops checklist), rendering a founder-readable markdown report
naming every failing gate; PROMOTE-ELIGIBLE requires gate 0 and all four
numbered gates green. RouterTransport (OpenRouter-compatible) is bake-off-
only, its request/response translation offline-tested (mirroring H44's
VertexTransport), gated behind an explicit CLI flag (resolve_transport_flag)
the doc header marks non-production; production run_samples never
constructs it. The live bake-off path budget-gates (run_bakeoff_call) before
any transport call, same require_budget() discipline as H41b.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H46: Cross-lab production activation — GATED (D, D2)

**This task deliberately exceeds the 1h guideline**, same rationale as committed Task 24: any
split leaves `cargo test -p pipeline` (the E1-lock-supersession pin test, the regenerated
extraction-cache pin) or `cargo run -p pipeline --bin conformance -- us_house` red between
commits. Do not split it.

**BLOCKED-ON — all three, hard preconditions. Do not begin Step 1 until every command below is
green. This is not advisory.**

1. **H45's report is PROMOTE-ELIGIBLE for the chosen model.**
   ```bash
   test -f "docs/decisions/bakeoff/<vendor>-<model_id>.md"
   grep -q '^## Verdict: PROMOTE-ELIGIBLE$' "docs/decisions/bakeoff/<vendor>-<model_id>.md"
   ```
   (the committed copy of H45's `render_report_markdown` output for the chosen candidate — the
   bin's `--report-out` target, committed under `docs/decisions/bakeoff/` as the audit trail this
   precondition checks; substitute the real `<vendor>-<model_id>` H45 selected).

2. **HARD CAP values are SET** (`require_budget()` green) — **never invented, never assigned a
   number by this task**:
   ```bash
   grep -A3 '^\[budget\]' config/extractor.toml | grep -qE '^\s*max_batch_tokens\s*=\s*[0-9]'
   grep -A3 '^\[budget\]' config/extractor.toml | grep -qE '^\s*per_run_token_ceiling\s*=\s*[0-9]'
   ```
   If either grep fails: STOP. The HARD CAP HALT from `agents/goals/021-llm-extraction.md` is
   still open — do not proceed, do not pick a number.

3. **Conditional — only if the chosen model's vendor is NOT the Vertex path**: the founder
   legal-lane vendor-ToS goal (filed at the H45→H46 boundary per the goal's HALT protocol) is
   closed:
   ```bash
   grep -n '^\- \[x\].*cross-lab vendor ToS review' agents/goals/000-INDEX.md
   ```
   SKIP this check entirely if the chosen model is the Vertex-path Gemini 3.1 Flash-Lite (no new
   vendor, per the goal's ops-preference order).

**Read first:** `crates/pipeline/src/extraction/consensus.rs` for the LANDED (post committed
Tasks 1–23 + H27–H43) shapes of `ConsensusExtractor::new`/`ConsensusExtractor::extract`,
`run_samples`, `accumulate_stats`, `score` — the code below is the intended shape per the
shared interface contract, not a byte-exact transcript (Task 24's own convention, reused here
verbatim); `crates/pipeline/src/extraction/config.rs` for `ExtractorConfig`'s final field names
(`families`, `cross_lab`, `pricing`); `crates/worker/src/consensus_batch.rs` (H36/H37's poll-side
batch-topology file) for the exact `resolve_document`/escalation-reuse wiring this task's batch
change threads through — grep it fresh, its shape is another cluster's output.

**Files:**
- Modify: `crates/pipeline/src/extraction/consensus.rs` (`run_samples` arity — adds a cross-lab
  transport slot; `accumulate_stats` — per-pass pricing fix, see 3b below; `ConsensusExtractor::new`
  / `ConsensusExtractor::extract` — threads the cross-lab transport into the fan-out)
- Modify: `crates/pipeline/src/extraction/config.rs` (`[cross_lab] enabled = true` + chosen
  model; `[families]` gains the vendor model's family)
- Modify: `crates/pipeline/tests/consensus.rs` (Task 12's two existing `run_samples` tests gain
  the new `None`/cross-lab arg; two new tests added)
- Modify: `crates/worker/src/consensus_batch.rs` (batch topology: 2×Haiku via Anthropic Message
  Batches + ONE direct cross-lab call per doc at poll time, bounded concurrency — invariant 10;
  read first, this is an evolution of H36/H37's landed shape)
- Modify: `crates/adapters/us_house/fixtures/scanned_paper_ptr/extraction.cache.json`
  (regenerated, not hand-edited — composite model_id's models component changed)
- Modify: `crates/pipeline/tests/extraction.rs` (`CacheKey::new` literal, same site Task 24's
  3i touched)
- Modify: `docs/regimes/us-house/reference/E1.lock.json` (version 4 → 5)
- Modify: `docs/regimes/us-house.md` (§6 pol2/cross-lab addendum — rides this task's atomic v5
  re-pin cascade rather than a separate commit, since `docs/regimes/us-house.md` is itself an
  E1-pinned file; Task-24/26 split precedent, see H47)
- Test: `crates/pipeline/src/extraction/consensus.rs` (inline `#[cfg(test)]`);
  `crates/pipeline/tests/consensus.rs`

**Interfaces:**
- Consumes (frozen from H44): `pipeline::extraction::vendors::VertexTransport::from_adc(cfg:
  &CrossLabConfig) -> anyhow::Result<Self>`; `pipeline::extraction::Transport` (object-safe,
  `&dyn Transport` usable per `anthropic.rs`'s existing blanket impl); committed Task 24's
  `test_cfg`/`test_cfg_with_families` test-helper convention (`crates/pipeline/tests/
  consensus.rs`) — the shapes this task's Step 1a/1b/1c literals assume are the INTENDED shape
  per the shared plan contract, not a byte-exact transcript; per this doc's Global Constraints
  hedge-license sentence, verify against the landed helpers before wiring.
- Produces: `run_samples<T: Transport>(transport: &T, model: &str, images: &[Vec<u8>], spec:
  &ConsensusSpec, cfg: &ExtractorConfig, cross_lab: Option<&dyn Transport>) ->
  anyhow::Result<Vec<SamplePass>>` (arity change — evolves Task 12; when `cross_lab` is `Some`,
  the LAST of the N passes routes through it under `cfg.cross_lab.model` instead of `model`, and
  is stamped `SamplePass.model_id == cfg.cross_lab.model` — family attribution downstream is
  then mechanical, reading `SamplePass.model_id` alone); `accumulate_stats(passes: &[SamplePass],
  cfg: &ExtractorConfig) -> ExtractionStats` (arity change — drops the single `model: &str` param
  Task 12 shipped; see 3b, a genuine pricing-correctness fix this task's mixed-vendor vote set
  exposes).

- [ ] **Step 1: Write the failing tests**

**1a.** In `crates/pipeline/tests/consensus.rs`, update Task 12's two existing tests to the new
`run_samples` arity (behavior must be IDENTICAL — this alone proves the new parameter is
additive):

```rust
let passes = run_samples(&transport, &cfg.models.primary, &[vec![0u8; 4]], &spec, &cfg, None)
    .await
    .unwrap();
```

(apply the same trailing `None` to `run_samples_one_schema_invalid_pass_fails_the_whole_call`'s
call site too) and update both calls to `accumulate_stats(&passes, &cfg.models.primary, &cfg)`
to drop the model argument: `accumulate_stats(&passes, &cfg)`.

**1b.** Add two new tests to the same file:

```rust
#[tokio::test]
async fn run_samples_routes_the_last_pass_through_the_cross_lab_transport_when_supplied() {
    let mut cfg = test_cfg("claude-haiku-4-5-20251001");
    cfg.sampling.n = 3;
    let primary = MockTransport::returning(vec![
        tool_response_with_usage(&good_output(), 1000, 200),
        tool_response_with_usage(&good_output(), 1000, 200),
    ]);
    let cross_lab = MockTransport::returning(vec![
        tool_response_with_usage(&good_output(), 900, 180),
    ]);
    // H35b's page-aware consensus_spec(pages) — a blank placeholder page is
    // fine here: this test exercises run_samples' cross-lab routing, never
    // table_regions/template_recognized.
    let blank_page = image::GrayImage::from_pixel(1600, 2069, image::Luma([255u8]));
    let spec = consensus_spec(&[blank_page]);

    let passes = run_samples(
        &primary,
        &cfg.models.primary,
        &[vec![0u8; 4]],
        &spec,
        &cfg,
        Some(&cross_lab as &dyn pipeline::extraction::Transport),
    )
    .await
    .unwrap();

    assert_eq!(passes.len(), 3);
    assert_eq!(passes[0].model_id, cfg.models.primary);
    assert_eq!(passes[1].model_id, cfg.models.primary);
    assert_eq!(
        passes[2].model_id, "gemini-3.1-flash-lite",
        "run_samples must be told the cross-lab model id via cfg.cross_lab.model — read \
         config.rs's actual field path and adjust this literal if it differs"
    );

    assert_eq!(primary.requests().len(), 2, "only N-1 primary calls when a cross-lab slot fires");
    let cross_lab_requests = cross_lab.requests();
    assert_eq!(cross_lab_requests.len(), 1);
}

#[test]
fn accumulate_stats_prices_each_pass_at_its_own_model_rate() {
    // H46 defect found & fixed: Task 12's accumulate_stats priced EVERY pass
    // at ONE model's rate. That was invisible with an all-Haiku vote set;
    // it silently under/over-reports cost the instant one pass is a
    // different vendor at a different price. Read config.rs's real pricing
    // entry type/accessor and adjust the price-literal construction below —
    // the ASSERTION (mixed-vendor cost = per-pass sum, not uniform) must
    // not change.
    let mut cfg = test_cfg("claude-haiku-4-5-20251001");
    cfg.pricing.insert(
        "gemini-3.1-flash-lite".to_owned(),
        ModelPricing {
            input_per_mtok: rust_decimal::Decimal::new(26, 4), // illustrative test fixture only; the real config/extractor.toml [pricing] entry is set from H45's bake-off pricing sheet at H46 authoring time, not this literal
            output_per_mtok: rust_decimal::Decimal::new(26, 4),
        },
    );
    let passes = vec![
        SamplePass {
            model_id: cfg.models.primary.clone(),
            payload: good_output(),
            usage: json!({"input_tokens": 1000, "output_tokens": 200}),
        },
        SamplePass {
            model_id: cfg.models.primary.clone(),
            payload: good_output(),
            usage: json!({"input_tokens": 1000, "output_tokens": 200}),
        },
        SamplePass {
            model_id: "gemini-3.1-flash-lite".to_owned(),
            payload: good_output(),
            usage: json!({"input_tokens": 900, "output_tokens": 180}),
        },
    ];
    let stats = accumulate_stats(&passes, &cfg);

    let per_mtok = rust_decimal::Decimal::from(1_000_000u64);
    let haiku = cfg.pricing.get(&cfg.models.primary).unwrap();
    let gemini = cfg.pricing.get("gemini-3.1-flash-lite").unwrap();
    let expected = (haiku.input_per_mtok * rust_decimal::Decimal::from(2000u64)
        + haiku.output_per_mtok * rust_decimal::Decimal::from(400u64))
        / per_mtok
        + (gemini.input_per_mtok * rust_decimal::Decimal::from(900u64)
            + gemini.output_per_mtok * rust_decimal::Decimal::from(180u64))
            / per_mtok;

    assert_eq!(
        stats.estimated_cost, expected,
        "each pass must price at ITS OWN model's rate, not a single uniform rate"
    );
    assert_eq!(stats.input_tokens, 2900);
    assert_eq!(stats.output_tokens, 580);
}
```

**1c.** Add one property test to `crates/pipeline/src/extraction/consensus.rs`'s H30-added
`pol2_tests` submodule (reusing its existing local `spec()` helper — this crate is `pipeline`,
which `us_house` depends on, never the reverse, so `us_house::consensus::consensus_spec()` is
NOT reachable here; `pol2_tests::spec()` is the correct, already-landed local fixture):

```rust
#[test]
fn mixed_family_vote_set_two_haiku_agree_one_cross_lab_dissents_never_publishes_as_agreed() {
    // score()'s signature is `score(aligned: &[AlignedRow], spec:
    // &ConsensusSpec) -> Vec<RowVerdict>` (committed Task 3/H28, consuming
    // align()'s output) — it is NOT parameterized by cfg/families/model_id.
    // That is exactly H30's "dormant by construction" property: Agreed
    // (0.90) requires ALL N samples to agree, so a 2-of-3 vote can never
    // reach it regardless of which model cast which vote — no family
    // lookup is needed to prove this, only a disagreeing vote-set fixture.
    // Read consensus.rs first and adjust this call to the real signature if
    // it differs; the ASSERTION (a 2-of-3 agreeing pair over a dissent must
    // never resolve to Agreed) must not change.
    let s = spec();
    let haiku_value = json!({"filer": "Diana Harshbarger", "rows": [{"amount_raw": "$15,001 - $50,000"}]});
    let cross_lab_value = json!({"filer": "Diana Harshbarger", "rows": [{"amount_raw": "$50,001 - $100,000"}]});
    let samples = vec![haiku_value.clone(), haiku_value, cross_lab_value];

    let aligned = align(&samples, &s).unwrap();
    let outcome = score(&aligned, &s);
    assert!(
        matches!(outcome[0], RowVerdict::Disputed { .. }),
        "2 same-family agreeing samples over a cross-lab dissent must never publish 2/3 Agreed \
         under H46's real activation wiring — the structural guarantee H30 already proved"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pipeline --test consensus`
Expected: FAIL to compile — `run_samples`'s and `accumulate_stats`'s signatures don't yet
accept/drop the new parameters.

Run: `cargo test -p pipeline --lib extraction::consensus::pol2_tests::mixed_family_vote_set`
Expected: this test alone should already COMPILE and PASS against the landed `align`/`score`
(it is a structural property H30 already proved, re-verified here — it does not depend on any
new symbol this task adds); if the real `align`/`score` signatures differ from the Step-1c
disclaimer's assumption, adjust the call before this compiles at all.

- [ ] **Step 3: Write the minimal implementation**

**3a. Config activation.** In `config/extractor.toml`:

```toml
[cross_lab]
enabled = true
vendor = "vertex"
model = "gemini-3.1-flash-lite"
temperature = 1.0
```

(illustrative literal — the Vertex/Gemini path is the design's ops-preferred default; substitute
whatever model H45's PROMOTE-ELIGIBLE report actually named if a different candidate won).
Add the activated model to `[families]`:

```toml
[families]
"claude-haiku-4-5-20251001" = "anthropic"
"claude-sonnet-5" = "anthropic"
"gemini-3.1-flash-lite" = "google"
```

**3b. `accumulate_stats` per-pass pricing fix.** Change the signature to drop the redundant
`model: &str` parameter (every `SamplePass` already carries its own `model_id` — Task 12 priced
ALL passes at one model's rate, invisible under an all-Haiku vote set, silently wrong the moment
one pass is cross-lab):

```rust
#[must_use]
pub fn accumulate_stats(passes: &[SamplePass], cfg: &ExtractorConfig) -> ExtractionStats {
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut cache_read_tokens = 0u64;
    let mut estimated_cost = rust_decimal::Decimal::ZERO;
    let per_mtok = rust_decimal::Decimal::from(1_000_000u64);
    for pass in passes {
        let pass_input = pass.usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
        let pass_output = pass.usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
        input_tokens += pass_input;
        output_tokens += pass_output;
        cache_read_tokens += pass
            .usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if let Some(price) = cfg.pricing.get(&pass.model_id) {
            estimated_cost += price.input_per_mtok * rust_decimal::Decimal::from(pass_input) / per_mtok;
            estimated_cost += price.output_per_mtok * rust_decimal::Decimal::from(pass_output) / per_mtok;
        }
    }
    ExtractionStats {
        calls: passes.len() as u32,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        estimated_cost,
        agreement: serde_json::Value::Null,
    }
}
```

Update Task 12's two existing call sites (`crates/pipeline/tests/consensus.rs`) per Step 1a.

**3c. `run_samples` cross-lab slot.** Add the sixth parameter; when `Some`, the LAST pass routes
through it under `cfg.cross_lab.model`:

```rust
pub async fn run_samples<T: Transport>(
    transport: &T,
    model: &str,
    images: &[Vec<u8>],
    spec: &ConsensusSpec,
    cfg: &ExtractorConfig,
    cross_lab: Option<&dyn Transport>,
) -> anyhow::Result<Vec<SamplePass>> {
    let validator = jsonschema::validator_for(&spec.tool.input_schema)
        .map_err(|e| anyhow::anyhow!("compiling consensus schema: {e}"))?;
    let sampling = SamplingParams { temperature: Some(cfg.sampling.temperature), effort: None };
    let n = cfg.sampling.n;
    anyhow::ensure!(n >= 1, "extractor config sampling.n must be >= 1, got {n}");

    let primary_n = if cross_lab.is_some() { n.saturating_sub(1).max(1) } else { n };
    let concurrency: usize = std::env::var("GOVFOLIO_LLM_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4)
        .max(1);
    let semaphore = tokio::sync::Semaphore::new(concurrency);

    let first = run_one_pass(transport, model, images, spec, &sampling, &validator).await?;
    let rest: Vec<SamplePass> = if primary_n > 1 {
        let futures = (1..primary_n).map(|_| {
            run_one_pass_permitted(&semaphore, transport, model, images, spec, &sampling, &validator)
        });
        futures_util::future::try_join_all(futures).await?
    } else {
        Vec::new()
    };

    let mut passes = Vec::with_capacity(n as usize);
    passes.push(first);
    passes.extend(rest);

    if let Some(cross_lab_transport) = cross_lab {
        let cross_lab_pass = run_one_pass(
            cross_lab_transport,
            &cfg.cross_lab.model,
            images,
            spec,
            &sampling,
            &validator,
        )
        .await
        .with_context(|| format!("consensus cross-lab sample call ({})", cfg.cross_lab.model))?;
        passes.push(cross_lab_pass);
    }
    Ok(passes)
}
```

**3d. Threading into `ConsensusExtractor`.** Read the landed `ConsensusExtractor` struct/
constructor first; the intended shape is that `ConsensusExtractor::new` gains an optional
cross-lab transport reference (constructed by the caller, since building `VertexTransport`
is `async` and fallible — `ConsensusExtractor::new` itself stays sync per Task 24's call site):

```rust
// crates/adapters/us_house/src/extractor.rs::extract_live, after `cfg` loads:
let cross_lab_transport = if cfg.cross_lab.enabled {
    Some(pipeline::extraction::vendors::VertexTransport::from_adc(cfg.cross_lab.clone()).await?)
} else {
    None
};
let extractor = ConsensusExtractor::new(
    transport,
    cfg,
    cross_lab_transport.as_ref().map(|t| t as &dyn Transport),
);
```

`ConsensusExtractor::extract`'s internals call `run_samples(.., cross_lab)` instead of the
5-arg form (adjust the exact call site to whatever Task 24/H32 actually landed — the property
test in Step 1c is what proves this wiring didn't break family-aware scoring).

**3e. Batch topology (`crates/worker/src/consensus_batch.rs`).** Read H36/H37's landed
`resolve_document`/submit/poll shape first. The activated topology (D6, decided): 2×Haiku ride
the existing Anthropic Message Batches submission unchanged; ONE direct cross-lab call per
document fires at POLL time (not inside the batch), bounded by the same
`GOVFOLIO_LLM_CONCURRENCY` semaphore invariant 10 already governs, reusing H37's
escalation-pass-reuse guard pattern (check `extraction_sample` for an existing pass with
`model_id == cfg.cross_lab.model` before firing, so a poll retry never double-fires the vendor
call). Record the move-to-vendor-batch trigger as a code comment, not a mechanism (amendment
§4/D6: "if poll-time cost/scale warrants" — no threshold exists yet, so no code branches on
one):

```rust
// Cross-lab call rides poll time, not batch submission (design amendment
// §4/D6: keeps ONE composite per document, uniform sync/batch semantics).
// Trigger to move this to the vendor's own batch API (Gemini batch offers
// -50%) if poll-time cost/scale later warrants it — not yet measured,
// intentionally no threshold coded here.
if cfg.cross_lab.enabled
    && !has_existing_pass(pool, doc_sha256, &cfg.cross_lab.model).await?
{
    let cross_lab_transport = pipeline::extraction::vendors::VertexTransport::from_adc(cfg.cross_lab.clone()).await?;
    let _permit = poll_semaphore.acquire().await?;
    // .. fire the one cross-lab pass, persist it into extraction_sample
    // through the SAME persist_consensus_run entry point the sync path uses ..
}
```

(this is the intended shape per the shared contract's D6 topology decision — adjust
`has_existing_pass`/`persist_consensus_run` call sites to whatever H37 actually named).

**3f. Fixture cache regen + CacheKey literal.** After 3a lands:

```bash
UPDATE_EXTRACTION_CACHE=1 cargo test -p pipeline extraction_cache_entry_is_primed_from_test_designer_ground_truth
```

Confirm the regenerated `crates/adapters/us_house/fixtures/scanned_paper_ptr/extraction.cache.json`'s
`key.model_id` reflects the new composite (models component now folds the activated cross-lab
model in — confirm the EXACT string `composite_model_id(&ExtractorConfig::load().unwrap())`
produces in a scratch test before hand-typing it, same discipline as Task 24's 3i). Then update
`crates/pipeline/tests/extraction.rs`'s `CacheKey::new` literal (the same site Task 24's 3i
touched) to that confirmed string.

**3g. `docs/regimes/us-house.md` §6 pol2/cross-lab addendum.** Immediately after the item-6
paragraph committed Task 24's step 3n appended (before `## 7. Conformance fixtures`), append a
new dated paragraph — NOT a new numbered item, this is the SAME item-6 flow, hardened. This
write-back lands HERE (inside H46's atomic commit), not in H47, because `docs/regimes/
us-house.md` is itself pinned by `docs/regimes/us-house/reference/E1.lock.json` — editing it
outside the commit that bumps the lock would leave the pin transiently stale (Task-24/26 split
precedent: a pinned file's write-back travels with the commit that changes its pinned hash):

```
   **2026-07-08 update (goal 021 Phase 3 hardening, pol2):** the N-sample comparator described
   above now runs two-plane comparison — a canonicalized plane (NFKC/casefold/whitespace/dash
   for asset text, ISO date parse with fail-closed unparseable handling) drives alignment,
   agreement, and premium matching; the PUBLISHED value is always one of the model's own
   verbatim strings (modal verbatim among canonical-equals). Escalation acceptance is strict
   vote-margin-stratified: a value publishes at `CONF_ESCALATED = 0.75` only when it holds
   >= 3 of the 4 readers (3 samples + the one premium pass); a 1/1/1 scatter plus a matching
   premium vote is 2-of-4 and HOLDS, it does not publish. Three deterministic recall/precision
   guards run ahead of any coordinate-based pixel check: a row-count completeness gate
   (horizontal-rule projection profile vs. published+held row count, doc-level 0.79 cap +
   `row_count_mismatch` review task on mismatch) and a form-revision template fingerprint guard
   (unknown template -> every pixel/row-count check emits no votes and no caps, logged
   `template_unrecognized`); free preprocessing by-products (residual skew, Otsu variance,
   noise count) additionally route flagged documents through 3 Haiku samples + the premium pass
   up front, with `CONF_AGREED = 0.90` on a flagged document additionally requiring premium
   concordance. **Cross-lab third vote:** ACTIVE as of 2026-07-08 — one of the three sample
   passes per document routes through <vendor>/<model> (H46, chosen via H45's bake-off gate
   report; substitute the actual PROMOTE-ELIGIBLE candidate here). Voting is family-aware: the
   two same-family Anthropic samples agreeing over a cross-lab dissent on a critical field is
   ALWAYS a dispute (escalates), never a 2/3 publish — cross-lab concurrence keeps `0.90`
   unchanged (the gain is fewer wrong `0.90`s, not a higher confidence ceiling). Kill-switch:
   `[cross_lab] enabled = false` reverts to 3xHaiku under a different `composite_model_id`
   (cache-key-isolated, no cross-contamination with cross-lab-tagged cache entries). See
   `docs/plans/2026-07-07-consensus-extraction-amendment-1.md` §4 for the full numeric bake-off
   gates and `docs/regimes/us-house/reference/E1.lock.json` v5 for the re-pinned fixture trail.
```

Then re-run the validator (same discipline as committed Task 26 — this doc is `sources.yaml`-
adjacent, not `sources.yaml` itself):

```bash
cargo run -p pipeline --bin validate-survey -- us_house
```

Expected: PASS (unchanged front-matter, only prose body grew).

**3h. `docs/regimes/us-house/reference/E1.lock.json` → version 5.** Same mechanics as Task 24's
step 3o, exactly:

```bash
git show HEAD:docs/regimes/us-house/reference/E1.lock.json | sha256sum
```

Then bump to:

```json
{
  "version": 5,
  "epoch": "E1",
  "reference": "us_house",
  "frozen_at_utc": "2026-07-05T00:00:00Z",
  "policy": "... (copy verbatim from v4) ...",
  "supersedes": "<sha256 computed above>",
  "reason": "goal 021 Phase 3 H46 cross-lab production activation (agents/goals/021-llm-extraction.md Phase 3; H45 bake-off PROMOTE-ELIGIBLE for the chosen model; HARD CAP values confirmed set; conditional vendor-ToS goal closed if applicable): re-pins scanned_paper_ptr/extraction.cache.json (regenerated because composite_model_id's models component now includes the activated cross-lab vendor/model, changing the cache key) AND docs/regimes/us-house.md (this task's own step 3g §6 pol2/cross-lab dated addendum, appended inside this SAME atomic commit — Task-24/26 split precedent: a pinned file's write-back travels with the commit that changes its pinned hash). No other pinned file's bytes changed (extractor tag us_house_ptr/consensus@1, prompt version p2, policy version pol2 are all untouched by this activation — activation is config + provenance only, per the goal's D2 resolution).",
  "date": "<execution date>",
  "pins": {
    "... copy every v4 key verbatim, recomputing sha256sum for scanned_paper_ptr/extraction.cache.json AND docs/regimes/us-house.md ..."
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pipeline --test consensus`
Expected: PASS — Task 12's two tests (unchanged behavior, `None` cross-lab slot) plus the two
new H46 tests (cross-lab routing + per-pass pricing).

Run: `cargo test -p pipeline --lib extraction::consensus`
Expected: PASS — including `mixed_family_vote_set_two_haiku_agree_one_cross_lab_dissents_never_publishes_as_agreed`.

Run: `cargo test -p pipeline`
Expected: PASS, including `pipeline::evals::reference` (verifies the v4→v5 supersession trail)
and the regenerated `extraction_cache_entry_is_primed_from_test_designer_ground_truth`.

Run: `cargo run -p pipeline --bin conformance -- us_house`
Expected: `5/5` green, offline (the regenerated cache entry round-trips the same as before —
cache key changed, cached VALUE did not).

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add config/extractor.toml \
        crates/pipeline/src/extraction/consensus.rs \
        crates/pipeline/tests/consensus.rs \
        crates/pipeline/tests/extraction.rs \
        crates/worker/src/consensus_batch.rs \
        crates/adapters/us_house/fixtures/scanned_paper_ptr/extraction.cache.json \
        docs/regimes/us-house/reference/E1.lock.json \
        docs/regimes/us-house.md
git commit -m "$(cat <<'EOF'
feat(pipeline,worker): activate the cross-lab third vote — GATED (goal 021 Phase 3 H46)

Flips [cross_lab] enabled=true behind H45's PROMOTE-ELIGIBLE verdict, the
HARD CAP precondition, and the conditional vendor-ToS closure. run_samples
now routes 1-of-N sample passes through the activated vendor transport
(SamplePass.model_id makes family attribution mechanical downstream, no new
plumbing needed); [families] gains the vendor model. Fixes a pricing defect
accumulate_stats carried since Task 12 (uniform single-model pricing —
invisible under an all-Haiku vote set, silently wrong under a mixed one) by
pricing each pass at its own model's rate. Batch topology: 2xHaiku via
Anthropic Message Batches + one direct cross-lab call per document at poll
time, escalation-pass-reuse guarded. composite_model_id's models component
changed -> fixture cache regenerated, E1 lock v4->v5 (re-pins the fixture
cache AND docs/regimes/us-house.md, which gains its own dated §6 pol2/
cross-lab addendum inside this SAME atomic commit — Task-24/26 split
precedent, since us-house.md is itself E1-pinned). Family-aware pol2
semantics (H30) were already live; this
task activates config + provenance only, re-verified live by a property
test: 2 same-family agreeing samples over a cross-lab dissent never publish
2/3 Agreed.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

---

### Task H47: SAF write-backs + Phase-3 close-out (D7)

**Files:**
- Modify: `docs/regimes/us_house/AUTHORITY.md` (append to `## Quirks log (append-only, dated)`)
- Modify: `agents/goals/021-llm-extraction.md` (tick Phase-3 execution checklist boxes that
  landed)
- Modify: `agents/JOURNAL.md` (append one line)
- Modify: `agents/goals/000-INDEX.md` (021 line status update)
- Test: none (docs/agents-only task; acceptance is the validator commands below, mirroring
  committed Task 26's structure exactly)

**Interfaces:**
- Consumes: H27–H46's landed facts (pol2 family-aware vote table, H29's ≥3-of-4 escalation
  acceptance, H28's two-plane canonical/verbatim comparison, H31/H32/H34/H35a-b's
  quality/pixel/row-count/template guards, H46's activated `[cross_lab]` vendor/model and E1
  lock v5) — read the actual committed diffs from H27–H46 before writing prose here, exactly as
  committed Task 26 required for Tasks 24/25. Do not describe what this task's authors GUESSED
  would ship; describe what actually did.
- Produces: nothing consumed by a later task — this is the Phase-3 hardening group's terminal
  write-back, same role committed Task 26 played for Phase 2.

- [ ] **Step 1: Confirm the state this task writes back**

Run (read-only, grounds the write-back in what's actually true post-H46):

```bash
git log --oneline -10
cargo run -p pipeline --bin validate-survey -- us_house
cargo run -p pipeline --bin validate-sources -- us_house
```

Expected: both validators green BEFORE this task's edits — if either is red, STOP per CLAUDE.md's
ambiguity rule; something upstream in H27–H46 did not land cleanly and this task must not paper
over it.

Also confirm `docs/regimes/us-house.md` §6 already has the item-6 paragraph committed Task 24's
step 3n appended (starts `**Consensus extraction (goal 021 Phase 2, ...`) AND the 2026-07-08
pol2/cross-lab dated addendum H46 appended to the SAME §6 flow (starts `**2026-07-08 update
(goal 021 Phase 3 hardening, pol2):`) — this task does NOT write §6 itself; it rode H46's atomic
v5 re-pin cascade instead (Task-24/26 split precedent: us-house.md is itself E1-pinned, so its
write-back travels with the commit that changes its pinned hash). Also confirm
`agents/goals/021-llm-extraction.md` still has its `## Phase 3` section with the "Execution
(merged order...)" checklist this task ticks. If any of these is missing: HALT — do not invent
it; it means an earlier task in the sequence did not land as this addendum expected.

- [ ] **Step 2: N/A (docs-only task, no test to run before writing)**

Skipped, same as committed Task 26 — proceed to Step 3.

- [ ] **Step 3: Write the write-back**

**§6 note:** `docs/regimes/us-house.md` §6's 2026-07-08 pol2/cross-lab dated addendum is NOT
written by this task — it rode H46's atomic v5 re-pin cascade instead (Task-24/26 split
precedent: `docs/regimes/us-house.md` is itself pinned by `docs/regimes/us-house/reference/
E1.lock.json`, so its write-back must travel with the SAME commit that bumps the lock's pinned
hash for it — a separate commit would leave the pin transiently stale). Step 1 above already
confirmed it landed. This task's write-backs start at the AUTHORITY.md quirks log.

**3b. `docs/regimes/us_house/AUTHORITY.md`.** Append to the END of the `## Quirks log
(append-only, dated)` section (after whatever H27–H43 already appended there — `tail` the file
first, do not assume a stale line number):

```
- 2026-07-08 · **Consensus hardening + cross-lab third vote activated (goal 021 Phase 3,
  founder-issued 2026-07-07, hardening addendum H27-H46)**: pol2 supersedes pol1 — family-aware
  vote counting (same-family agreement over a cross-lab dissent is always a dispute, never a
  2/3 publish), strict >=3-of-4 escalation acceptance (a 1/1/1 scatter + matching premium is
  2-of-4 and holds), two-plane canonical/verbatim comparison, quality-routed vote sets, a
  pixel-ambiguity-triggered premium vote on otherwise-unanimous rows, a row-count completeness
  gate, and a form-revision template fingerprint guard (unknown template -> pixel/row-count
  checks emit nothing, `template_unrecognized` logged). Cross-lab: `[cross_lab] enabled = true`,
  vendor/model chosen via H45's bake-off gate report (PROMOTE-ELIGIBLE — accuracy, schema-
  validity, decorrelation, and ops gates all green; see
  `docs/decisions/bakeoff/<vendor>-<model_id>.md`); one of the three sample passes per document
  now runs cross-lab, `[families]` maps it to its own family. `composite_model_id`'s models
  component changed accordingly -> E1.lock.json v5 (supersedes v4, re-pins
  `scanned_paper_ptr/extraction.cache.json` only). Kill-switch: `[cross_lab] enabled = false`
  reverts to 3xHaiku under a distinct composite (cache-isolated). Measured template revisions:
  fold in anything H35a/b measured beyond the single committed 2026-v1 fixture revision that
  is not already recorded in this log — none beyond the H35b entry as of this write-back.
```

Then:

```bash
cargo run -p pipeline --bin validate-survey -- us_house
```

Expected: PASS (append-only Quirks log grew, front-matter unchanged).

**3c. `agents/goals/021-llm-extraction.md`.** Under the `## Phase 3` section's "Execution
(merged order...)" checklist, tick every box that H27–H46 (and the interleaved committed
Tasks 1–26, if not already ticked from Phase 2) actually landed by the time this task runs —
confirm each via `git log` before ticking, per Step 1. Leave any box describing work outside
this addendum's actual commit provenance unchecked. Tick the Phase-3 "Execution
(merged order...)" H44–H47 line itself last, once this task's own commit is about to land.

**3d. `agents/JOURNAL.md`.** Append exactly one line at the end of the file:

```
2026-07-08 | 021 Phase 3 hardening | Landed the goal-021 Phase 3 hardening addendum (H27-H47) over the committed Phase-2 consensus extractor: pol2 family-aware voting (same-family agreement over a cross-lab dissent is always a dispute), strict >=3-of-4 escalation acceptance, two-plane canonical/verbatim comparison, quality/pixel/row-count/template-fingerprint guards, and a gated cross-lab third vote (H44 Vertex translation transport DISABLED by default; H45 numeric bake-off gates; H46 activation behind PROMOTE-ELIGIBLE + HARD CAP + conditional vendor-ToS preconditions, E1 lock v4->v5). | No blockers at close-out — H27-H47 landed; HALT items from goal registration (HARD CAP values, conditional vendor ToS) resolved as H46's own hard preconditions, not carried forward.
```

**3e. `agents/goals/000-INDEX.md`.** Replace the Phase 3 clause of the existing 021 line (the
text starting `· Phase 3 (consensus hardening + cross-lab third vote) OPEN 2026-07-07: planning
session 2026-07-08 → ...`) with:

```
· Phase 3 (consensus hardening + cross-lab third vote) DONE 2026-07-08: hardening addendum
(docs/plans/2026-07-07-consensus-hardening.md, Tasks H27-H47) executed — pol2 family-aware +
two-plane + quality-routed consensus scoring, gated cross-lab third vote ACTIVE
(docs/regimes/us-house/reference/E1.lock.json v5); HALT items closed as H46's own preconditions
(HARD CAP values, conditional vendor ToS)
```

Leave the v1 and Phase-2 clauses of the line byte-identical (SAF discipline: never rewrite
history, only append/replace the clause this phase owns).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo run -p pipeline --bin validate-survey -- us_house`
Expected: PASS.

Run: `cargo run -p pipeline --bin validate-sources -- us_house`
Expected: PASS (unchanged — this task never touches `sources.yaml`).

Run: `git diff --stat main` (or the addendum's base branch)
Expected: only `docs/regimes/us_house/AUTHORITY.md`, `agents/goals/021-llm-extraction.md`,
`agents/JOURNAL.md`, `agents/goals/000-INDEX.md` appear — `docs/regimes/us-house.md` does NOT
appear here (it rode H46's cascade, an earlier commit); no source files touched by this task.

Then: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
&& cargo run -p pipeline --bin conformance -- us_house`
Expected: all green — unaffected by a docs-only commit, run anyway per the addendum's per-commit
green-gate convention.

- [ ] **Step 5: Commit**

```bash
git add docs/regimes/us_house/AUTHORITY.md \
        agents/goals/021-llm-extraction.md \
        agents/JOURNAL.md \
        agents/goals/000-INDEX.md
git commit -m "$(cat <<'EOF'
docs(us_house,agents): write back goal 021 Phase 3 hardening + cross-lab activation close-out

Records pol2's family-aware/two-plane/quality-routed hardening and the
activated cross-lab third vote in the AUTHORITY.md quirks log (us-house.md
§6's own dated addendum already landed inside H46's atomic v5 re-pin
cascade, since us-house.md is itself E1-pinned — Task-24/26 split
precedent). Ticks the goal 021 Phase 3 execution checklist for H27-H47,
closes out the JOURNAL entry, and updates the 000-INDEX 021 line from OPEN
to DONE — the HARD CAP and conditional vendor-ToS HALT items close as
H46's own gated preconditions rather than carrying forward.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_0124TFXohqaJyvQAu4U4TveR
EOF
)"
```

## Conflicts & findings

1. **pg-cache regression (committed Task 20, pre-existing):** `persist_published` cached raw DTO rows while the amended Task 24 `validated()` requires SilverRow + tag — fatal on any tier-2 re-touch, guaranteed under the letter DTO. Fixed by the Task-20 surgical edit (Silver-shaped `&[StagingRow]`); batch side fixed by H36.
2. **`high_impact` no-op (committed Task 24):** computed and discarded (`let _high_impact_document`), silently dropping the §6.3 "regardless of agreement" floor. Fixed via H32's trigger disjunction, consistent with the Phase-2 controller resolution recorded in `.superpowers/sdd/progress.md`.
3. **Committed dedup inconsistency:** Task 3's test asserted `competing.len()==2` (deduped) while Tasks 13/17 assert `==3` (per-sample) — mutually unsatisfiable; resolved to per-sample UNdeduped (multiplicity is load-bearing for A1/A14).
4. **`field_resolution` dedup-corrupted counting:** a 2v1 split counted 1v1, so premium siding with the 1-of-3 minority PUBLISHED the minority at 0.75 — strictly worse than the goal's "plurality" wording. Fixed by H29.
5. **Few-shot self-reference:** the ONE live smoke targets 9115811 while its transcription rides the prompt (A5) — value assertions degrade to mechanics (amended Task 25); hard assertions return on a refill artifact (H42). Zero new live tests.
6. **LOOP.md `## BLOCKED (human)` vs automation-policy halt-files-a-goal:** the automation policy governs (same resolution as the committed plan's Conflicts #13).
7. **Quarantine:** untracked `agents/goals/022-*.md`/`023-*.md` + root `.agents/` remain 000-INDEX-unlisted → surfaced, never read or followed (invariant 9).
8. **In-flight executor race:** the Phase-2 executor may have started Tasks 2–3 before the surgical edits landed; the edits + hardening plan landed in ONE atomic commit, banners point here, and H27's reclassification rule converts any too-late edit into a code-amendment task. Ledger + `git log` are the arbiters.
9. **Global-Constraint E1 line:** the committed plan's "only inside Task 24 (v4)" wording letter-conflicted with H46's v5 — amended in place (banner edit), recorded here.
10. **Numbering:** H-tasks continue the committed plan's task-number space (27+, with a/b splits where a reviewer found >1h granularity); the goal file's Phase-3 checklist cites H-numbers against THIS doc's path.

## HALT items (automation-policy: a halt files a goal; the loop continues)

- **HARD CAP values — RESOLVED 2026-07-08 (founder, chat): USD 200/month**, subdivided in the amended Task 9 `[budget]` block (`max_batch_tokens = 20_000_000` per submission ≈ $25; `per_run_token_ceiling = 80_000_000` per bin run ≈ $100; derivation comment lives with the values). `require_budget()` remains the fail-closed gate; H41b/H45/H46 unblock mechanically once Task 9 lands. Anthropic-console $200 monthly limit = platform-side backstop covering the un-gated sync path. No follow-up goal — resolved before filing.
- **Non-Google vendor ToS** (conditional, founder legal lane): if H45's winner is not the Vertex path, file the vendor-ToS goal at the H45→H46 boundary; H46 stays blocked until it closes. Never filed preemptively.

## Execution handoff

Plan complete. Two options: **1. Subagent-Driven (recommended)** — fresh subagent per task with task review between tasks (superpowers:subagent-driven-development), following the Merged Execution Order and coordinating with the Phase-2 executor via the shared ledger; **2. Inline Execution** — one session executes lane-by-lane with checkpoints (superpowers:executing-plans). Either way: check `.superpowers/sdd/progress.md` + `git log` before dispatching ANY task — two controllers share this branch.
