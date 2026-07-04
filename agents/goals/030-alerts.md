# 030 — alerts

## Objective
Outbox dispatcher worker: match outbox_event against alert_rule (filter grammar == /records grammar), fan out email + HMAC-signed webhooks with idempotent dedup keys, retries, DLQ, digest mode.

## Context (read first)
- design §6.3 · packages/core query grammar · plan Task 9 outbox writes

## Acceptance criteria
```bash
pnpm --filter worker test -- alerts   # incl. exactly-once delivery under redelivery
```

## Checklist
- [ ] rules CRUD  - [ ] matcher  - [ ] email sender  - [ ] webhook signer  - [ ] DLQ  - [ ] digest
