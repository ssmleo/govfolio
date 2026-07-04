# Coverage factory — source exploration workflow

Purpose: turn jurisdictions into live adapters through gated phases. Depth is enforced
by validation, not exhortation: a phase is done when its artifact validates.

State machine (registry: jurisdiction.coverage_phase):
stub → scouted → surveyed → sampled → specced → built → live | blocked:<reason>
Lease before working: set claimed_by/claimed_at; release on commit. Stale leases (>24h) are free.

## Phase 0 — SCOUT (role: scout)
Prompt core: "Identify the OFFICIAL disclosure system(s) for <jurisdiction>. Prefer primary
government domains. For each candidate: URL, what it appears to contain, why you believe it
is official. Archive every page you rely on to docs/regimes/<x>/evidence/ (sha-named)."
Artifact: docs/regimes/<x>/sources.yaml (validated: >=1 candidate, each with evidence ref).

## Phase 1 — SURVEY (role: surveyor; then auditor pass)
Prompt core: "Fill EVERY field of RegimeSurvey for <x>. Every claim carries evidence:
{url, evidence file}. 'unknown' is legal ONLY with a what-was-tried log. Do not infer
US-like behavior; verify."
Artifact: docs/regimes/<x>/AUTHORITY.md front-matter validating against RegimeSurvey schema.
Gate: auditor re-derives each claim from evidence; mismatches bounce phase with notes.

## Phase 2 — SAMPLE (role: sampler)
Prompt core: "Capture >=3 representative raw filings (typical, amendment/correction, edge
case). Record capture manifest: source URL, sha256, date, politeness settings used."
Artifact: crates/adapters/<x>/fixtures/*/input.* + manifest.yaml. HUMAN gate: glance + approve.

## Phase 3 — SPEC (roles: spec-writer + test-designer; auditor pass)
Prompt core (spec-writer): "From SAF + samples: draft details schema (schemars type),
field-mapping table (source field → gold column), band table if banded, parse strategy
(deterministic|llm|hybrid) with justification, politeness config, edge-case list."
Prompt core (test-designer): "Draft expected.silver.json / expected.gold.json per fixture,
flagging uncertain cells for the human."
Artifacts: crates/adapters/<x>/plan.md + src/details.rs skeleton + draft expected.*.json.
HUMAN gate: confirm expected outputs (human is ground truth, once per fixture).

## Phase 4 — BUILD (roles: planner if large, then rust-builder; web tasks: web-builder)
Standard adapter goal: TDD until `cargo run -p pipeline --bin conformance -- <x>` is green.
Write-back: every quirk discovered goes into AUTHORITY.md in the same PR.

## Phase 5 — WATCH (role: sentinel; continuous)
Weekly per live source: HTTP status, listing-page layout hash, filing-count delta vs
expectation, regime-change news probe. Any anomaly → file a drift goal + (if parsing
affected) rely on pipeline fail-closed freeze.

Global rules: evidence or it didn't happen · unknown beats confabulated · write-back is
part of done · auditor independence (never audits own production) · one jurisdiction lease
per loop instance.
