-- 0010_silver_br: Silver staging for the br adapter (design §4.2 supporting-
-- tables note: "one stg_<regime> table per adapter, source-shaped, plus
-- stg_meta (run linkage)"). Expand-only. Mirrors the 0002_silver_us_house
-- convention: `id text primary key`, `extractor text not null`, `confidence
-- real not null`, `unique (raw_document_id, row_ordinal)`.
--
-- stg_br mirrors crates/adapters/br/src/parse.rs's SilverRow field for field:
-- verbatim source strings (dates/values stay text exactly as printed — Silver
-- keeps the source's own vocabulary; typing happens at normalize). Every
-- SilverRow field is required (`not null`) except the two PII passthrough
-- columns, which are `Option<String>` in Rust (production-only, gated on
-- `ctx.pool.is_some()` — see parse.rs's own doc comment) and therefore
-- nullable here.
--
-- `nm_candidato`/`sg_uf` (added alongside this migration, rust-builder
-- goal-081-follow-on "RunnerBinding for br"): PUBLIC disclosure content, not
-- PII — candidate identity/state, needed by `RunnerBinding::filing_identity()`
-- for politician-roster resolution (design §5.4), same as `us_house`'s
-- `filer_name_raw`/`state_district_raw`. See docs/regimes/br/AUTHORITY.md
-- Quirks log for the write-back explaining this SilverRow revision.

create table stg_br (
  id                             text primary key,
  raw_document_id                text not null references raw_document(id),
  row_ordinal                    int  not null check (row_ordinal >= 1),
  sq_candidato                   text not null,
  nm_candidato                   text not null,
  sg_uf                          text not null,
  dt_eleicao_raw                 text not null,
  election_year_raw              text not null,
  line_item_ordinal_raw          text not null,
  asset_type_code_raw            text not null,
  asset_type_label_raw           text not null,
  asset_description_raw          text not null,
  value_raw                      text not null,
  last_updated_date_raw          text not null,
  last_updated_time_raw          text not null,
  extractor                      text not null,
  nr_titulo_eleitoral_candidato  text,
  nr_cpf_candidato               text,
  confidence                     real not null,
  created_at                     timestamptz not null default now(),
  -- Idempotent staging (invariant 4): one row per (document, ordinal).
  unique (raw_document_id, row_ordinal)
);
