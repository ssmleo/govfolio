# 030 — alerts

## Objective
Outbox dispatcher worker: match outbox_event against alert_rule (filter grammar == /records grammar), fan out email + HMAC-signed webhooks with idempotent dedup keys, retries, DLQ, digest mode.

## Context (read first)
- design §6.3 · crates/core query grammar (single impl shared by /records and alert matching) · plan Task 9 outbox writes

## Acceptance criteria
```bash
cargo test -p worker alerts   # incl. exactly-once delivery under redelivery
```

## Checklist
- [x] rules CRUD  - [x] matcher  - [x] email sender  - [x] webhook signer  - [x] DLQ  - [x] digest

## Notes (2026-07-05, rust-builder)
- ONE grammar: `core::query::RecordFilter` — serde form = `alert_rule.filter` contract
  (snapshot `crates/core/schemas/record_filter.json`), `SQL_WHERE` const = the single
  evaluator behind BOTH `/v1/records` and the matcher (slots $1..$10; callers bind $11+).
- Exactly-once: dedup_key `(rule, event, channel)` unique + ON CONFLICT DO NOTHING;
  `dispatched_at` stamped in the matcher txn. Proven under redelivery in
  `worker/tests/alerts.rs::alerts_exactly_once_delivery_under_redelivery`.
- DLQ = `delivery.status='dead'` (+ partial index); retry budget `max_attempts=5`,
  exponential backoff, terminal errors dead-letter immediately.
- Channels: at most one per type per rule (dedup key is per type; validated at the API).
- AUTH: none yet on /v1/alert-rules — accounts/tiers are goal 050 (noted in code + contract).
- Email real impl = lettre STARTTLS behind SMTP_* config gate; unconfigured hosts
  dead-letter email deliveries loudly (never fake-send). DELETE /alert-rules cascades
  the rule's delivery ledger rows.
