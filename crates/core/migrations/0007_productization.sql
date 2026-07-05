-- 0007_productization: accounts, hashed API keys, usage metering and the
-- minimal Stripe billing mirror (design §4.2 supporting shapes, §6.2, §6.4;
-- goal 050). Expand-only.
--
-- Secrets discipline: api_key stores ONLY the SHA-256 hex of the presented
-- token (`gfk_<random>`); plaintext exists exactly once, in the creation
-- response. Revocation is a timestamp so the ledger keeps history
-- (supersede-never-delete spirit).
--
-- usage_event is the single metering source (design §6.4: Postgres-backed
-- counters at launch; Redis is a documented, unbuilt upgrade path): one row
-- per authenticated request, counted for the daily quota and aggregated by
-- the billing-sync worker into Stripe usage reports. usage_report makes that
-- aggregation exactly-once: events are stamped with their report in the same
-- transaction that creates it, and the report id doubles as the Stripe
-- idempotency key (crash between stamp and send = resend same id, no double
-- billing).

create table user_account (
  id                 text primary key,
  email              text not null unique,
  tier               text not null default 'free'
                       check (tier in ('free','pro','data')),
  stripe_customer_id text unique,
  created_at         timestamptz not null default now()
);

create table api_key (
  id         text primary key,
  user_id    text not null references user_account(id),
  key_hash   text not null unique,      -- sha256 hex; never the plaintext
  label      text not null,
  created_at timestamptz not null default now(),
  revoked_at timestamptz                -- non-null = revoked (immediate)
);

create index api_key_user on api_key (user_id);

create table usage_report (
  id           text primary key,        -- ULID; Stripe idempotency key
  user_id      text not null references user_account(id),
  period_start timestamptz not null,
  period_end   timestamptz not null,
  quantity     bigint not null check (quantity > 0),
  reported_at  timestamptz,             -- null = not yet accepted by Stripe
  created_at   timestamptz not null default now()
);

create index usage_report_unreported on usage_report (id) where reported_at is null;

create table usage_event (
  id          text primary key,
  user_id     text not null references user_account(id),
  api_key_id  text references api_key(id),
  endpoint    text not null,
  occurred_at timestamptz not null default now(),
  report_id   text references usage_report(id)  -- null = not yet billed
);

-- Quota counting (user, billing period) and the billing-sync unbilled scan.
create index usage_event_user_period on usage_event (user_id, occurred_at);
create index usage_event_unbilled on usage_event (user_id, occurred_at)
  where report_id is null;

-- Minimal Stripe mirror: enough to know who pays and until when. Stripe is
-- the source of truth; rows are upserted from verified webhook events only.
create table subscription (
  id                     text primary key,
  user_id                text not null references user_account(id),
  stripe_subscription_id text not null unique,
  status                 text not null,
  current_period_end     timestamptz,
  created_at             timestamptz not null default now(),
  updated_at             timestamptz not null default now()
);

create index subscription_user on subscription (user_id);
