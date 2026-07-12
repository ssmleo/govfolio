-- 0015_historical_contract_receipt: optional immutable evidence for application
-- work produced from a trusted stale worktree. Expand-only and nullable for all
-- existing receipts.

alter table integration_receipt
  add column historical_contract jsonb
  check (
    historical_contract is null
    or jsonb_typeof(historical_contract) = 'object'
  );

alter table integration_receipt_state
  add column candidate_sha text;
