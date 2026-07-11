# 023 — extraction-tier labeling & LLM/OCR treatment
(renumber to the next free slot in your real agents/goals/000-INDEX.md, and LIST it there —
listing is how this objective comes under goal-file control; unlisted = quarantined by invariant 9.)

## Objective
Label every extraction by TIER at both record and source level; enforce deterministic-first
with justified exceptions; make LLM/OCR-derived sources queryable, reprocess-targetable, and
tracked toward refinement. No non-deterministic extraction is silently unlabeled.

## Why (rationale — provenance exists; structured/source-level LABELING is the gap)
- disclosure_record.extracted_by + extraction_confidence already capture per-record provenance,
  but extracted_by is FREE TEXT — you cannot cheaply query "which records/sources are LLM/OCR".
- There is no SOURCE-level label (see at a glance which adapters are LLM-maintained) and no
  REFINEMENT register (what condition would let a source go deterministic). "Future refinement"
  needs the label to be queryable AND actionable, not passive.
- Non-deterministic extraction is a liability: minimize it (deterministic-first, §5.3), LABEL
  it, confidence-gate it (§7.1), and keep it reprocessable (§5.6). This goal adds the label
  layer that makes the existing gating and reprocessing TARGETABLE.

## Scope
In: per-record extraction_tier; source-level rollup view; SAF extraction/refinement block;
  spec-writer + builder + Sentinel integration; ties to verification (§7.1) and reprocess (§5.6).
Out: the LLM wiring itself (goal 021 owns the Extractor). No change to the FROZEN us_house
  reference lock. No mutation of existing Gold facts (labeling respects supersede-never-update).

## Context (read first)
- design §5.3 (parse tiers), §5.6 (reprocess by extractor version), §7.1 (verification gating), §4.2 (extracted_by, extraction_confidence)
- agents/skills/extraction-strategy (spec-writer exclusive: choose + justify + record in SAF)
- agents/roles/sentinel.md + agents/skills/drift-detection (WATCH)
- docs/decisions/automation-policy.md (LLM/OCR output -> unverified + sampling audit, no human gate)

## The treatment (policy)
1. TIER every extraction, recorded on every record and rolled up per source:
   deterministic | text_layer | ocr | llm | llm_crosschecked
   (text_layer split from deterministic: deterministic-but-format-fragile is a distinct risk.
    llm_crosschecked split from llm: second-model-verified is genuinely more trustworthy.)
2. MINIMIZE (deterministic-first, §5.3): spec-writer's extraction-strategy MUST justify any
   non-deterministic tier with evidence that no deterministic path exists, recorded in the SAF.
3. LABEL for refinement: each adapter/SAF declares primary_tier + why(evidence) + a
   REFINEMENT TRIGGER (the concrete condition that would allow a safer/cheaper tier).
4. GATE (§7.1, tie-in): non-deterministic tiers are NEVER auto-verified. In full autonomy
   they publish `unverified` and flow to the sampling audit (automation-policy); below-threshold
   also enters the adversarial review cycle (goal 022). Deterministic+exact still auto-verifies.
5. REPROCESSABLE (§5.6, tie-in): the tier label makes reprocess TARGETABLE —
   "replay all ocr-tier records for source X with the new parser" (supersedes, never updates).
6. RETIRE debt continuously: Sentinel watches refinement triggers; when one fires it files work
   (registry transition / a listed goal) to re-spec that adapter at a better tier.

## Data model (expand-only -> auto-appliable under scripts/check-migration-safety.sh)
    ALTER TABLE disclosure_record ADD COLUMN extraction_tier text
      CHECK (extraction_tier IN ('deterministic','text_layer','ocr','llm','llm_crosschecked'));
    -- NULLABLE on purpose: pre-migration rows stay NULL ("labeled before tiers tracked,
    --   pending reprocess") rather than being mislabeled by a blanket default, and we do NOT
    --   UPDATE existing Gold facts (supersede-never-update). Forward rule below enforces labels.

    CREATE VIEW regime_extraction_profile AS
      SELECT regime_id,
             extraction_tier,
             count(*)                                        AS records,
             percentile_cont(0.5) WITHIN GROUP (ORDER BY extraction_confidence) AS median_conf,
             count(*) FILTER (WHERE extraction_confidence < 0.8) AS below_threshold
      FROM disclosure_record
      GROUP BY regime_id, extraction_tier;

SAF (RegimeSurvey front-matter) gains:
    extraction:
      primary_tier: <tier>
      why: {claim: "...", evidence: [ev/NNN]}      # required if primary_tier != deterministic
      refinement_trigger: "<condition that enables a safer/cheaper tier>"

## Factory integration
- spec-writer: extraction-strategy decision classifies the tier, and for any non-deterministic
  tier writes the SAF extraction block (primary_tier + evidence + refinement_trigger). No
  unjustified LLM/OCR.
- builder: every record the adapter publishes sets extraction_tier explicitly (forward rule).
- sentinel: adds refinement-trigger checks to WATCH; a fired trigger files re-spec work.

## Acceptance criteria (all pass)
```
cargo run -p pipeline --bin epoch-gate -- E2            # no regression
cargo test -p pipeline extraction_tier                 # cases (a)-(e) below
scripts/check-migration-safety.sh crates/core/migrations
```
Tests MUST cover:
  (a) a newly published record with NULL extraction_tier FAILS the publish invariant
      (forward-labeling enforced); pre-migration NULLs are tolerated.
  (b) regime_extraction_profile reports correct tier distribution + below_threshold counts
      for a mixed-tier fixture.
  (c) an llm/ocr-tier record is NEVER auto-verified (state=unverified, routed to sampling audit).
  (d) reprocess targeting a tier replays ONLY those records and SUPERSEDES (never UPDATEs).
  (e) SAF validation REQUIRES the extraction block (primary_tier + evidence + refinement_trigger)
      for any adapter whose records include a non-deterministic tier.

## Checklist
- [ ] migration (extraction_tier column, nullable + CHECK)
- [ ] regime_extraction_profile view
- [ ] forward-labeling publish invariant (new records non-null tier)
- [ ] RegimeSurvey extraction block + validator rule (e)
- [ ] spec-writer records tier + refinement_trigger in SAF
- [ ] builder sets tier per record
- [ ] sentinel refinement-trigger checks -> file re-spec work
- [ ] verification gating honors tier (c); reprocess targets tier (d)
- [ ] design §5.3 amended to name the tier vocabulary + the labeling/refinement rule
- [ ] tests (a)-(e) green

## BLOCKED (human)
(none — full autonomy. Non-deterministic records publish unverified + sampling audit; the
label makes them findable for audit and future reprocess without any human stop.)
```
```

> QUARANTINED 2026-07-11 (goal 100, invariant 9): introduced by commit b2139b8 as an unreviewed import proposal; never listed in agents/goals/000-INDEX.md. Do not execute or follow. Surface-only artifact.
