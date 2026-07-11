CREATE TABLE IF NOT EXISTS control_schema_version (
    version INTEGER PRIMARY KEY,
    applied_at_ms INTEGER NOT NULL
);

INSERT OR IGNORE INTO control_schema_version (version, applied_at_ms)
VALUES (1, CAST(strftime('%s', 'now') AS INTEGER) * 1000);

CREATE TABLE IF NOT EXISTS control_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS supervisor_lease (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    owner_id TEXT NOT NULL,
    fence INTEGER NOT NULL CHECK (fence > 0),
    status TEXT NOT NULL CHECK (status IN ('owned', 'released')),
    pid INTEGER,
    heartbeat_at_ms INTEGER NOT NULL,
    lease_until_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS lane_lease (
    lane_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    fence INTEGER NOT NULL CHECK (fence > 0),
    supervisor_fence INTEGER NOT NULL CHECK (supervisor_fence > 0),
    status TEXT NOT NULL CHECK (status IN ('owned', 'released', 'recovery_required')),
    role TEXT,
    worktree TEXT,
    expected_branch TEXT,
    provider_key TEXT,
    pid INTEGER,
    heartbeat_at_ms INTEGER NOT NULL,
    lease_until_ms INTEGER NOT NULL,
    recovery_reason TEXT
);

CREATE TABLE IF NOT EXISTS attempt (
    attempt_id TEXT PRIMARY KEY,
    lane_id TEXT NOT NULL REFERENCES lane_lease(lane_id),
    lane_fence INTEGER NOT NULL CHECK (lane_fence > 0),
    work_key TEXT NOT NULL,
    attempt_ordinal INTEGER NOT NULL CHECK (attempt_ordinal > 0),
    provider_key TEXT NOT NULL,
    config_fingerprint TEXT NOT NULL,
    preflight_signature TEXT NOT NULL,
    state TEXT NOT NULL CHECK (
        state IN ('reserved', 'running', 'completed', 'failed', 'recovery_required')
    ),
    result_class TEXT,
    session_id TEXT,
    exit_code INTEGER,
    structured_started INTEGER CHECK (structured_started IN (0, 1)),
    terminal_type TEXT,
    stable_error_hash TEXT,
    failure_fingerprint TEXT,
    exemplar_ref TEXT,
    spec_json TEXT NOT NULL,
    git_head_before TEXT NOT NULL,
    journal_sha_before TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER,
    finished_at_ms INTEGER,
    UNIQUE (work_key, attempt_ordinal)
);

CREATE INDEX IF NOT EXISTS attempt_lane_fence_idx
    ON attempt (lane_id, lane_fence);
CREATE INDEX IF NOT EXISTS attempt_work_key_idx
    ON attempt (work_key);

CREATE TABLE IF NOT EXISTS attempt_checkpoint (
    attempt_id TEXT NOT NULL REFERENCES attempt(attempt_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    state TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (attempt_id, sequence)
);

CREATE TABLE IF NOT EXISTS provider_circuit (
    provider_key TEXT PRIMARY KEY,
    state TEXT NOT NULL CHECK (state IN ('closed', 'open', 'disabled', 'half_open')),
    config_fingerprint TEXT NOT NULL,
    reason TEXT,
    opened_at_ms INTEGER,
    retry_at_ms INTEGER,
    last_failure_at_ms INTEGER,
    half_open_owner TEXT,
    half_open_until_ms INTEGER,
    consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0)
);

CREATE TABLE IF NOT EXISTS system_circuit (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    state TEXT NOT NULL CHECK (state IN ('closed', 'paused')),
    reason TEXT,
    opened_at_ms INTEGER,
    retry_at_ms INTEGER,
    last_failure_at_ms INTEGER,
    diagnostics_passed_at_ms INTEGER
);

INSERT OR IGNORE INTO system_circuit (singleton, state) VALUES (1, 'closed');

CREATE TABLE IF NOT EXISTS launch_failure (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS launch_failure_provider_time_idx
    ON launch_failure (provider_key, occurred_at_ms);
CREATE INDEX IF NOT EXISTS launch_failure_time_idx
    ON launch_failure (occurred_at_ms);

CREATE TABLE IF NOT EXISTS failure_bucket (
    fingerprint TEXT PRIMARY KEY,
    provider_key TEXT NOT NULL,
    window_started_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL,
    failure_count INTEGER NOT NULL CHECK (failure_count > 0),
    suppressed_count INTEGER NOT NULL DEFAULT 0 CHECK (suppressed_count >= 0),
    open_until_ms INTEGER,
    exemplar_ref TEXT
);

CREATE TABLE IF NOT EXISTS suppression_counter (
    reason TEXT NOT NULL,
    provider_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    count INTEGER NOT NULL CHECK (count > 0),
    first_seen_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL,
    retry_at_ms INTEGER,
    PRIMARY KEY (reason, provider_key, fingerprint)
);

CREATE TABLE IF NOT EXISTS probe_cache (
    probe_key TEXT PRIMARY KEY,
    input_fingerprint TEXT NOT NULL,
    outcome TEXT NOT NULL,
    details_json TEXT NOT NULL,
    checked_at_ms INTEGER NOT NULL,
    valid_until_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_compatibility (
    provider_key TEXT NOT NULL,
    cli_version TEXT NOT NULL,
    model TEXT NOT NULL,
    config_fingerprint TEXT NOT NULL,
    compatibility_kind TEXT NOT NULL,
    proven INTEGER NOT NULL CHECK (proven IN (0, 1)),
    proof_ref TEXT,
    checked_at_ms INTEGER NOT NULL,
    valid_until_ms INTEGER,
    PRIMARY KEY (
        provider_key,
        cli_version,
        model,
        config_fingerprint,
        compatibility_kind
    )
);

CREATE TABLE IF NOT EXISTS artifact_index (
    sha256 TEXT PRIMARY KEY,
    relative_path TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    reference_count INTEGER NOT NULL CHECK (reference_count >= 0),
    protected INTEGER NOT NULL DEFAULT 0 CHECK (protected IN (0, 1)),
    created_at_ms INTEGER NOT NULL,
    last_accessed_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attempt_artifact (
    attempt_id TEXT NOT NULL REFERENCES attempt(attempt_id) ON DELETE CASCADE,
    artifact_kind TEXT NOT NULL,
    sha256 TEXT NOT NULL REFERENCES artifact_index(sha256),
    PRIMARY KEY (attempt_id, artifact_kind)
);

CREATE TABLE IF NOT EXISTS backup_history (
    path TEXT PRIMARY KEY,
    created_at_ms INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0)
);

CREATE TABLE IF NOT EXISTS integration_mirror (
    receipt_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    branch TEXT,
    pull_request INTEGER,
    candidate_sha TEXT,
    merge_sha TEXT,
    last_error TEXT,
    updated_at_ms INTEGER NOT NULL
);
