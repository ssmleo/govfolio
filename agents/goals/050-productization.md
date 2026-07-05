# 050 — productization

## Objective
Auth, hashed API keys, per-tier quotas via usage_event → Stripe metered billing, free-tier 24h data delay enforced in /v1, monthly snapshot export job (CC BY).

## Context (read first)
- design §6.2, §6.4 (delay is THE monetization lever — enforce in one query-layer place)

## Acceptance criteria
```bash
cargo test -p api tiers   # free sees nothing < 24h old; pro sees realtime; quotas metered
```

## Checklist
- [x] auth  - [x] keys  - [x] delay  - [x] metering  - [x] stripe  - [x] snapshot job

## Notes (2026-07-05, rust-builder)
- Delay column choice: `filing.discovered_at` (our knowledge time — design §4.2 "our
  latency, honestly"); `published_at` is the government's clock and often absent. The
  bound is slot `$11` INSIDE `core::query::RecordFilter::SQL_WHERE` (serde-skipped,
  never contract surface), so every composed record query enforces it; the api stamps
  it in exactly one place (`api::auth::AuthContext::apply`).
- Real signup DEFERRED: accounts bootstrap via `POST /v1/users` behind `X-Admin-Token`
  (env `ADMIN_TOKEN`; unset = surface disabled, fail closed).
- "Everywhere records flow" also closed two back doors: review surface is now
  admin-token-gated (real-time record context) and alert-rules require a pro/data key
  (alerts are the paid fast path). FOLLOW-UP: apps/web reviewer UI must forward the
  admin token (one server-side header) — web untouched per goal scope, so reviewer
  e2e is red until then.
- Anonymous traffic: per-IP per-minute in-memory backstop only (default 600/min;
  `UNAUTH_REQUESTS_PER_MINUTE`). Authoritative anonymous limits belong to the CDN
  edge (design §6.4); the SSR origin shares one IP, hence the generous ceiling.
- Stripe: no credentials on hosts. Webhook = `core::stripe` HMAC verify (canned signed
  payloads in tests); outbound = `worker::stripe::StripeClient` trait, live impl
  config-gated on `STRIPE_SECRET_KEY`, metering via billing meter events with
  usage_report-ULID identifiers (exactly-once). Tier grants travel as subscription
  metadata `govfolio_tier`; lapse downgrades to free + deactivates alert rules.
- Snapshot exports through the SAME evaluator with the 24h bound (free-tier artifact
  must not tunnel under the delay).
