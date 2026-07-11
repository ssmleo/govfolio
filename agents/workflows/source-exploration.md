# Coverage factory — source exploration workflow

Purpose: turn jurisdictions into live adapters through gated phases. Depth is enforced
by validation, not exhortation: a phase is done when its artifact validates.

State machine (registry: jurisdiction.coverage_phase):
stub → scouted → surveyed → sampled → specced → built → live | blocked:<reason>
Claim through `jurisdiction-lease` and retain its generation. Renew/abandon are
generation-CAS; producers never write lease fields or phase directly. After local commit,
submit an immutable receipt and wait. The integrator applies phase and terminal release.

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

## Artifact schemas (DERIVED 2026-07-04, goal 015 setup — enforced by the validators)

The phase descriptions above left the artifact shapes underspecified; these minimal
schemas were derived from them and are now normative. The validators
(`cargo run -p pipeline --bin validate-sources|validate-survey|validate-manifest -- <x>`,
source: `crates/pipeline/src/factory.rs`) FAIL CLOSED outside them: missing file,
missing key, missing section, missing evidence URL, or any undocumented key rejects.
Evolving a shape = amend this section + the validator tests in the same PR.

### sources.yaml (Phase 0 gate)
```yaml
jurisdiction: <x>            # must equal the registry id being validated
candidates:                  # >= 1
  - url: https://...         # http(s) only
    contains: "..."          # what it appears to contain (non-empty)
    official_rationale: "..."# why you believe it is official (non-empty)
    evidence:                # >= 1 bare file names (no paths), each existing
      - <sha-named file>     #   at docs/regimes/<x>/evidence/
    notes: "..."             # optional
notes: "..."                 # optional
```
No other keys. Evidence refs are bare names because Phase 0 archives whole pages
sha-named into the evidence dir; the candidate's own `url` is the provenance.

### AUTHORITY.md front-matter = RegimeSurvey (Phase 1 gate)
Every template key is REQUIRED (see `docs/regimes/_templates/AUTHORITY.template.md`);
unknown keys reject. Field rules:
- Claim fields `legal_basis, who_files, cadence_and_lag, amendment_mechanism,
  tos_and_politeness` (and `historical_depth` with `from` instead of `claim`):
  `{claim: "...", evidence: [...], tried: [...]?}`.
  - claim != "unknown" → `evidence` non-empty; each item is `{url: https://..., file: <name>}`
    with the file existing under `docs/regimes/<x>/evidence/` (evidence carries BOTH the
    source URL and the archived copy).
  - claim == "unknown" → non-empty `tried` list (what-was-tried log) required instead.
- `jurisdiction` == the id being validated; `bodies`, `formats`, `language`: non-empty
  string lists.
- `record_types`: non-empty, each in the core `RecordType` vocabulary
  (transaction|holding|interest|change_notification).
- `value_precision` in exact|banded|categorical|none; `band_table` required key,
  non-empty when banded.
- `access`: `{method: "..." (non-empty), session_required: bool, captcha?: str, notes?: str}`.
- `identifiers_available`: `{politician: "...", instrument: "..."}` non-empty ("none" is
  a legal affirmative claim; empty is not).
- `personal_data_to_redact`: list (empty = affirmative "nothing found to redact").
- `open_questions`: each `{question: "...", tried: [non-empty list]}`.
- `regime_versions`: each `{effective_from, change, evidence}` with non-empty evidence
  per the claim rules.
Body must keep the template's five `##` sections (Data catalog · Field mapping ·
Parse strategy · Quirks log · Operational notes), prefix-matched.

### Capture manifest (Phase 2 gate)
Exactly one of `crates/adapters/<x>/fixtures/manifest.yaml` (`.yml`) or
`MANIFEST.json` (the us_house reference spelling). Required keys — EXTRA keys are
allowed here, the manifest doubles as the capture/provenance record:
```yaml
captured_at_utc: <RFC3339>
politeness:
  user_agent: "..."          # identified UA, invariant 10 (extra fields welcome)
cases:                       # >= 3 (typical, amendment/correction, edge case)
  <case_name>:               # bijective with fixtures/<case_name>/ directories
    url: https://...         # where the raw filing was fetched
    sha256: <64 hex>         # MUST equal sha256(fixtures/<case_name>/input.*)
```
Each case directory holds exactly one `input.*`; its hash is re-computed and compared
(raw is sacred). Undeclared fixture directories and phantom manifest entries both reject.
