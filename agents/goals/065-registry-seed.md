# 065 — worldwide registry seed + coverage dashboard

## Objective
Seed jurisdiction table with all countries + disclosure_regime stubs (regime_type, precision, cadence, source_url or 'none') from public sources; expose /v1/jurisdictions coverage fields; research-per-country is a repeatable sub-goal.

## Acceptance criteria
```bash
cargo test -p core registry   # >=190 jurisdictions, every one has a regime row (possibly type=none)
```

## Checklist
- [x] seed script  - [x] sources cited in details  - [x] endpoint fields  - [x] research template

Done (rust-builder, goal 065):
- SEED: `crates/core/src/seed/` — `iso.rs` (195 sovereigns = 193 UN members + 2
  observer states, canonical ISO 3166-1 alpha-2 static data) + `mod.rs`
  (`LIVE_REGIMES` = the 8 built launch regimes reusing the adapters' pinned
  fixture regime ids; `seed_registry()` idempotent `ON CONFLICT DO NOTHING`).
  196 jurisdictions (195 national + EU supranational); every jurisdiction ≥1
  regime; 8 real + 189 `type='none'` stubs (197 regimes). 7 live jurisdictions (us,gb,ca,au,
  eu,fr,de — matches sentinel `live_targets()`; us carries House+Senate),
  epoch 1; Brazil epoch 2 (EPOCHS.md E2); rest stub.
- APPROACH: seed fn run by the migrate bin (`cargo run -p core --bin migrate`
  applies schema then seeds) — NOT a new DDL migration, so migrate pin (n=9)
  and the expand-only guardrail stay green. Rust static data = source of truth.
- SOURCES: real regimes cite regime doc + source_url in `details`; stubs cite
  ISO-3166-1 basis + "regime unresearched — coverage factory".
- ENDPOINT: `/v1/jurisdictions` gains `coverage_phase`, `epoch`,
  `priority_score` (the coverage dashboard / §6.1 scorecard). openapi + TS
  client regenerated + committed; drift gate green.
- TEMPLATE: `agents/workflows/research-regime-template.md` (lean pointer to
  source-exploration.md phases + the 015 validators).
- VERIFY: `cargo test -p core registry` (2 non-DB + 3 `--ignored` DB) green;
  workspace + core/api `--ignored` green; clippy `-D warnings` + fmt clean;
  check-migration-safety green; conformance us_house 5/5, eu_fr_de 9/9,
  canada 7/7; web lint/typecheck/test green.
