# 050 — productization

## Objective
Auth, hashed API keys, per-tier quotas via usage_event → Stripe metered billing, free-tier 24h data delay enforced in /v1, monthly snapshot export job (CC BY).

## Context (read first)
- design §6.2, §6.4 (delay is THE monetization lever — enforce in one query-layer place)

## Acceptance criteria
```bash
pnpm --filter api test -- tiers   # free sees nothing < 24h old; pro sees realtime; quotas metered
```

## Checklist
- [ ] auth  - [ ] keys  - [ ] delay  - [ ] metering  - [ ] stripe  - [ ] snapshot job
