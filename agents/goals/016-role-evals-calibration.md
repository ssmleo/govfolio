# 016 — role evals + E1 calibration

## Objective
Make agent quality measurable: build the eval harness that scores each role's artifact against the us_house ground truth (survey, mapping table, expected outputs) with thresholds; epochs gate on green.

## Context
- agents/roles/*.md · docs/regimes/us-house/ (reference) · agents/EPOCHS.md

## Acceptance criteria
```bash
cargo test -p pipeline role_evals   # all roles >= threshold vs us_house reference
```

## Checklist
- [x] reference bundle frozen  - [x] scorer per role  - [x] thresholds documented  - [x] epoch gate wired

## Progress (2026-07-05, rust-builder)
- [x] Reference bundle frozen: `docs/regimes/us-house/reference/E1.lock.json` — 17 sha256
  pins (regime doc, MANIFEST.json, 4 fixture dirs' input.pdf + expected.silver/gold.json,
  details schema snapshot, 2 archived evidence files). Tamper-evident: `role_evals`
  freeze test + epoch-gate re-hash every pin; supersede-don't-mutate policy in the lock.
- [x] Scorer per role: `crates/pipeline/src/evals/` — deterministic, mechanical, NO LLM.
  spec-writer 10 checks (front-matter/RegimeSurvey keys/record-type vocab/band table
  decimal strings/sections/§3.3-§3.4-§3.6 mapping tables/§7 fixture pins/§8 evidence log);
  test-designer 7 checks (validate-manifest reuse, silver wrapper shape + counts,
  gold vs GoldCandidate schema snapshot + domain validation + details contract, §7 pins);
  rust-builder 4 checks (real commands: conformance us_house 4/4, fmt --check,
  clippy -D warnings, test --workspace — nested run isolated in its own target dir);
  auditor 6 checks (JOURNAL audit line + verdict, goal-001 T8d block: PASS, independent
  re-derivation + commit-order integrity, non-blocking findings surfaced).
  scout/surveyor/sampler = NOT_APPLICABLE with explicit reasons (walking skeleton skipped
  those phases); scorers auto-convert to validator-backed scores once artifacts exist.
- [x] Thresholds documented: `docs/decisions/role-eval-thresholds.md` — 1.00 per role
  (conservative; checks are mechanical vs an audited reference), NOT_APPLICABLE = BLOCKING
  for E2 entry (fail closed), out-of-harness roles listed with rationale, founder-gated.
- [x] Epoch gate wired: `cargo test -p pipeline role_evals` (acceptance) green 11/11;
  `cargo run -p pipeline --bin epoch-gate -- E2` prints per-role scores + verdict —
  currently (correctly) E2 BLOCKED on missing scout/surveyor/sampler references, exit 1.
