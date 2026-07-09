-- 0012_ops_indexes: read-path indexes for the admin ops observability surface
-- (goal 090, /v1/admin/ops/*). Expand-only: five additive indexes, no rewrite
-- of existing rows, passes scripts/check-migration-safety.sh.
--
-- pipeline_run_started       — /ops/runs keyset (id desc ~ time desc) + the
--                              24h/summary windows scan started_at.
-- pipeline_run_adapter_stage — /ops/runs filters (adapter, stage, status) and
--                              the per-adapter stage rollup in /ops/backfill.
-- filing_regime_filed        — /ops/backfill year bucketing groups filings by
--                              (regime_id, extract(year from filed_date)).
-- disclosure_record_filing   — gold-per-filing joins (/ops/backfill gold-by-
--                              year attributes records through filing).
-- review_task_status_reason  — /ops/review-health open-by-reason rollup and
--                              every status-filtered task count.
--
-- Deliberately NOT indexed: raw_document.fetch_run_id — no query anywhere
-- filters or joins on it (no per-run Bronze drill-down endpoint exists), so an
-- index would be pure write-amplification on every Bronze ingest.

create index pipeline_run_started       on pipeline_run (started_at desc);
create index pipeline_run_adapter_stage on pipeline_run (adapter, stage, status, started_at desc);
create index filing_regime_filed        on filing (regime_id, filed_date);
create index disclosure_record_filing   on disclosure_record (filing_id);
create index review_task_status_reason  on review_task (status, reason);
