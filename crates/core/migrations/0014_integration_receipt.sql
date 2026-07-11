-- 0014_integration_receipt: Release-1 producer receipts and the CAS lifecycle
-- projection. Expand-only: existing registry rows receive generation zero and no
-- pending receipt; every new ledger table is additive.

alter table jurisdiction
  add column lease_generation bigint not null default 0
    check (lease_generation >= 0),
  add column pending_integration_id text;

create table integration_receipt (
  id                       text primary key,
  work_key                 text not null,
  jurisdiction_id          text not null references jurisdiction(id),
  from_phase               text not null check (from_phase in
    ('stub','scouted','surveyed','sampled','specced','built','live','blocked')),
  to_phase                 text check (to_phase in
    ('stub','scouted','surveyed','sampled','specced','built','live','blocked')),
  blocked_reason           text,
  source_sha               text not null check (source_sha ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
  base_sha                 text not null check (base_sha ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
  source_branch            text not null check (btrim(source_branch) <> ''),
  lane_id                  text not null check (btrim(lane_id) <> ''),
  lease_generation         bigint not null check (lease_generation >= 0),
  provider                 text not null check (provider in ('claude','codex')),
  model                    text not null check (btrim(model) <> ''),
  attempt_id               text not null check (btrim(attempt_id) <> ''),
  validation_evidence      jsonb not null check (
    jsonb_typeof(validation_evidence) = 'array'
    and jsonb_array_length(validation_evidence) > 0
  ),
  artifact_hashes          jsonb not null check (jsonb_typeof(artifact_hashes) = 'array'),
  real_source_proof        jsonb,
  journal_summary          text not null check (
    btrim(journal_summary) <> '' and journal_summary !~ E'[\r\n]'
  ),
  repair_of                text references integration_receipt(id),
  repair_ordinal           smallint check (repair_ordinal between 1 and 2),
  payload_sha256           text not null check (payload_sha256 ~ '^[0-9a-f]{64}$'),
  submitted_at             timestamptz not null default now(),
  unique (id, jurisdiction_id),
  check (
    (repair_of is null and repair_ordinal is null)
    or (repair_of is not null and repair_ordinal is not null)
  ),
  check (
    (to_phase = 'blocked' and nullif(btrim(blocked_reason), '') is not null)
    or (to_phase is distinct from 'blocked' and blocked_reason is null)
  ),
  check (
    to_phase is null
    or (from_phase = 'stub' and to_phase = 'scouted')
    or (from_phase = 'scouted' and to_phase = 'surveyed')
    or (from_phase = 'surveyed' and to_phase = 'sampled')
    or (from_phase = 'sampled' and to_phase = 'specced')
    or (from_phase = 'specced' and to_phase = 'built')
    or (from_phase = 'built' and to_phase = 'live')
    or (from_phase not in ('live','blocked') and to_phase = 'blocked')
  ),
  check (
    from_phase <> 'built' or to_phase <> 'live' or real_source_proof is not null
  )
);

create unique index integration_receipt_idempotency_idx
  on integration_receipt (work_key, from_phase, coalesce(to_phase, ''), source_sha);
create index integration_receipt_jurisdiction_idx
  on integration_receipt (jurisdiction_id, submitted_at);

alter table jurisdiction
  add constraint jurisdiction_pending_integration_fk
  foreign key (pending_integration_id, id)
  references integration_receipt (id, jurisdiction_id)
  deferrable initially deferred;

create table integration_receipt_state (
  receipt_id               text primary key references integration_receipt(id),
  state                    text not null check (state in
    ('submitted','preparing','awaiting_ci','merged_unapplied','applied',
     'rework_required','deferred')),
  version                  bigint not null default 0 check (version >= 0),
  candidate_base_sha       text check (
    candidate_base_sha is null
    or candidate_base_sha ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'
  ),
  integration_branch       text,
  pr_number                bigint check (pr_number > 0),
  merge_sha                text check (
    merge_sha is null or merge_sha ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'
  ),
  last_error               text,
  updated_at               timestamptz not null default now()
);

create index integration_receipt_state_queue_idx
  on integration_receipt_state (state, updated_at, receipt_id);

create table integration_event (
  sequence                 bigint generated always as identity primary key,
  receipt_id               text not null references integration_receipt(id),
  from_state               text check (from_state in
    ('submitted','preparing','awaiting_ci','merged_unapplied','applied',
     'rework_required','deferred')),
  to_state                 text not null check (to_state in
    ('submitted','preparing','awaiting_ci','merged_unapplied','applied',
     'rework_required','deferred')),
  version                  bigint not null check (version >= 0),
  actor                    text not null check (btrim(actor) <> ''),
  evidence                 jsonb not null default '{}' check (jsonb_typeof(evidence) = 'object'),
  occurred_at              timestamptz not null default now(),
  unique (receipt_id, version)
);

create index integration_event_receipt_idx
  on integration_event (receipt_id, sequence);

create function reject_immutable_integration_row() returns trigger
language plpgsql as $$
begin
  raise exception 'immutable integration ledger rows cannot be changed'
    using errcode = '55000';
end
$$;

create trigger integration_receipt_immutable
before update or delete on integration_receipt
for each row execute function reject_immutable_integration_row();

create trigger integration_event_immutable
before update or delete on integration_event
for each row execute function reject_immutable_integration_row();

create trigger integration_receipt_state_no_delete
before delete on integration_receipt_state
for each row execute function reject_immutable_integration_row();
