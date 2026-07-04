# 001 — walking skeleton (M0–M1)

## Objective
Execute Tasks 1–11 of docs/plans/2026-07-04-govfolio-implementation.md exactly as written.

## Acceptance criteria
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
cargo run -p pipeline --bin conformance -- us_house
docker compose up -d && cargo test --workspace -- --ignored
```

## Checklist
- [x] T1 workspace  - [x] T2 CI  - [ ] T3 migrations  - [ ] T4 domain  - [ ] T5 DDL  - [ ] T6 fingerprint  - [ ] T7 conformance  - [ ] T8 us_house  - [ ] T9 pipeline  - [ ] T10 /v1  - [ ] T11 promote

## BLOCKED (human)
- fixture expected.*.json completion is human ground truth (plan Task 8)
