-- 0005_alerts: alert rules + the transactional-outbox delivery ledger
-- (design §6.3, shapes fixed in §4.2; goal 030). Expand-only.
--
-- user_id is free text until accounts land (goal 050); tier enforcement
-- (alerts are Pro/instant, design §6.2) is also goal 050 — schema leaves room.
-- filter speaks the ONE record filter grammar (core::query::RecordFilter,
-- snapshot crates/core/schemas/record_filter.json) — same grammar as
-- /v1/records; validated strictly at the API door.
-- channels is a list of core::alerts::AlertChannel (at most one per type;
-- the delivery dedup key is per channel type).

create table alert_rule (
  id         text primary key,
  user_id    text not null check (length(user_id) > 0),
  filter     jsonb not null default '{}'::jsonb,
  channels   jsonb not null default '[]'::jsonb,
  digest     boolean not null default false,
  active     boolean not null default true,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index alert_rule_active on alert_rule (id) where active;

-- Exactly-once fan-out ledger: dedup_key = '<rule_id>:<outbox_event_id>:<channel>'
-- is deterministic, so at-least-once redelivery inserts ON CONFLICT DO NOTHING
-- (invariant 4). The DLQ is rows with status 'dead' — no external queue infra
-- at this volume (design §6.3).
create table delivery (
  id              text primary key,
  alert_rule_id   text not null references alert_rule(id) on delete cascade,
  outbox_event_id text not null references outbox_event(id),
  channel         text not null check (channel in ('email','webhook')),
  dedup_key       text not null unique,
  status          text not null default 'pending' check (status in
                    ('pending','pending_digest','sent','dead')),
  attempts        int not null default 0 check (attempts >= 0),
  last_error      text,
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now()
);

create index delivery_undelivered on delivery (status, id)
  where status in ('pending','pending_digest');
create index delivery_dlq on delivery (id) where status = 'dead';
create index delivery_rule on delivery (alert_rule_id);

-- The dispatcher polls undispatched events oldest-first.
create index outbox_undispatched on outbox_event (id) where dispatched_at is null;
