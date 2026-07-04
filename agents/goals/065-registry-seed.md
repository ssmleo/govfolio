# 065 — worldwide registry seed + coverage dashboard

## Objective
Seed jurisdiction table with all countries + disclosure_regime stubs (regime_type, precision, cadence, source_url or 'none') from public sources; expose /v1/jurisdictions coverage fields; research-per-country is a repeatable sub-goal.

## Acceptance criteria
```bash
cargo test -p core registry   # >=190 jurisdictions, every one has a regime row (possibly type=none)
```

## Checklist
- [ ] seed script  - [ ] sources cited in details  - [ ] endpoint fields  - [ ] research template
