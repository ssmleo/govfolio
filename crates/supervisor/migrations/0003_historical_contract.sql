INSERT OR IGNORE INTO control_schema_version (version, applied_at_ms)
VALUES (3, CAST(strftime('%s', 'now') AS INTEGER) * 1000);

CREATE TABLE IF NOT EXISTS lane_historical_contract (
    activation_id TEXT PRIMARY KEY,
    lane_id TEXT NOT NULL REFERENCES lane_lease(lane_id),
    expected_branch TEXT NOT NULL,
    worktree TEXT NOT NULL,
    work_key TEXT NOT NULL,
    merge_base_sha TEXT NOT NULL,
    active_policy_sha256 TEXT NOT NULL,
    source_sha TEXT NOT NULL,
    changed_paths_json TEXT NOT NULL,
    activated_at_ms INTEGER NOT NULL,
    consumed_at_ms INTEGER,
    consumed_by_receipt_id TEXT,
    CHECK (consumed_at_ms IS NULL OR consumed_at_ms >= activated_at_ms),
    CHECK ((consumed_at_ms IS NULL) = (consumed_by_receipt_id IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS lane_historical_contract_active
ON lane_historical_contract(lane_id) WHERE consumed_at_ms IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS lane_historical_contract_receipt
ON lane_historical_contract(consumed_by_receipt_id)
WHERE consumed_by_receipt_id IS NOT NULL;
