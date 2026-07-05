# Role-eval thresholds & epoch-gate semantics (goal 016)

Status: ACTIVE, founder-gated — changes to thresholds, to the scored-role set, or to
the NOT_APPLICABLE gating rule below are governance changes and require founder
approval, exactly like edits to `agents/roles/*.md` (automation-policy discipline).

## What the harness is

`crates/pipeline/src/evals/` scores each coverage-factory role's E1 artifact against
the frozen us_house reference bundle (`docs/regimes/us-house/reference/E1.lock.json`,
sha256 pins over 17 ground-truth files). Every scorer is deterministic and MECHANICAL
— filesystem reads, hash comparison, JSON-Schema validation, real command invocation.
**No LLM-judge anywhere** (world-verifies-model; model-verifies-model never gates).

- Acceptance: `cargo test -p pipeline role_evals`
- Orchestrator verdict: `cargo run -p pipeline --bin epoch-gate -- E2`
  (exit 0 = open, nonzero = blocked; per-role scores printed)

Freeze discipline: the lock is tamper-evident, supersede-never-mutate. Amending any
pinned artifact requires superseding the lock (version bump + note), founder-gated.
A scorer that finds a defect in a reference artifact surfaces a FINDING; it never
edits the artifact.

## Per-role thresholds

Score = passed mechanical checks / total checks, in [0, 1]. Conservative default:
**1.00 for every scored role** — the checks run against a known-good, audited
reference corpus (goal 001 T8d PASS), so anything below full marks is drift, tamper,
or a defect, and the gate must fail closed. Thresholds below 1.00 would only be
justified for checks with known environmental flakiness; none exist today.

| Role | Threshold | What is scored (all mechanical) |
|---|---|---|
| scout | 1.00 | `docs/regimes/us_house/sources.yaml` validates (`validate_sources`) — NOT_APPLICABLE today, see below |
| surveyor | 1.00 | `docs/regimes/us_house/AUTHORITY.md` validates (`validate_survey`) — NOT_APPLICABLE today, see below |
| sampler | 1.00 | sampler-attributed capture manifest validates (`validate_manifest`) — NOT_APPLICABLE today, see below |
| spec-writer | 1.00 | regime-doc structural completeness: front-matter parses with all 18 RegimeSurvey keys, record types in vocabulary, band table with decimal-string bounds, all reference sections present, §3.3/§3.4/§3.6 mapping tables parse with required tokens, every fixture input hashes to a §7 pin, §8 evidence log populated (rows/URLs/sha pins) |
| test-designer | 1.00 | `validate_manifest` clean; every expected.silver.json is a non-empty `{payload, confidence}` wrapper array with counts matching the manifest; every expected.gold.json candidate is schema-valid vs the committed GoldCandidate snapshot, deserializes + passes domain validation, and satisfies the (us_house, transaction) details contract; manifest sha pins appear in regime doc §7 |
| rust-builder | 1.00 | the real commands, invoked: `conformance -- us_house` prints 4/4 green, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` all exit 0 |
| auditor | 1.00 | audit journal line (`AUDIT … T8d`) exists with an explicit verdict; goal-001 T8d findings block exists, records PASS, records independent re-derivation + fixture-commit-order integrity, and surfaces non-blocking findings |

## NOT_APPLICABLE and what it means for epoch gating

`NOT_APPLICABLE` = the role has **no reference artifact** to score (the walking
skeleton skipped that phase). It is *not* a pass and *not* an exemption:

> **The epoch gate treats every NOT_APPLICABLE role as BLOCKING for E2 entry**
> until its reference artifact exists and scores at threshold. Fail closed: E2
> (Brazil) needs the scout/survey/sample phases live, so gating on their absence
> is correct — an epoch must not open on unmeasured roles.

Currently NOT_APPLICABLE, with the artifact that unblocks each:

- **scout** — needs `docs/regimes/us_house/sources.yaml` (E1 source was
  founder-designated; no SCOUT run ever produced the artifact). Any validated
  E1-scope scout artifact converts the score to `validate_sources` output.
- **surveyor** — needs `docs/regimes/us_house/AUTHORITY.md`. The E1 survey
  knowledge lives in `docs/regimes/us-house.md`, which PRE-DATES the goal-015
  AUTHORITY.md convention and its `{url, file}` evidence schema; the scorer will
  not grandfather it (fail closed).
- **sampler** — fixtures exist and validate, but `MANIFEST.json.captured_by`
  attributes the capture to test-designer (goal 001 T8b): there is no
  sampler-produced artifact. A sampler-attributed, validating capture manifest
  converts the score.

## Roles outside the harness

orchestrator, planner, sentinel, web-builder produce no us_house-reference-scoped
artifact in E1, so the calibration harness has nothing mechanical to score them
against; they are OUT OF SCOPE here and do not appear in the gate. Extending the
harness to them requires (a) a reference artifact for each and (b) a founder-gated
amendment of this document. They remain governed by their role files and the
orchestrator's verification duties in the meantime.

## Epoch coverage

Only the **E2** gate is wired (calibrated against the frozen E1 reference). Later
epochs (E3+) need their own reference bundles frozen from the preceding epoch's
audited corpus; the gate rejects any other epoch id (fail closed).
