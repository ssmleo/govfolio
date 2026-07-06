-- 0009_sample_audit: monthly sampling-audit queue + precision-report source
-- (design §7.4 "≥99% extraction precision on a monthly random-sample audit";
-- goal 070 trust hardening). Expand-only: one additive table, no rewrite.
--
-- The sampler (worker::sampler) draws a stratified-per-regime, deterministically
-- seeded sample of published Gold records once a month and queues each drawn
-- record here as `pending`. An auditor (human or the expected.*.json
-- auto-resolution the automation-policy references) then marks each row
-- `confirmed` or `discrepancy`; the per-regime precision estimate is computed
-- from those outcomes. Keyed by `regime_id` (the real Gold regime FK) — the
-- sample is a fact about published records, unlike the operational
-- `sentinel_watch`/`drift_report` tables which key by adapter regime_code.
--
-- `unique (sample_month, record_id)` makes a month's draw idempotent: re-running
-- the same batch queues each record at most once (invariant 4).

create table sample_audit (
  id               text primary key,
  regime_id        text not null references disclosure_regime(id),
  record_id        text not null references disclosure_record(id),
  sample_month     text not null,               -- 'YYYY-MM' batch label
  seed             bigint not null,             -- deterministic draw seed (reproducible)
  status           text not null default 'pending'
                     check (status in ('pending','confirmed','discrepancy')),
  discrepancy_note text,                         -- what the audit found, when status='discrepancy'
  sampled_at       timestamptz not null default now(),
  audited_at       timestamptz,                  -- NULL = not yet audited
  unique (sample_month, record_id)
);

-- Precision report groups a batch by regime.
create index sample_audit_regime_month on sample_audit (regime_id, sample_month);
-- The audit queue: rows still awaiting a verdict.
create index sample_audit_pending on sample_audit (status) where status = 'pending';
