# 082 — local sampled real-write backfill (verification runs)

## Objective
Let the existing us_house real-write backfill run as a BOUNDED per-year sample against a
LOCAL database, so the founder can verify original documents next to extracted rows
(Bronze PDF ↔ Silver/Gold rows ↔ reviewer UI) on ~30 real filings across 2012–2026
without touching prod and without processing whole years.

## Scope
- Additive `--limit <N>` flag on `crates/worker/src/bin/backfill-real.rs`: after
  discovery, truncate each year's `FilingRef` list to N (print `sampled N of M`).
- Gate semantics under `--limit`: the full-year `gate_year` dry-run (which fetches the
  whole year) is replaced by the mechanical upper bound `min(discovered, N)` compared to
  `BACKFILL_BUDGET` — every sampled filing counted as a worst-case add; rationale printed.
  Without `--limit`, behavior is byte-identical to today (full-year gate).
- NO change to the dry-run bin, the Runner, adapters, or prod posture: the goal-080/081
  prod HALTs stand; this goal's runs target `DATABASE_URL` = local Postgres (5433) and
  the durable local Bronze root `target/bronze-backfill-real`.
- Operational half: seed historical rosters locally, run `--from 2012 --to 2026 --limit 2`
  (≤30 filings; scanned docs take the live LLM seam ≈ $0.30 sync — within the founder's
  2026-07-08 USD 200/month HARD CAP), then verify side-by-side.

## Context (read first)
/CLAUDE.md invariants · `crates/worker/src/bin/backfill-real.rs` (goal 081 Tasks 2–4
machinery being extended additively; goal 081's remaining prod tasks are untouched) ·
`crates/worker/src/backfill.rs` (`gate_year`, `budget_verdict`, dry-run sampler) ·
docs/runbooks/dev-host-windows.md (local pg 5433).

## Acceptance criteria (all must pass)
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
# sampled run (local DB + key in env):
cargo run -p worker --bin backfill-real -- --from 2012 --to 2026 --limit 2
# then: raw_document rows exist for sampled shas; stg_us_house + gold records for parsed
# docs; review_task rows for fail-closed docs; every gold record's raw sha resolves to a
# PDF under target/bronze-backfill-real; rerun inserts nothing new (invariant 4 replay).
```

## Checklist
- [ ] `--limit` flag + sampled-gate bound implemented (additive; no-`--limit` path unchanged)
- [ ] workspace green (fmt/clippy/test)
- [ ] historical rosters seeded locally; sampled run executed against local pg
- [ ] side-by-side verification delivered (original PDF ↔ extracted rows); rerun replay proven
