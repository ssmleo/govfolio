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
- [ ] reference bundle frozen  - [ ] scorer per role  - [ ] thresholds documented  - [ ] epoch gate wired
