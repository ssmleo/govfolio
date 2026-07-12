INSERT OR IGNORE INTO control_schema_version (version, applied_at_ms)
VALUES (2, CAST(strftime('%s', 'now') AS INTEGER) * 1000);

CREATE TABLE IF NOT EXISTS build_policy_snapshot (
    policy_sha256 TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    status TEXT NOT NULL CHECK (status IN ('advisory', 'shadow', 'enforced')),
    source_commit TEXT NOT NULL,
    loaded_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS build_request (
    queue_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL UNIQUE,
    supervisor_fence INTEGER NOT NULL CHECK (supervisor_fence > 0),
    lane_id TEXT,
    lane_fence INTEGER CHECK (lane_fence > 0),
    owner_identity TEXT NOT NULL,
    policy_sha256 TEXT NOT NULL REFERENCES build_policy_snapshot(policy_sha256),
    resource_class TEXT NOT NULL CHECK (resource_class IN ('focused', 'exclusive')),
    category TEXT,
    worktree TEXT NOT NULL,
    target_dir TEXT NOT NULL,
    command_sha256 TEXT NOT NULL,
    effective_jobs INTEGER NOT NULL CHECK (effective_jobs > 0),
    state TEXT NOT NULL CHECK (
        state IN (
            'queued', 'running', 'completed', 'failed', 'cancelled', 'timed_out',
            'inconclusive', 'recovery_required'
        )
    ),
    queued_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER,
    heartbeat_at_ms INTEGER,
    finished_at_ms INTEGER,
    deadline_at_ms INTEGER NOT NULL,
    pid INTEGER CHECK (pid > 0),
    pid_started_at_ms INTEGER,
    exit_code INTEGER,
    outcome TEXT,
    evidence_sha256 TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count BETWEEN 0 AND 1),
    CHECK ((lane_id IS NULL) = (lane_fence IS NULL))
);

CREATE INDEX IF NOT EXISTS build_request_state_queue_idx
    ON build_request (state, queue_sequence);
CREATE INDEX IF NOT EXISTS build_request_fence_state_idx
    ON build_request (supervisor_fence, state);
CREATE INDEX IF NOT EXISTS build_request_class_state_idx
    ON build_request (resource_class, state, queue_sequence);

CREATE TABLE IF NOT EXISTS build_request_event (
    request_id TEXT NOT NULL REFERENCES build_request(request_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    event_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (request_id, sequence)
);

CREATE INDEX IF NOT EXISTS build_request_event_time_idx
    ON build_request_event (created_at_ms);

CREATE TABLE IF NOT EXISTS build_evidence (
    evidence_sha256 TEXT PRIMARY KEY,
    request_id TEXT NOT NULL REFERENCES build_request(request_id) ON DELETE CASCADE,
    evidence_kind TEXT NOT NULL,
    protected_path TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS build_evidence_request_idx
    ON build_evidence (request_id, evidence_kind);
