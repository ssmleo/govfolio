# 080 — US backfill + launch

## Objective
Run the same pipeline over US archives back to 2012; human-gated diff review; complete docs/runbooks/launch-checklist.md (SLOs live, legal pages, budget alerts, status page).

## Acceptance criteria
```bash
cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --dry-run   # human reviews diff, then real run
```

## Checklist
- [ ] dry-run diff  - [ ] real run  - [ ] SLO dashboards  - [ ] legal pages  - [ ] checklist done

## BLOCKED (human)
- diff approval; launch go/no-go
