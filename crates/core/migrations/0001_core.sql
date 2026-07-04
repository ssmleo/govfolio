-- 0001_core: canonical Gold layer, copied verbatim from design §4.2, plus full DDL
-- for the three supporting tables whose shapes §4.2 fixes (review_task, pipeline_run,
-- outbox_event). Remaining §4.2 supporting tables land with the features that need them.

-- ULIDs stored as text: time-sortable, URL-safe, no coordination.

create table jurisdiction (
  id          text primary key,
  name        text not null,
  iso_code    text,                      -- ISO 3166-1 alpha-2 where applicable
  level       text not null check (level in ('supranational','national','subnational')),
  parent_id   text references jurisdiction(id)
);

create table disclosure_regime (
  id                  text primary key,
  jurisdiction_id     text not null references jurisdiction(id),
  body                text not null,     -- 'US House', 'US Senate', 'Bundestag', ...
  regime_type         text not null check (regime_type in
                        ('transaction_report','periodic_declaration',
                         'change_notification','none')),
  value_precision     text not null check (value_precision in
                        ('exact','banded','categorical','none')),
  cadence             text,
  disclosure_lag_days int,
  source_url          text,
  details             jsonb not null default '{}',
  effective_from      date not null,
  effective_to        date,
  unique (jurisdiction_id, body, effective_from)
);

create table politician (
  id             text primary key,
  canonical_name text not null,
  wikidata_qid   text unique,
  details        jsonb not null default '{}'
);

create table politician_alias (
  politician_id text not null references politician(id),
  alias         text not null,
  lang          text,
  primary key (politician_id, alias)
);

create table mandate (
  id              text primary key,
  politician_id   text not null references politician(id),
  jurisdiction_id text not null references jurisdiction(id),
  body            text not null,
  role            text not null,
  party           text,
  district        text,
  start_date      date not null,
  end_date        date
);

create table instrument (
  id          text primary key,
  name        text not null,
  ticker      text,
  isin        text,
  figi        text,
  asset_class text not null,
  country     text,
  details     jsonb not null default '{}'
);

create table instrument_alias (
  instrument_id text not null references instrument(id),
  alias         text not null,
  source        text,
  primary key (instrument_id, alias)
);

create table raw_document (
  id           text primary key,
  storage_uri  text not null,
  sha256       text not null unique,     -- dedup + integrity + reprocess cache key
  mime_type    text not null,
  source_url   text,
  fetched_at   timestamptz not null,
  fetch_run_id text
);

create table filing (
  id                   text primary key,
  regime_id            text not null references disclosure_regime(id),
  politician_id        text not null references politician(id),
  raw_document_id      text not null references raw_document(id),
  external_id          text,             -- source-native id when the source has one
  filing_type          text not null,
  filed_date           date,
  published_at         timestamptz,      -- when the government made it public
  discovered_at        timestamptz not null,  -- when we found it (our latency, honestly)
  supersedes_filing_id text references filing(id),
  details              jsonb not null default '{}',
  unique (regime_id, external_id)
);

create table disclosure_record (
  id                    text primary key,
  filing_id             text not null references filing(id),
  politician_id         text not null references politician(id),  -- denorm (hot path)
  regime_id             text not null references disclosure_regime(id), -- denorm
  instrument_id         text references instrument(id),           -- nullable: never guess
  asset_description_raw text not null,                            -- as filed, always kept
  record_type           text not null check (record_type in
                          ('transaction','holding','interest','change_notification')),
  asset_class           text not null,   -- equity|bond|fund|option|crypto|commodity|
                                         -- real_estate|private|other
  side                  text check (side in ('buy','sell','exchange')),
  transaction_date      date,
  as_of_date            date,
  notified_date         date,
  event_date            date generated always as
                          (coalesce(transaction_date, notified_date, as_of_date)) stored,
  value_low             numeric(16,2),
  value_high            numeric(16,2),   -- NULL = open-ended (e.g. UK "> threshold")
  currency              char(3),
  owner                 text check (owner in
                          ('self','spouse','dependent','joint','unknown')),
  verification_state    text not null default 'unverified' check (verification_state in
                          ('unverified','verified','corrected','disputed')),
  extraction_confidence real,
  extracted_by          text not null,   -- parser id / model+prompt version
  fingerprint           text not null unique,  -- hash(filing_id, ordinal, content) → idempotency
  supersedes_record_id  text references disclosure_record(id),
  details               jsonb not null default '{}',  -- validated by (regime, type) JSON Schema
  created_at            timestamptz not null default now(),
  -- per-type integrity inside one table:
  check (record_type <> 'transaction' or (side is not null and transaction_date is not null)),
  check (record_type <> 'holding' or as_of_date is not null),
  check (value_low is null or value_high is null or value_high >= value_low)
);

create index dr_politician_time on disclosure_record (politician_id, event_date desc);
create index dr_instrument_time on disclosure_record (instrument_id, event_date desc)
  where instrument_id is not null;
create index dr_type_time       on disclosure_record (record_type, event_date desc);
create index dr_review          on disclosure_record (verification_state)
  where verification_state = 'unverified';
-- GIN on details added per-need, not by default.

-- Supporting tables (shapes fixed in §4.2; full DDL written here).

create table review_task (
  id             text primary key,
  target_kind    text not null,          -- 'disclosure_record', 'filing', 'instrument_match', ...
  target_id      text not null,
  reason         text not null,
  priority_score real not null default 0,
  status         text not null default 'open' check (status in ('open','resolved','dismissed')),
  assignee       text,
  resolution     jsonb,                  -- verdict payload (e.g. verify / edit + corrected fields)
  created_at     timestamptz not null default now(),
  resolved_at    timestamptz
);

create table pipeline_run (
  id              text primary key,
  adapter         text not null,
  stage           text not null,
  idempotency_key text not null unique,
  status          text not null default 'running' check (status in ('running','succeeded','failed')),
  stats           jsonb not null default '{}',
  started_at      timestamptz not null default now(),
  finished_at     timestamptz,
  error           text
);

create table outbox_event (
  id            text primary key,
  kind          text not null,
  payload       jsonb not null,
  created_at    timestamptz not null default now(),
  dispatched_at timestamptz              -- NULL = pending dispatch
);
