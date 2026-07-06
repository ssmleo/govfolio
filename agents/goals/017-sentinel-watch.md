# 017 — sentinel WATCH

## Objective
Weekly scheduled job per live source: HTTP status, listing layout-hash delta, filing-count delta, regime-change probe; anomalies file ranked drift goals automatically.

## Acceptance criteria
```bash
cargo test -p worker sentinel   # incl. dedup + ranking of drift reports
```

## Checklist
- [x] checks  - [x] scheduler wiring (goal 020 infra)  - [x] goal-filing  - [x] ranking/dedup

Done (rust-builder, goal 017):
- CHECKS: `crates/worker/src/sentinel.rs` — per-source WATCH over 8 live regimes
  (`live_targets()`): (a) HTTP status via polite conditional GET (`SourceProbe`
  seam, `HttpProbe` wraps `PoliteClient` — invariant 10); (b) structural
  layout-hash delta (`layout_hash` = sha256 of a count-invariant tag/class token
  SET, so row growth ≠ layout shift); (c) filing-count delta (row_marker count);
  (d) regime-markers probe. Every check fail-closed: transport error →
  `probe_error`, non-2xx → `status_error`, markers gone → `regime_change`.
- GOAL-FILING: anomalies auto-file ranked `drift_report` rows (migration 0008);
  `freezes_publication` kinds (layout_shift, count_zero, regime_change) set
  `sentinel_watch.frozen` + open a `review_task` (design §5.6). No human gate.
- DEDUP: `dedup_key = regime:kind:signature` + partial-unique index over open
  reports; re-detection bumps `detections`, never multiplies. Baseline holds
  last-known-GOOD while drifted so the same drift re-detects and dedups.
- RANKING: `priority_score` layout_shift 100 > count_zero 90 > regime_change 80
  > status_error 70 > probe_error 60 > count_delta 30; `rank()` sorts severity
  desc then (regime, dedup_key) — deterministic, tested.
- SCHEDULER: `crates/worker/src/bin/sentinel.rs` (`--once` default / `--loop`);
  weekly Cloud Scheduler stub (PAUSED, fail-closed) in `infra/scheduler.tf`.
- Migration 0008 (expand-only, safety-gate PASS); migrate pin 8→9.
- Verify: `cargo test -p worker sentinel` 16 pass + 1 db (`--ignored`) green;
  full workspace + ignored suites green; clippy `-D warnings` clean; fmt clean;
  us_house + eu_fr_de_annual conformance green.
