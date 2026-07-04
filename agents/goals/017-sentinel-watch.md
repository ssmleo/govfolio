# 017 — sentinel WATCH

## Objective
Weekly scheduled job per live source: HTTP status, listing layout-hash delta, filing-count delta, regime-change probe; anomalies file ranked drift goals automatically.

## Acceptance criteria
```bash
cargo test -p worker sentinel   # incl. dedup + ranking of drift reports
```

## Checklist
- [ ] checks  - [ ] scheduler wiring (goal 020 infra)  - [ ] goal-filing  - [ ] ranking/dedup
