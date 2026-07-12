use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row as _, Sqlite, Transaction};

use crate::build_classifier::ResourceClass;
use crate::build_policy::{BuildPolicySnapshot, BuildPolicyStatus};
use crate::process::observed_process_identity;
use crate::store::{ControlStore, StoreError, SupervisorFence};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildRequestState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    Inconclusive,
    RecoveryRequired,
}

impl BuildRequestState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Inconclusive => "inconclusive",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "timed_out" => Ok(Self::TimedOut),
            "inconclusive" => Ok(Self::Inconclusive),
            "recovery_required" => Ok(Self::RecoveryRequired),
            other => Err(StoreError::Integrity(format!(
                "unknown build request state {other:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub started_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildRequestSpec {
    pub request_id: String,
    pub lane_id: Option<String>,
    pub lane_fence: Option<i64>,
    pub owner_identity: String,
    pub policy_sha256: String,
    pub resource_class: ResourceClass,
    pub category: Option<String>,
    pub worktree: PathBuf,
    pub target_dir: PathBuf,
    pub command_sha256: String,
    pub effective_jobs: usize,
    pub deadline: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildRequestRecord {
    pub request_id: String,
    pub queue_sequence: i64,
    pub supervisor_fence: i64,
    pub lane_id: Option<String>,
    pub lane_fence: Option<i64>,
    pub owner_identity: String,
    pub policy_sha256: String,
    pub resource_class: ResourceClass,
    pub category: Option<String>,
    pub worktree: PathBuf,
    pub target_dir: PathBuf,
    pub command_sha256: String,
    pub effective_jobs: usize,
    pub state: BuildRequestState,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub deadline: DateTime<Utc>,
    pub process_identity: Option<ProcessIdentity>,
    pub exit_code: Option<i32>,
    pub outcome: Option<String>,
    pub evidence_sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildTerminal {
    Completed { exit_code: i32 },
    Failed { exit_code: i32 },
    Cancelled,
    TimedOut,
    Inconclusive { reason: String },
}

impl BuildTerminal {
    fn fields(&self) -> (BuildRequestState, Option<i32>, String) {
        match self {
            Self::Completed { exit_code } => (
                BuildRequestState::Completed,
                Some(*exit_code),
                "completed".to_owned(),
            ),
            Self::Failed { exit_code } => (
                BuildRequestState::Failed,
                Some(*exit_code),
                "failed".to_owned(),
            ),
            Self::Cancelled => (BuildRequestState::Cancelled, None, "cancelled".to_owned()),
            Self::TimedOut => (BuildRequestState::TimedOut, None, "timed_out".to_owned()),
            Self::Inconclusive { reason } => (
                BuildRequestState::Inconclusive,
                None,
                format!("inconclusive:{reason}"),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BuildReconciliation {
    pub cancelled_queued: u64,
    pub recovery_required: u64,
}

impl ControlStore {
    pub async fn control_schema_version(&self) -> Result<i64, StoreError> {
        let version =
            sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM control_schema_version")
                .fetch_one(&self.pool)
                .await?;
        Ok(version)
    }

    pub async fn record_build_policy_snapshot(
        &self,
        snapshot: &BuildPolicySnapshot,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT OR IGNORE INTO build_policy_snapshot \
             (policy_sha256, schema_version, status, source_commit, loaded_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&snapshot.policy_sha256)
        .bind(i64::from(snapshot.schema_version))
        .bind(policy_status(snapshot.status))
        .bind(&snapshot.source_commit)
        .bind(snapshot.loaded_at.timestamp_millis())
        .execute(&self.pool)
        .await?;
        let row = sqlx::query(
            "SELECT schema_version, status, source_commit, loaded_at_ms \
             FROM build_policy_snapshot WHERE policy_sha256 = ?1",
        )
        .bind(&snapshot.policy_sha256)
        .fetch_one(&self.pool)
        .await?;
        let unchanged = row.try_get::<i64, _>("schema_version")?
            == i64::from(snapshot.schema_version)
            && row.try_get::<String, _>("status")? == policy_status(snapshot.status);
        if unchanged {
            Ok(())
        } else {
            Err(StoreError::Integrity(format!(
                "build policy snapshot {} is not immutable",
                snapshot.policy_sha256
            )))
        }
    }

    pub async fn enqueue_build(
        &self,
        supervisor: &SupervisorFence,
        spec: &BuildRequestSpec,
        now: DateTime<Utc>,
    ) -> Result<BuildRequestRecord, StoreError> {
        self.ensure_writer()?;
        validate_spec(spec, now)?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        validate_lane(&self.pool, supervisor, spec, now).await?;
        let worktree = path_text(&spec.worktree)?;
        let target_dir = path_text(&spec.target_dir)?;
        sqlx::query(
            "INSERT INTO build_request \
             (request_id, supervisor_fence, lane_id, lane_fence, owner_identity, policy_sha256, \
              resource_class, category, worktree, target_dir, command_sha256, effective_jobs, \
              state, queued_at_ms, heartbeat_at_ms, deadline_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, \
                     'queued', ?13, ?13, ?14)",
        )
        .bind(&spec.request_id)
        .bind(supervisor.fence)
        .bind(&spec.lane_id)
        .bind(spec.lane_fence)
        .bind(&spec.owner_identity)
        .bind(&spec.policy_sha256)
        .bind(resource_class(spec.resource_class))
        .bind(&spec.category)
        .bind(worktree)
        .bind(target_dir)
        .bind(&spec.command_sha256)
        .bind(i64::try_from(spec.effective_jobs).map_err(|_| {
            StoreError::InvalidBuildTransition("effective jobs exceed i64".to_owned())
        })?)
        .bind(now.timestamp_millis())
        .bind(spec.deadline.timestamp_millis())
        .execute(&self.pool)
        .await?;
        self.append_build_event(&spec.request_id, "queued", "{}", now)
            .await?;
        self.build_request(&spec.request_id)
            .await?
            .ok_or_else(|| StoreError::BuildRequestNotFound(spec.request_id.clone()))
    }

    pub async fn start_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        identity: &ProcessIdentity,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        ensure_request_lane(&self.pool, supervisor, request_id, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET state = 'running', started_at_ms = ?1, \
             heartbeat_at_ms = ?1, pid = ?2, pid_started_at_ms = ?3 \
             WHERE request_id = ?4 AND state = 'queued' AND supervisor_fence = ?5",
        )
        .bind(now.timestamp_millis())
        .bind(i64::from(identity.pid))
        .bind(identity.started_at_ms)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "queued -> running")?;
        self.append_build_event(request_id, "running", "{}", now)
            .await
    }

    pub async fn heartbeat_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        ensure_request_lane(&self.pool, supervisor, request_id, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET heartbeat_at_ms = ?1 \
             WHERE request_id = ?2 AND state = 'running' AND supervisor_fence = ?3",
        )
        .bind(now.timestamp_millis())
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "running heartbeat")
    }

    pub async fn retry_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        identity: &ProcessIdentity,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        ensure_request_lane(&self.pool, supervisor, request_id, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET retry_count = retry_count + 1, heartbeat_at_ms = ?1, \
             pid = ?2, pid_started_at_ms = ?3 \
             WHERE request_id = ?4 AND state = 'running' AND supervisor_fence = ?5 \
             AND retry_count = 0",
        )
        .bind(now.timestamp_millis())
        .bind(i64::from(identity.pid))
        .bind(identity.started_at_ms)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(
            result.rows_affected(),
            request_id,
            "running transient retry",
        )?;
        self.append_build_event(request_id, "transient_retry", "{}", now)
            .await
    }

    pub async fn finish_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        terminal: BuildTerminal,
        evidence_sha256: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        ensure_request_lane(&self.pool, supervisor, request_id, now).await?;
        let (state, exit_code, outcome) = terminal.fields();
        let result = sqlx::query(
            "UPDATE build_request SET state = ?1, finished_at_ms = ?2, heartbeat_at_ms = ?2, \
             exit_code = ?3, outcome = ?4, evidence_sha256 = ?5 \
             WHERE request_id = ?6 AND state = 'running' AND supervisor_fence = ?7",
        )
        .bind(state.as_str())
        .bind(now.timestamp_millis())
        .bind(exit_code)
        .bind(&outcome)
        .bind(evidence_sha256)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "running -> terminal")?;
        self.append_build_event(request_id, state.as_str(), "{}", now)
            .await
    }

    pub async fn mark_build_recovery_required(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        reason: &str,
        evidence_sha256: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET state = 'recovery_required', heartbeat_at_ms = ?1, \
             outcome = ?2, evidence_sha256 = ?3 \
             WHERE request_id = ?4 AND state = 'running' AND supervisor_fence = ?5",
        )
        .bind(now.timestamp_millis())
        .bind(reason)
        .bind(evidence_sha256)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(
            result.rows_affected(),
            request_id,
            "running -> recovery_required",
        )?;
        self.append_build_event(request_id, "recovery_required", "{}", now)
            .await
    }

    pub async fn build_request(
        &self,
        request_id: &str,
    ) -> Result<Option<BuildRequestRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT request_id, queue_sequence, supervisor_fence, lane_id, lane_fence, \
             owner_identity, policy_sha256, resource_class, category, worktree, target_dir, \
             command_sha256, effective_jobs, state, queued_at_ms, started_at_ms, heartbeat_at_ms, \
             finished_at_ms, deadline_at_ms, pid, pid_started_at_ms, exit_code, outcome, \
             evidence_sha256 FROM build_request WHERE request_id = ?1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| build_request_from_row(&row)).transpose()
    }

    pub async fn list_build_requests(&self) -> Result<Vec<BuildRequestRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT request_id, queue_sequence, supervisor_fence, lane_id, lane_fence, \
             owner_identity, policy_sha256, resource_class, category, worktree, target_dir, \
             command_sha256, effective_jobs, state, queued_at_ms, started_at_ms, heartbeat_at_ms, \
             finished_at_ms, deadline_at_ms, pid, pid_started_at_ms, exit_code, outcome, \
             evidence_sha256 FROM build_request ORDER BY queue_sequence",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(build_request_from_row).collect()
    }

    pub async fn cancel_queued_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET state = 'cancelled', finished_at_ms = ?1, \
             heartbeat_at_ms = ?1, outcome = ?2 \
             WHERE request_id = ?3 AND state = 'queued' AND supervisor_fence = ?4",
        )
        .bind(now.timestamp_millis())
        .bind(reason)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "queued -> cancelled")?;
        self.append_build_event(request_id, "cancelled", "{}", now)
            .await
    }

    pub async fn fail_queued_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        reason: &str,
        evidence_sha256: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        ensure_request_lane(&self.pool, supervisor, request_id, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET state = 'failed', finished_at_ms = ?1, \
             heartbeat_at_ms = ?1, exit_code = 1, outcome = ?2, evidence_sha256 = ?3 \
             WHERE request_id = ?4 AND state = 'queued' AND supervisor_fence = ?5",
        )
        .bind(now.timestamp_millis())
        .bind(reason)
        .bind(evidence_sha256)
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(
            result.rows_affected(),
            request_id,
            "queued launch -> failed",
        )?;
        self.append_build_event(request_id, "failed", "{}", now)
            .await
    }

    pub async fn timeout_queued_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        let result = sqlx::query(
            "UPDATE build_request SET state = 'timed_out', finished_at_ms = ?1, \
             heartbeat_at_ms = ?1, outcome = 'queue_deadline' \
             WHERE request_id = ?2 AND state = 'queued' AND supervisor_fence = ?3",
        )
        .bind(now.timestamp_millis())
        .bind(request_id)
        .bind(supervisor.fence)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "queued -> timed_out")?;
        self.append_build_event(request_id, "timed_out", "{}", now)
            .await
    }

    pub async fn record_build_evidence(
        &self,
        request_id: &str,
        evidence_sha256: &str,
        evidence_kind: &str,
        protected_path: &Path,
        size_bytes: u64,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT INTO build_evidence \
             (evidence_sha256, request_id, evidence_kind, protected_path, size_bytes, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(evidence_sha256)
        .bind(request_id)
        .bind(evidence_kind)
        .bind(path_text(protected_path)?)
        .bind(i64::try_from(size_bytes).map_err(|_| {
            StoreError::InvalidBuildTransition("evidence size exceeds i64".to_owned())
        })?)
        .bind(now.timestamp_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn reconcile_build_requests(
        &self,
        supervisor: &SupervisorFence,
        now: DateTime<Utc>,
    ) -> Result<BuildReconciliation, StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        let mut tx = self.pool.begin().await?;
        let cancelled = sqlx::query(
            "UPDATE build_request SET state = 'cancelled', finished_at_ms = ?1, \
             heartbeat_at_ms = ?1, outcome = 'supervisor_restarted' \
             WHERE state = 'queued' AND supervisor_fence <> ?2",
        )
        .bind(now.timestamp_millis())
        .bind(supervisor.fence)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let recovery = sqlx::query(
            "UPDATE build_request SET state = 'recovery_required', heartbeat_at_ms = ?1, \
             outcome = 'supervisor_restarted_while_running' \
             WHERE state = 'running' AND supervisor_fence <> ?2",
        )
        .bind(now.timestamp_millis())
        .bind(supervisor.fence)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(BuildReconciliation {
            cancelled_queued: cancelled,
            recovery_required: recovery,
        })
    }

    pub async fn has_build_recovery_required(&self) -> Result<bool, StoreError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM build_request WHERE state = 'recovery_required'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn recover_build(
        &self,
        supervisor: &SupervisorFence,
        request_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        ensure_current_supervisor(&self.pool, supervisor, now).await?;
        let record = self
            .build_request(request_id)
            .await?
            .ok_or_else(|| StoreError::BuildRequestNotFound(request_id.to_owned()))?;
        if record.state != BuildRequestState::RecoveryRequired {
            return Err(StoreError::InvalidBuildTransition(format!(
                "request {request_id:?} is not recovery_required"
            )));
        }
        let recorded = record.process_identity.ok_or_else(|| {
            StoreError::RecoveryRequired(format!(
                "build request {request_id} has no complete PID/start identity"
            ))
        })?;
        let observed = observed_process_identity(recorded.pid)?;
        if observed.as_ref() == Some(&recorded) {
            return Err(StoreError::RecoveryRequired(format!(
                "build request {request_id} still owns PID {} with the recorded identity",
                recorded.pid
            )));
        }
        let result = sqlx::query(
            "UPDATE build_request SET state = 'cancelled', finished_at_ms = ?1, \
             heartbeat_at_ms = ?1, outcome = 'recovery_proved_process_dead' \
             WHERE request_id = ?2 AND state = 'recovery_required'",
        )
        .bind(now.timestamp_millis())
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        require_transition(result.rows_affected(), request_id, "recovery -> cancelled")?;
        self.append_build_event(request_id, "recovery_cleared", "{}", now)
            .await
    }

    async fn append_build_event(
        &self,
        request_id: &str,
        kind: &str,
        payload_json: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        append_event(&mut tx, request_id, kind, payload_json, now).await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn append_event(
    tx: &mut Transaction<'_, Sqlite>,
    request_id: &str,
    kind: &str,
    payload_json: &str,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO build_request_event \
         (request_id, sequence, event_kind, payload_json, created_at_ms) \
         SELECT ?1, COALESCE(MAX(sequence), -1) + 1, ?2, ?3, ?4 \
         FROM build_request_event WHERE request_id = ?1",
    )
    .bind(request_id)
    .bind(kind)
    .bind(payload_json)
    .bind(now.timestamp_millis())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn ensure_current_supervisor(
    pool: &sqlx::SqlitePool,
    supervisor: &SupervisorFence,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let current = sqlx::query(
        "SELECT fence FROM supervisor_lease WHERE singleton = 1 AND owner_id = ?1 \
         AND status = 'owned' AND lease_until_ms >= ?2",
    )
    .bind(&supervisor.owner_id)
    .bind(now.timestamp_millis())
    .fetch_optional(pool)
    .await?
    .map(|row| row.get::<i64, _>("fence"));
    if current == Some(supervisor.fence) {
        Ok(())
    } else {
        Err(StoreError::StaleFence {
            scope: "supervisor build admission".to_owned(),
            expected: supervisor.fence,
            current,
        })
    }
}

async fn validate_lane(
    pool: &sqlx::SqlitePool,
    supervisor: &SupervisorFence,
    spec: &BuildRequestSpec,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    match (&spec.lane_id, spec.lane_fence) {
        (None, None) => Ok(()),
        (Some(lane_id), Some(lane_fence)) => {
            let valid = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM lane_lease WHERE lane_id = ?1 AND fence = ?2 \
                 AND supervisor_fence = ?3 AND status = 'owned' AND lease_until_ms >= ?4 \
                 AND owner_id = ?5",
            )
            .bind(lane_id)
            .bind(lane_fence)
            .bind(supervisor.fence)
            .bind(now.timestamp_millis())
            .bind(&spec.owner_identity)
            .fetch_one(pool)
            .await?;
            if valid == 1 {
                Ok(())
            } else {
                Err(StoreError::StaleFence {
                    scope: format!("lane {lane_id}"),
                    expected: lane_fence,
                    current: None,
                })
            }
        }
        _ => Err(StoreError::InvalidBuildTransition(
            "lane id and lane fence must be supplied together".to_owned(),
        )),
    }
}

async fn ensure_request_lane(
    pool: &sqlx::SqlitePool,
    supervisor: &SupervisorFence,
    request_id: &str,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let row = sqlx::query("SELECT lane_id, lane_fence FROM build_request WHERE request_id = ?1")
        .bind(request_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| StoreError::BuildRequestNotFound(request_id.to_owned()))?;
    let lane_id = row.try_get::<Option<String>, _>("lane_id")?;
    let lane_fence = row.try_get::<Option<i64>, _>("lane_fence")?;
    match (lane_id, lane_fence) {
        (None, None) => Ok(()),
        (Some(lane_id), Some(lane_fence)) => {
            let valid = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM lane_lease WHERE lane_id = ?1 AND fence = ?2 \
                 AND supervisor_fence = ?3 AND status = 'owned' AND lease_until_ms >= ?4",
            )
            .bind(&lane_id)
            .bind(lane_fence)
            .bind(supervisor.fence)
            .bind(now.timestamp_millis())
            .fetch_one(pool)
            .await?;
            if valid == 1 {
                Ok(())
            } else {
                Err(StoreError::StaleFence {
                    scope: format!("build request lane {lane_id}"),
                    expected: lane_fence,
                    current: None,
                })
            }
        }
        _ => Err(StoreError::Integrity(format!(
            "build request {request_id} has partial lane identity"
        ))),
    }
}

fn validate_spec(spec: &BuildRequestSpec, now: DateTime<Utc>) -> Result<(), StoreError> {
    if spec.request_id.trim().is_empty()
        || spec.owner_identity.trim().is_empty()
        || spec.policy_sha256.len() != 64
        || spec.command_sha256.len() != 64
        || spec.effective_jobs == 0
        || spec.deadline <= now
    {
        return Err(StoreError::InvalidBuildTransition(
            "invalid build request identity, hash, jobs, or deadline".to_owned(),
        ));
    }
    Ok(())
}

fn require_transition(rows: u64, request_id: &str, transition: &str) -> Result<(), StoreError> {
    if rows == 1 {
        Ok(())
    } else {
        Err(StoreError::InvalidBuildTransition(format!(
            "request {request_id:?} cannot perform {transition}"
        )))
    }
}

fn build_request_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<BuildRequestRecord, StoreError> {
    let state = BuildRequestState::parse(&row.try_get::<String, _>("state")?)?;
    let class = match row.try_get::<String, _>("resource_class")?.as_str() {
        "focused" => ResourceClass::Focused,
        "exclusive" => ResourceClass::Exclusive,
        other => {
            return Err(StoreError::Integrity(format!(
                "unknown build resource class {other:?}"
            )));
        }
    };
    let pid = row.try_get::<Option<i64>, _>("pid")?;
    let pid_started_at_ms = row.try_get::<Option<i64>, _>("pid_started_at_ms")?;
    let process_identity = match (pid, pid_started_at_ms) {
        (Some(pid), Some(started_at_ms)) => Some(ProcessIdentity {
            pid: u32::try_from(pid)
                .map_err(|_| StoreError::Integrity(format!("invalid persisted build PID {pid}")))?,
            started_at_ms,
        }),
        (None, None) => None,
        _ => {
            return Err(StoreError::Integrity(
                "build PID identity is partially populated".to_owned(),
            ));
        }
    };
    Ok(BuildRequestRecord {
        request_id: row.try_get("request_id")?,
        queue_sequence: row.try_get("queue_sequence")?,
        supervisor_fence: row.try_get("supervisor_fence")?,
        lane_id: row.try_get("lane_id")?,
        lane_fence: row.try_get("lane_fence")?,
        owner_identity: row.try_get("owner_identity")?,
        policy_sha256: row.try_get("policy_sha256")?,
        resource_class: class,
        category: row.try_get("category")?,
        worktree: PathBuf::from(row.try_get::<String, _>("worktree")?),
        target_dir: PathBuf::from(row.try_get::<String, _>("target_dir")?),
        command_sha256: row.try_get("command_sha256")?,
        effective_jobs: usize::try_from(row.try_get::<i64, _>("effective_jobs")?)
            .map_err(|_| StoreError::Integrity("invalid persisted effective jobs".to_owned()))?,
        state,
        queued_at: timestamp(row.try_get("queued_at_ms")?)?,
        started_at: optional_timestamp(row.try_get("started_at_ms")?)?,
        heartbeat_at: optional_timestamp(row.try_get("heartbeat_at_ms")?)?,
        finished_at: optional_timestamp(row.try_get("finished_at_ms")?)?,
        deadline: timestamp(row.try_get("deadline_at_ms")?)?,
        process_identity,
        exit_code: row.try_get("exit_code")?,
        outcome: row.try_get("outcome")?,
        evidence_sha256: row.try_get("evidence_sha256")?,
    })
}

fn timestamp(value: i64) -> Result<DateTime<Utc>, StoreError> {
    DateTime::from_timestamp_millis(value).ok_or(StoreError::InvalidTimestamp(value))
}

fn optional_timestamp(value: Option<i64>) -> Result<Option<DateTime<Utc>>, StoreError> {
    value.map(timestamp).transpose()
}

fn resource_class(class: ResourceClass) -> &'static str {
    match class {
        ResourceClass::Focused => "focused",
        ResourceClass::Exclusive => "exclusive",
    }
}

fn policy_status(status: BuildPolicyStatus) -> &'static str {
    match status {
        BuildPolicyStatus::Advisory => "advisory",
        BuildPolicyStatus::Shadow => "shadow",
        BuildPolicyStatus::Enforced => "enforced",
    }
}

fn path_text(path: &Path) -> Result<&str, StoreError> {
    path.to_str().ok_or_else(|| {
        StoreError::InvalidBuildTransition("build path is not valid UTF-8".to_owned())
    })
}
