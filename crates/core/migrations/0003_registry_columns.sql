-- 0003_registry_columns: coverage-factory state on jurisdiction (design §5.8,
-- goal 015). Expand-only: additive columns, no rewrite of existing rows beyond
-- the constant default.
--
-- The registry IS the work queue: coverage_phase is the §5.8 state machine
-- (stub → scouted → surveyed → sampled → specced → built → live | blocked);
-- 'blocked:<reason>' is spelled as phase 'blocked' + blocked_reason. Leasing
-- (claimed_by/claimed_at) keeps parallel loop instances off the same
-- jurisdiction; stale leases (>24h) are free per the workflow. priority_score
-- orders work within the current epoch (agents/EPOCHS.md).

alter table jurisdiction
  add column epoch          smallint,
  add column coverage_phase text not null default 'stub'
    check (coverage_phase in
      ('stub','scouted','surveyed','sampled','specced','built','live','blocked')),
  add column priority_score real,
  add column claimed_by     text,
  add column claimed_at     timestamptz,
  add column blocked_reason text;
