-- 0002_silver_us_house: Silver staging for the us_house adapter (design §4.2
-- supporting-tables note: "one stg_<regime> table per adapter, source-shaped,
-- plus stg_meta (run linkage)"). Expand-only.
--
-- stg_us_house mirrors the regime doc §4 StagingRow field for field: verbatim
-- source strings (dates stay text exactly as printed — Silver keeps the
-- source's vocabulary; typing happens at normalize). NULLability mirrors the
-- §4 "string|null" columns. `confidence` is the pipeline StagingRow wrapper
-- field; `extractor` identifies the parser version (reprocessing key).

create table stg_us_house (
  id                     text primary key,
  raw_document_id        text not null references raw_document(id),
  row_ordinal            int  not null check (row_ordinal >= 1),
  doc_id                 text not null,
  filer_name_raw         text not null,
  filer_status_raw       text not null,
  state_district_raw     text not null,
  row_id_raw             text,
  owner_code_raw         text,
  asset_raw              text not null,
  asset_type_code_raw    text,
  transaction_type_raw   text not null,
  transaction_date_raw   text not null,
  notification_date_raw  text not null,
  amount_raw             text not null,
  cap_gains_over_200     boolean,
  filing_status_raw      text not null,
  subholding_of_raw      text,
  description_raw        text,
  comments_raw           text,
  vehicle_owner_code_raw text,
  vehicle_location_raw   text,
  signed_date_raw        text not null,
  extractor              text not null,
  confidence             real not null,
  created_at             timestamptz not null default now(),
  -- Idempotent staging (invariant 4): one row per (document, ordinal).
  unique (raw_document_id, row_ordinal)
);

-- Run linkage: which pipeline_run staged which Silver row. First writer wins
-- (ON CONFLICT DO NOTHING); replays add nothing.
create table stg_meta (
  stg_table       text not null,
  stg_id          text not null,
  raw_document_id text not null references raw_document(id),
  pipeline_run_id text not null references pipeline_run(id),
  created_at      timestamptz not null default now(),
  primary key (stg_table, stg_id)
);
