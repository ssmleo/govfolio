-- 0011_backfill_run: per-year backfill/seed run bookkeeping (admin observability
-- plan §Architecture 1; dashboard sections B1 backfill runs, B2 historical
-- completion, D4 idempotency evidence). Expand-only: one additive table, no
-- rewrite.
--
-- OPERATIONAL bookkeeping, like `pipeline_run` — NOT a Gold fact. Rows describe
-- what the worker bins did (which years ran, what the budget gate decided, what
-- the counters said), not what a politician disclosed; the supersede-never-update
-- invariant does not apply here. Keyed by adapter `regime_code` (operational
-- vocabulary, same as `sentinel_watch`/`drift_report`), not the Gold regime FK.
--
-- Writers: `bin/backfill-real.rs`, `bin/backfill-real-br.rs` (kind='backfill'),
-- `bin/seed-historical-rosters.rs`, `bin/seed-br-candidates.rs` (kind='seed') —
-- one INSERT per processed year at completion. This is the documented exception
-- to "admin endpoints are read-only": the worker INSERTs here; the admin API
-- only SELECTs. History from before this migration stays log-only (stated on
-- the /admin/backfill page — no backdated rows are fabricated).

create table backfill_run (
  id             text primary key,            -- ULID (time-sortable, matches pipeline_run)
  regime_code    text not null,               -- adapter regime code, e.g. 'us_house', 'br'
  year           int  not null,               -- archive/election year the run covered
  kind           text not null check (kind in ('backfill','seed')),
  bin            text not null,               -- worker bin that wrote the row, e.g. 'backfill-real-br'
  scope          text,                        -- optional scope note, e.g. '--uf SP' or 'nationwide'
  status         text not null
                   check (status in ('succeeded','skipped_budget','failed')),
  filings        bigint not null default 0,   -- filings seen for the year
  published      bigint not null default 0,   -- filings published (adds + supersessions + changes)
  replayed       bigint not null default 0,   -- already-published filings left untouched (invariant 4 evidence)
  gold_inserted  bigint not null default 0,   -- Gold rows actually inserted
  outbox_written bigint not null default 0,   -- outbox_event rows written
  review_tasks   bigint not null default 0,   -- review_task rows opened
  failed_count   bigint not null default 0,   -- per-filing failures (year continued)
  record_delta   bigint not null default 0,   -- dry-run Gold-row delta the budget gate compared
  budget         bigint,                      -- BACKFILL_BUDGET in force; NULL when no budget gate applied
  error          text,                        -- year-level error, when status='failed'
  details        jsonb not null default '{}', -- bin-specific extras (summary counts, flags)
  started_at     timestamptz not null,        -- when the year's processing began
  finished_at    timestamptz not null default now()
);

-- Per-regime history: "show me br 2018's runs, newest first" (B1/B2).
create index backfill_run_regime_year on backfill_run (regime_code, year, finished_at desc);
-- Global recency: "latest runs across everything" (overview strip).
create index backfill_run_finished on backfill_run (finished_at desc);
