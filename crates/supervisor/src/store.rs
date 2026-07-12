use std::ffi::OsString;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Executor, Sqlite, SqlitePool, Transaction};
use thiserror::Error;
use ulid::Ulid;

use crate::model::{AttemptSpec, NormalizedResult, ResultClass, SuppressionReason};
use crate::policy::{PolicyDecision, RetryAction, StormThresholds};

const MIGRATION_0001: &str = include_str!("../migrations/0001_control_store.sql");
const MIGRATION_0002: &str = include_str!("../migrations/0002_build_admission.sql");
const BUSY_TIMEOUT: StdDuration = StdDuration::from_secs(5);

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("control-store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("control-store SQLite operation failed: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("control-store serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("another control-store writer owns {0}")]
    WriterAlreadyRunning(PathBuf),
    #[error("control-store integrity check failed: {0}")]
    Integrity(String),
    #[error("{scope} is owned by {owner}")]
    LeaseBusy { scope: String, owner: String },
    #[error("stale fence for {scope}: expected {expected}, current {current:?}")]
    StaleFence {
        scope: String,
        expected: i64,
        current: Option<i64>,
    },
    #[error("{0} requires explicit recovery before it can be acquired")]
    RecoveryRequired(String),
    #[error("attempt {0} does not exist")]
    AttemptNotFound(String),
    #[error("build request {0} does not exist")]
    BuildRequestNotFound(String),
    #[error("invalid build request transition: {0}")]
    InvalidBuildTransition(String),
    #[error("release-0 attempt budget is exhausted for {0}")]
    AttemptBudgetExhausted(String),
    #[error("invalid alternate-provider recovery attempt: {0}")]
    InvalidRecoveryAttempt(String),
    #[error("monitor connections are read-only")]
    ReadOnly,
    #[error("lease duration must be positive")]
    InvalidLeaseDuration,
    #[error("persisted timestamp {0} is outside the supported range")]
    InvalidTimestamp(i64),
    #[error("backup destination already exists: {0}")]
    BackupExists(PathBuf),
    #[error("unsupported SQLite pragma {0:?}")]
    UnsupportedPragma(String),
    #[error("fence counter is exhausted for {0}")]
    FenceExhausted(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoreMode {
    Writer,
    Monitor,
}

pub struct ControlStore {
    pub(crate) pool: SqlitePool,
    database_path: PathBuf,
    mode: StoreMode,
    _writer_lock: Option<File>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisorFence {
    pub owner_id: String,
    pub fence: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaneFence {
    pub lane_id: String,
    pub owner_id: String,
    pub fence: i64,
    pub supervisor_fence: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaneRuntimeContext {
    pub role: String,
    pub worktree: PathBuf,
    pub expected_branch: String,
    pub provider_key: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct FailureObservation<'a> {
    pub attempt_id: &'a str,
    pub provider_key: &'a str,
    pub config_fingerprint: &'a str,
    pub fingerprint: &'a str,
    pub exemplar_ref: Option<&'a str>,
    pub result: &'a NormalizedResult,
    pub decision: &'a PolicyDecision,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureBucket {
    pub fingerprint: String,
    pub provider_key: String,
    pub failure_count: i64,
    pub suppressed_count: i64,
    pub open_until: Option<DateTime<Utc>>,
    pub exemplar_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderGate {
    Closed,
    Open { retry_at: Option<DateTime<Utc>> },
    DisabledUntilFingerprintChanges,
    HalfOpenAvailable,
    HalfOpenOwned { until: DateTime<Utc> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemGate {
    Closed,
    Paused {
        retry_at: DateTime<Utc>,
        diagnostics_required: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FingerprintGate {
    Closed,
    Open { retry_at: DateTime<Utc> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeCacheEntry {
    pub probe_key: String,
    pub input_fingerprint: String,
    pub outcome: String,
    pub details_json: String,
    pub checked_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityRecord {
    pub provider_key: String,
    pub cli_version: String,
    pub model: String,
    pub config_fingerprint: String,
    pub compatibility_kind: String,
    pub proven: bool,
    pub proof_ref: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptMirror {
    pub receipt_id: String,
    pub state: String,
    pub branch: Option<String>,
    pub pull_request: Option<i64>,
    pub candidate_sha: Option<String>,
    pub merge_sha: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl ControlStore {
    pub async fn open_writer(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let database_path = path.as_ref().to_path_buf();
        create_parent(&database_path)?;
        let lock_path = writer_lock_path(&database_path);
        let writer_lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        match writer_lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(StoreError::WriterAlreadyRunning(lock_path));
            }
            Err(TryLockError::Error(source)) => return Err(StoreError::Io(source)),
        }

        let options = writer_options(&database_path);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::raw_sql(MIGRATION_0001).execute(&pool).await?;
        sqlx::raw_sql(MIGRATION_0002).execute(&pool).await?;
        let store = Self {
            pool,
            database_path,
            mode: StoreMode::Writer,
            _writer_lock: Some(writer_lock),
        };
        store.integrity_check().await?;
        Ok(store)
    }

    pub async fn open_monitor(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let database_path = path.as_ref().to_path_buf();
        let options = SqliteConnectOptions::new()
            .filename(&database_path)
            .read_only(true)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self {
            pool,
            database_path,
            mode: StoreMode::Monitor,
            _writer_lock: None,
        };
        store.integrity_check().await?;
        Ok(store)
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub async fn integrity_check(&self) -> Result<(), StoreError> {
        let results = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_all(&self.pool)
            .await?;
        if results.len() == 1 && results[0].eq_ignore_ascii_case("ok") {
            Ok(())
        } else {
            Err(StoreError::Integrity(results.join("; ")))
        }
    }

    pub async fn backup_atomic(&self, destination: &Path) -> Result<(), StoreError> {
        self.ensure_writer()?;
        if destination.exists() {
            return Err(StoreError::BackupExists(destination.to_path_buf()));
        }
        create_parent(destination).map_err(|error| {
            StoreError::Io(std::io::Error::new(
                error.kind(),
                format!("create backup parent: {error}"),
            ))
        })?;
        let temporary = temporary_backup_path(destination);
        let _checkpoint = sqlx::query("PRAGMA wal_checkpoint(FULL)")
            .fetch_all(&self.pool)
            .await?;
        let destination_sql = temporary.to_string_lossy().into_owned();
        if let Err(error) = sqlx::query("VACUUM INTO ?1")
            .persistent(false)
            .bind(destination_sql)
            .execute(&self.pool)
            .await
        {
            let _cleanup = std::fs::remove_file(&temporary);
            return Err(StoreError::Sql(error));
        }
        let temporary_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                StoreError::Io(std::io::Error::new(
                    error.kind(),
                    format!("open temporary backup for sync: {error}"),
                ))
            })?;
        temporary_file.sync_all().map_err(|error| {
            StoreError::Io(std::io::Error::new(
                error.kind(),
                format!("sync temporary backup: {error}"),
            ))
        })?;
        drop(temporary_file);
        std::fs::rename(&temporary, destination).map_err(|error| {
            StoreError::Io(std::io::Error::new(
                error.kind(),
                format!("publish temporary backup: {error}"),
            ))
        })?;
        sync_parent(destination).map_err(|error| {
            StoreError::Io(std::io::Error::new(
                error.kind(),
                format!("sync backup parent: {error}"),
            ))
        })?;
        Ok(())
    }

    pub async fn backup_if_due(
        &self,
        directory: &Path,
        now: DateTime<Utc>,
        interval: Duration,
    ) -> Result<Option<PathBuf>, StoreError> {
        self.ensure_writer()?;
        let now_ms = millis(now);
        let last_backup = sqlx::query_scalar::<_, String>(
            "SELECT value FROM control_metadata WHERE key = 'last_backup_ms'",
        )
        .fetch_optional(&self.pool)
        .await?
        .and_then(|value| value.parse::<i64>().ok());
        if last_backup.is_some_and(|last| now_ms.saturating_sub(last) < interval.num_milliseconds())
        {
            return Ok(None);
        }

        let destination =
            directory.join(format!("control-backup-{now_ms}-{}.sqlite3", Ulid::new()));
        self.backup_atomic(&destination).await?;
        let size_bytes = i64::try_from(std::fs::metadata(&destination)?.len()).unwrap_or(i64::MAX);
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO backup_history (path, created_at_ms, size_bytes) VALUES (?1, ?2, ?3)",
        )
        .bind(destination.to_string_lossy().as_ref())
        .bind(now_ms)
        .bind(size_bytes)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO control_metadata (key, value, updated_at_ms) VALUES ('last_backup_ms', ?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
        )
        .bind(now_ms.to_string())
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(destination))
    }

    #[cfg(test)]
    async fn pragma_i64(&self, name: &str) -> Result<i64, StoreError> {
        let query = match name {
            "synchronous" => "PRAGMA synchronous",
            "foreign_keys" => "PRAGMA foreign_keys",
            "busy_timeout" => "PRAGMA busy_timeout",
            _ => return Err(StoreError::UnsupportedPragma(name.to_owned())),
        };
        Ok(sqlx::query_scalar(query).fetch_one(&self.pool).await?)
    }

    #[cfg(test)]
    async fn pragma_text(&self, name: &str) -> Result<String, StoreError> {
        let query = match name {
            "journal_mode" => "PRAGMA journal_mode",
            _ => return Err(StoreError::UnsupportedPragma(name.to_owned())),
        };
        Ok(sqlx::query_scalar(query).fetch_one(&self.pool).await?)
    }

    pub(crate) fn ensure_writer(&self) -> Result<(), StoreError> {
        if self.mode == StoreMode::Writer {
            Ok(())
        } else {
            Err(StoreError::ReadOnly)
        }
    }
}

impl ControlStore {
    pub async fn acquire_supervisor(
        &self,
        owner_id: &str,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<SupervisorFence, StoreError> {
        self.ensure_writer()?;
        let now_ms = millis(now);
        let lease_until_ms = lease_until(now, ttl)?;
        let mut transaction = self.pool.begin().await?;
        let existing = sqlx::query_as::<_, (String, i64, String, i64)>(
            "SELECT owner_id, fence, status, lease_until_ms FROM supervisor_lease WHERE singleton = 1",
        )
        .fetch_optional(&mut *transaction)
        .await?;
        let fence = match existing {
            Some((owner, current, status, until)) => {
                if status == "owned" && until > now_ms {
                    return Err(StoreError::LeaseBusy {
                        scope: "supervisor".to_owned(),
                        owner,
                    });
                }
                current
                    .checked_add(1)
                    .ok_or_else(|| StoreError::FenceExhausted("supervisor".to_owned()))?
            }
            None => 1,
        };
        sqlx::query(
            "INSERT INTO supervisor_lease \
             (singleton, owner_id, fence, status, pid, heartbeat_at_ms, lease_until_ms) \
             VALUES (1, ?1, ?2, 'owned', NULL, ?3, ?4) \
             ON CONFLICT(singleton) DO UPDATE SET \
             owner_id = excluded.owner_id, fence = excluded.fence, status = 'owned', \
             pid = NULL, heartbeat_at_ms = excluded.heartbeat_at_ms, \
             lease_until_ms = excluded.lease_until_ms",
        )
        .bind(owner_id)
        .bind(fence)
        .bind(now_ms)
        .bind(lease_until_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(SupervisorFence {
            owner_id: owner_id.to_owned(),
            fence,
        })
    }

    pub async fn renew_supervisor(
        &self,
        token: &SupervisorFence,
        pid: u32,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE supervisor_lease SET pid = ?1, heartbeat_at_ms = ?2, lease_until_ms = ?3 \
             WHERE singleton = 1 AND owner_id = ?4 AND fence = ?5 AND status = 'owned'",
        )
        .bind(i64::from(pid))
        .bind(millis(now))
        .bind(lease_until(now, ttl)?)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(self.stale_fence("supervisor", token.fence, None).await?)
        }
    }

    pub async fn release_supervisor(
        &self,
        token: &SupervisorFence,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE supervisor_lease SET status = 'released', heartbeat_at_ms = ?1, \
             lease_until_ms = ?1, pid = NULL \
             WHERE singleton = 1 AND owner_id = ?2 AND fence = ?3 AND status = 'owned'",
        )
        .bind(millis(now))
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(self.stale_fence("supervisor", token.fence, None).await?)
        }
    }

    pub async fn acquire_lane(
        &self,
        lane_id: &str,
        owner_id: &str,
        supervisor: &SupervisorFence,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<LaneFence, StoreError> {
        self.ensure_writer()?;
        let now_ms = millis(now);
        let lease_until_ms = lease_until(now, ttl)?;
        let mut transaction = self.pool.begin().await?;
        ensure_supervisor_fence(&mut transaction, supervisor, now_ms).await?;
        let existing = sqlx::query_as::<_, (String, i64, String, i64)>(
            "SELECT owner_id, fence, status, lease_until_ms FROM lane_lease WHERE lane_id = ?1",
        )
        .bind(lane_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let fence = match existing {
            Some((owner, current, status, until)) => {
                if status == "recovery_required" {
                    return Err(StoreError::RecoveryRequired(lane_id.to_owned()));
                }
                if status == "owned" && until > now_ms {
                    return Err(StoreError::LeaseBusy {
                        scope: format!("lane {lane_id}"),
                        owner,
                    });
                }
                current
                    .checked_add(1)
                    .ok_or_else(|| StoreError::FenceExhausted(format!("lane {lane_id}")))?
            }
            None => 1,
        };
        sqlx::query(
            "INSERT INTO lane_lease \
             (lane_id, owner_id, fence, supervisor_fence, status, role, worktree, \
              expected_branch, provider_key, pid, heartbeat_at_ms, lease_until_ms, recovery_reason) \
             VALUES (?1, ?2, ?3, ?4, 'owned', NULL, NULL, NULL, NULL, NULL, ?5, ?6, NULL) \
             ON CONFLICT(lane_id) DO UPDATE SET \
             owner_id = excluded.owner_id, fence = excluded.fence, \
             supervisor_fence = excluded.supervisor_fence, status = 'owned', role = NULL, \
             worktree = NULL, expected_branch = NULL, provider_key = NULL, pid = NULL, \
             heartbeat_at_ms = excluded.heartbeat_at_ms, \
             lease_until_ms = excluded.lease_until_ms, recovery_reason = NULL",
        )
        .bind(lane_id)
        .bind(owner_id)
        .bind(fence)
        .bind(supervisor.fence)
        .bind(now_ms)
        .bind(lease_until_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(LaneFence {
            lane_id: lane_id.to_owned(),
            owner_id: owner_id.to_owned(),
            fence,
            supervisor_fence: supervisor.fence,
        })
    }

    pub async fn update_lane_context(
        &self,
        token: &LaneFence,
        context: &LaneRuntimeContext,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE lane_lease SET role = ?1, worktree = ?2, expected_branch = ?3, \
             provider_key = ?4, pid = ?5, heartbeat_at_ms = ?6 \
             WHERE lane_id = ?7 AND owner_id = ?8 AND fence = ?9 AND status = 'owned' \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(&context.role)
        .bind(context.worktree.to_string_lossy().as_ref())
        .bind(&context.expected_branch)
        .bind(&context.provider_key)
        .bind(context.pid.map(i64::from))
        .bind(millis(now))
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        self.require_lane_update(token, result.rows_affected())
            .await
    }

    pub async fn renew_lane(
        &self,
        token: &LaneFence,
        pid: Option<u32>,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE lane_lease SET pid = ?1, heartbeat_at_ms = ?2, lease_until_ms = ?3 \
             WHERE lane_id = ?4 AND owner_id = ?5 AND fence = ?6 AND status = 'owned' \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(pid.map(i64::from))
        .bind(millis(now))
        .bind(lease_until(now, ttl)?)
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        self.require_lane_update(token, result.rows_affected())
            .await
    }

    pub async fn release_lane(
        &self,
        token: &LaneFence,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE lane_lease SET status = 'released', pid = NULL, heartbeat_at_ms = ?1, \
             lease_until_ms = ?1 \
             WHERE lane_id = ?2 AND owner_id = ?3 AND fence = ?4 AND status = 'owned' \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(millis(now))
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        self.require_lane_update(token, result.rows_affected())
            .await
    }

    pub async fn mark_lane_recovery_required(
        &self,
        token: &LaneFence,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE lane_lease SET status = 'recovery_required', recovery_reason = ?1, \
             pid = NULL, heartbeat_at_ms = ?2, lease_until_ms = ?2 \
             WHERE lane_id = ?3 AND owner_id = ?4 AND fence = ?5 AND status = 'owned' \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(reason)
        .bind(millis(now))
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        self.require_lane_update(token, result.rows_affected())
            .await
    }

    pub async fn clear_lane_recovery(
        &self,
        token: &LaneFence,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let result = sqlx::query(
            "UPDATE lane_lease SET status = 'released', recovery_reason = NULL, \
             heartbeat_at_ms = ?1, lease_until_ms = ?1 \
             WHERE lane_id = ?2 AND owner_id = ?3 AND fence = ?4 \
             AND status = 'recovery_required' \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(millis(now))
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        self.require_lane_update(token, result.rows_affected())
            .await
    }

    pub async fn resolve_lane_recovery(
        &self,
        supervisor: &SupervisorFence,
        lane_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let mut transaction = self.pool.begin().await?;
        ensure_supervisor_fence(&mut transaction, supervisor, millis(now)).await?;
        let result = sqlx::query(
            "UPDATE lane_lease SET status = 'released', recovery_reason = NULL, \
             heartbeat_at_ms = ?1, lease_until_ms = ?1, pid = NULL \
             WHERE lane_id = ?2 AND status = 'recovery_required'",
        )
        .bind(millis(now))
        .bind(lane_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            return Err(StoreError::RecoveryRequired(lane_id.to_owned()));
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn require_lane_update(
        &self,
        token: &LaneFence,
        rows_affected: u64,
    ) -> Result<(), StoreError> {
        if rows_affected == 1 {
            Ok(())
        } else {
            Err(self
                .stale_fence(
                    &format!("lane {}", token.lane_id),
                    token.fence,
                    Some(&token.lane_id),
                )
                .await?)
        }
    }

    async fn stale_fence(
        &self,
        scope: &str,
        expected: i64,
        lane_id: Option<&str>,
    ) -> Result<StoreError, StoreError> {
        let current = if let Some(lane_id) = lane_id {
            sqlx::query_scalar("SELECT fence FROM lane_lease WHERE lane_id = ?1")
                .bind(lane_id)
                .fetch_optional(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT fence FROM supervisor_lease WHERE singleton = 1")
                .fetch_optional(&self.pool)
                .await?
        };
        Ok(StoreError::StaleFence {
            scope: scope.to_owned(),
            expected,
            current,
        })
    }
}

async fn ensure_supervisor_fence(
    transaction: &mut Transaction<'_, Sqlite>,
    token: &SupervisorFence,
    now_ms: i64,
) -> Result<(), StoreError> {
    let valid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM supervisor_lease WHERE singleton = 1 AND owner_id = ?1 \
         AND fence = ?2 AND status = 'owned' AND lease_until_ms > ?3",
    )
    .bind(&token.owner_id)
    .bind(token.fence)
    .bind(now_ms)
    .fetch_one(&mut **transaction)
    .await?;
    if valid == 1 {
        Ok(())
    } else {
        let current = sqlx::query_scalar("SELECT fence FROM supervisor_lease WHERE singleton = 1")
            .fetch_optional(&mut **transaction)
            .await?;
        Err(StoreError::StaleFence {
            scope: "supervisor".to_owned(),
            expected: token.fence,
            current,
        })
    }
}

impl ControlStore {
    pub async fn attempt_spec(&self, attempt_id: &str) -> Result<AttemptSpec, StoreError> {
        let value =
            sqlx::query_scalar::<_, String>("SELECT spec_json FROM attempt WHERE attempt_id = ?1")
                .bind(attempt_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| StoreError::AttemptNotFound(attempt_id.to_owned()))?;
        Ok(serde_json::from_str(&value)?)
    }

    pub async fn reserve_initial_attempt(
        &self,
        spec: &AttemptSpec,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let mut transaction = self.pool.begin().await?;
        let current_fence = sqlx::query_scalar::<_, i64>(
            "SELECT fence FROM lane_lease WHERE lane_id = ?1 AND fence = ?2 \
             AND status = 'owned' AND lease_until_ms > ?3 \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(&spec.lane_id)
        .bind(spec.lane_fence)
        .bind(millis(now))
        .fetch_optional(&mut *transaction)
        .await?;
        if current_fence != Some(spec.lane_fence) {
            let actual = sqlx::query_scalar("SELECT fence FROM lane_lease WHERE lane_id = ?1")
                .bind(&spec.lane_id)
                .fetch_optional(&mut *transaction)
                .await?;
            return Err(StoreError::StaleFence {
                scope: format!("lane {}", spec.lane_id),
                expected: spec.lane_fence,
                current: actual,
            });
        }
        let existing =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM attempt WHERE work_key = ?1")
                .bind(&spec.work_key)
                .fetch_one(&mut *transaction)
                .await?;
        if existing != 0 {
            return Err(StoreError::AttemptBudgetExhausted(spec.work_key.clone()));
        }
        let provider_key = provider_key(spec);
        sqlx::query(
            "INSERT INTO attempt \
             (attempt_id, lane_id, lane_fence, work_key, attempt_ordinal, provider_key, \
              config_fingerprint, preflight_signature, state, spec_json, git_head_before, \
              journal_sha_before, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, 'reserved', ?8, ?9, ?10, ?11)",
        )
        .bind(&spec.id)
        .bind(&spec.lane_id)
        .bind(spec.lane_fence)
        .bind(&spec.work_key)
        .bind(provider_key)
        .bind(&spec.provider.config_fingerprint)
        .bind(&spec.preflight_signature)
        .bind(serde_json::to_string(spec)?)
        .bind(&spec.git_head_before)
        .bind(&spec.journal_sha_before)
        .bind(millis(now))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Reserves the single fresh alternate-provider recovery for a work unit.
    ///
    /// The original attempt must be ordinal one on the same current lane fence;
    /// the recovery must retain its work key and use a different provider. A
    /// second recovery reservation fails closed.
    pub async fn reserve_alternate_attempt(
        &self,
        token: &LaneFence,
        original_attempt_id: &str,
        spec: &AttemptSpec,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let mut transaction = self.pool.begin().await?;
        let now_ms = millis(now);
        let current_fence = sqlx::query_scalar::<_, i64>(
            "SELECT fence FROM lane_lease WHERE lane_id = ?1 AND owner_id = ?2 AND fence = ?3 \
             AND status = 'owned' AND lease_until_ms > ?4 \
             AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                 WHERE singleton = 1 AND status = 'owned')",
        )
        .bind(&token.lane_id)
        .bind(&token.owner_id)
        .bind(token.fence)
        .bind(now_ms)
        .fetch_optional(&mut *transaction)
        .await?;
        if current_fence != Some(token.fence)
            || spec.lane_id != token.lane_id
            || spec.lane_fence != token.fence
        {
            let actual = sqlx::query_scalar("SELECT fence FROM lane_lease WHERE lane_id = ?1")
                .bind(&token.lane_id)
                .fetch_optional(&mut *transaction)
                .await?;
            return Err(StoreError::StaleFence {
                scope: format!("lane {}", token.lane_id),
                expected: token.fence,
                current: actual,
            });
        }
        let original = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT work_key, provider_key, attempt_ordinal FROM attempt \
             WHERE attempt_id = ?1 AND lane_id = ?2 AND lane_fence = ?3",
        )
        .bind(original_attempt_id)
        .bind(&token.lane_id)
        .bind(token.fence)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| StoreError::AttemptNotFound(original_attempt_id.to_owned()))?;
        let recovery_provider = provider_key(spec);
        if original.2 != 1
            || original.0 != spec.work_key
            || original.1 == recovery_provider
            || spec.prompt_kind != crate::model::PromptKind::Recovery
            || spec.resume_session_id.is_some()
        {
            return Err(StoreError::InvalidRecoveryAttempt(spec.id.clone()));
        }
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attempt WHERE work_key = ?1 AND attempt_ordinal >= 2",
        )
        .bind(&spec.work_key)
        .fetch_one(&mut *transaction)
        .await?;
        if existing != 0 {
            return Err(StoreError::AttemptBudgetExhausted(spec.work_key.clone()));
        }
        sqlx::query(
            "INSERT INTO attempt \
             (attempt_id, lane_id, lane_fence, work_key, attempt_ordinal, provider_key, \
              config_fingerprint, preflight_signature, state, spec_json, git_head_before, \
              journal_sha_before, created_at_ms) \
             VALUES (?1, ?2, ?3, ?4, 2, ?5, ?6, ?7, 'reserved', ?8, ?9, ?10, ?11)",
        )
        .bind(&spec.id)
        .bind(&spec.lane_id)
        .bind(spec.lane_fence)
        .bind(&spec.work_key)
        .bind(recovery_provider)
        .bind(&spec.provider.config_fingerprint)
        .bind(&spec.preflight_signature)
        .bind(serde_json::to_string(spec)?)
        .bind(&spec.git_head_before)
        .bind(&spec.journal_sha_before)
        .bind(now_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn start_attempt(
        &self,
        token: &LaneFence,
        attempt_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        self.ensure_current_lane(token).await?;
        let result = sqlx::query(
            "UPDATE attempt SET state = 'running', started_at_ms = ?1 \
             WHERE attempt_id = ?2 AND lane_id = ?3 AND lane_fence = ?4 AND state = 'reserved'",
        )
        .bind(millis(now))
        .bind(attempt_id)
        .bind(&token.lane_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(StoreError::AttemptNotFound(attempt_id.to_owned()))
        }
    }

    pub async fn finish_attempt(
        &self,
        token: &LaneFence,
        attempt_id: &str,
        result: &NormalizedResult,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        self.ensure_current_lane(token).await?;
        let state = if result.class == ResultClass::Completed {
            "completed"
        } else if result.class == ResultClass::PostconditionFailed {
            "recovery_required"
        } else {
            "failed"
        };
        let update = sqlx::query(
            "UPDATE attempt SET state = ?1, result_class = ?2, session_id = ?3, exit_code = ?4, \
             structured_started = ?5, terminal_type = ?6, stable_error_hash = ?7, finished_at_ms = ?8 \
             WHERE attempt_id = ?9 AND lane_id = ?10 AND lane_fence = ?11 \
             AND state IN ('reserved', 'running')",
        )
        .bind(state)
        .bind(result.class.as_str())
        .bind(&result.session_id)
        .bind(result.exit_code)
        .bind(i64::from(result.structured_started))
        .bind(&result.terminal_type)
        .bind(&result.stable_error_hash)
        .bind(millis(now))
        .bind(attempt_id)
        .bind(&token.lane_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        if update.rows_affected() == 1 {
            Ok(())
        } else {
            Err(StoreError::AttemptNotFound(attempt_id.to_owned()))
        }
    }

    pub async fn append_checkpoint(
        &self,
        token: &LaneFence,
        attempt_id: &str,
        sequence: i64,
        state: &str,
        payload: &serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        self.ensure_current_lane(token).await?;
        sqlx::query(
            "INSERT INTO attempt_checkpoint \
             (attempt_id, sequence, state, payload_json, created_at_ms) \
             SELECT ?1, ?2, ?3, ?4, ?5 FROM attempt \
             WHERE attempt_id = ?1 AND lane_id = ?6 AND lane_fence = ?7",
        )
        .bind(attempt_id)
        .bind(sequence)
        .bind(state)
        .bind(serde_json::to_string(payload)?)
        .bind(millis(now))
        .bind(&token.lane_id)
        .bind(token.fence)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn attempt_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM attempt")
            .fetch_one(&self.pool)
            .await?)
    }

    async fn ensure_current_lane(&self, token: &LaneFence) -> Result<(), StoreError> {
        let current = sqlx::query_as::<_, (i64, String, String, i64, Option<i64>)>(
            "SELECT lane_lease.fence, lane_lease.owner_id, lane_lease.status, \
             lane_lease.supervisor_fence, supervisor_lease.fence \
             FROM lane_lease LEFT JOIN supervisor_lease ON supervisor_lease.singleton = 1 \
             AND supervisor_lease.status = 'owned' WHERE lane_lease.lane_id = ?1",
        )
        .bind(&token.lane_id)
        .fetch_optional(&self.pool)
        .await?;
        if current.as_ref().is_some_and(
            |(fence, owner, status, supervisor_fence, current_supervisor)| {
                *fence == token.fence
                    && owner == &token.owner_id
                    && status == "owned"
                    && *supervisor_fence == token.supervisor_fence
                    && *current_supervisor == Some(token.supervisor_fence)
            },
        ) {
            Ok(())
        } else {
            Err(StoreError::StaleFence {
                scope: format!("lane {}", token.lane_id),
                expected: token.fence,
                current: current.map(|(fence, _, _, _, _)| fence),
            })
        }
    }
}

fn provider_key(spec: &AttemptSpec) -> String {
    spec.provider.model.as_ref().map_or_else(
        || spec.provider.provider.to_string(),
        |model| format!("{}/{model}", spec.provider.provider),
    )
}

impl ControlStore {
    pub async fn open_provider_circuit(
        &self,
        provider_key: &str,
        config_fingerprint: &str,
        reason: &str,
        retry_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        upsert_provider_circuit(
            &self.pool,
            provider_key,
            "open",
            config_fingerprint,
            reason,
            retry_at.map(millis),
            millis(now),
        )
        .await
    }

    pub async fn disable_provider_until_fingerprint_change(
        &self,
        provider_key: &str,
        config_fingerprint: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        upsert_provider_circuit(
            &self.pool,
            provider_key,
            "disabled",
            config_fingerprint,
            reason,
            None,
            millis(now),
        )
        .await
    }

    pub async fn close_provider_circuit(
        &self,
        provider_key: &str,
        config_fingerprint: &str,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT INTO provider_circuit \
             (provider_key, state, config_fingerprint, consecutive_failures) \
             VALUES (?1, 'closed', ?2, 0) \
             ON CONFLICT(provider_key) DO UPDATE SET state = 'closed', \
             config_fingerprint = excluded.config_fingerprint, reason = NULL, opened_at_ms = NULL, \
             retry_at_ms = NULL, half_open_owner = NULL, half_open_until_ms = NULL, \
             consecutive_failures = 0",
        )
        .bind(provider_key)
        .bind(config_fingerprint)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn provider_gate(
        &self,
        provider_key: &str,
        config_fingerprint: &str,
        now: DateTime<Utc>,
    ) -> Result<ProviderGate, StoreError> {
        let row = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>)>(
            "SELECT state, config_fingerprint, retry_at_ms, half_open_until_ms \
             FROM provider_circuit WHERE provider_key = ?1",
        )
        .bind(provider_key)
        .fetch_optional(&self.pool)
        .await?;
        let Some((state, stored_fingerprint, retry_at, half_open_until)) = row else {
            return Ok(ProviderGate::Closed);
        };
        if state == "disabled" && stored_fingerprint != config_fingerprint {
            if self.mode == StoreMode::Writer {
                self.close_provider_circuit(provider_key, config_fingerprint)
                    .await?;
            }
            return Ok(ProviderGate::Closed);
        }
        let now_ms = millis(now);
        match state.as_str() {
            "closed" => Ok(ProviderGate::Closed),
            "disabled" => Ok(ProviderGate::DisabledUntilFingerprintChanges),
            "open" => match retry_at {
                Some(retry_at) if retry_at <= now_ms => Ok(ProviderGate::HalfOpenAvailable),
                Some(retry_at) => Ok(ProviderGate::Open {
                    retry_at: Some(datetime(retry_at)?),
                }),
                None => Ok(ProviderGate::Open { retry_at: None }),
            },
            "half_open" => match half_open_until {
                Some(until) if until > now_ms => Ok(ProviderGate::HalfOpenOwned {
                    until: datetime(until)?,
                }),
                _ => Ok(ProviderGate::HalfOpenAvailable),
            },
            invalid => Err(StoreError::Integrity(format!(
                "invalid provider circuit state {invalid:?}"
            ))),
        }
    }

    pub async fn try_acquire_half_open(
        &self,
        provider_key: &str,
        owner_id: &str,
        config_fingerprint: &str,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Result<bool, StoreError> {
        self.ensure_writer()?;
        let now_ms = millis(now);
        let until_ms = lease_until(now, ttl)?;
        let result = sqlx::query(
            "UPDATE provider_circuit SET state = 'half_open', half_open_owner = ?1, \
             half_open_until_ms = ?2 \
             WHERE provider_key = ?3 AND config_fingerprint = ?4 AND ( \
               (state = 'open' AND retry_at_ms IS NOT NULL AND retry_at_ms <= ?5) OR \
               (state = 'half_open' AND (half_open_until_ms IS NULL OR half_open_until_ms <= ?5)) \
             )",
        )
        .bind(owner_id)
        .bind(until_ms)
        .bind(provider_key)
        .bind(config_fingerprint)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn fingerprint_gate(
        &self,
        fingerprint: &str,
        now: DateTime<Utc>,
    ) -> Result<FingerprintGate, StoreError> {
        let open_until = sqlx::query_scalar::<_, i64>(
            "SELECT open_until_ms FROM failure_bucket \
             WHERE fingerprint = ?1 AND open_until_ms > ?2",
        )
        .bind(fingerprint)
        .bind(millis(now))
        .fetch_optional(&self.pool)
        .await?;
        open_until.map_or(Ok(FingerprintGate::Closed), |retry_at| {
            Ok(FingerprintGate::Open {
                retry_at: datetime(retry_at)?,
            })
        })
    }

    pub async fn system_gate(&self, now: DateTime<Utc>) -> Result<SystemGate, StoreError> {
        let row = sqlx::query_as::<_, (String, Option<i64>, Option<i64>)>(
            "SELECT state, retry_at_ms, diagnostics_passed_at_ms \
             FROM system_circuit WHERE singleton = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        if row.0 == "closed" {
            return Ok(SystemGate::Closed);
        }
        let retry_at_ms = row.1.ok_or_else(|| {
            StoreError::Integrity("paused system circuit has no retry time".to_owned())
        })?;
        let diagnostics_ready = row.2.is_some_and(|passed_at| passed_at >= retry_at_ms);
        if millis(now) >= retry_at_ms && diagnostics_ready {
            if self.mode == StoreMode::Writer {
                sqlx::query(
                    "UPDATE system_circuit SET state = 'closed', reason = NULL, \
                     opened_at_ms = NULL, retry_at_ms = NULL WHERE singleton = 1",
                )
                .execute(&self.pool)
                .await?;
            }
            Ok(SystemGate::Closed)
        } else {
            Ok(SystemGate::Paused {
                retry_at: datetime(retry_at_ms)?,
                diagnostics_required: true,
            })
        }
    }

    pub async fn mark_system_diagnostics_passed(
        &self,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query("UPDATE system_circuit SET diagnostics_passed_at_ms = ?1 WHERE singleton = 1")
            .bind(millis(now))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn upsert_provider_circuit<'e, E>(
    executor: E,
    provider_key: &str,
    state: &str,
    config_fingerprint: &str,
    reason: &str,
    retry_at_ms: Option<i64>,
    now_ms: i64,
) -> Result<(), StoreError>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO provider_circuit \
         (provider_key, state, config_fingerprint, reason, opened_at_ms, retry_at_ms, \
          last_failure_at_ms, half_open_owner, half_open_until_ms, consecutive_failures) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, NULL, NULL, 1) \
         ON CONFLICT(provider_key) DO UPDATE SET state = excluded.state, \
         config_fingerprint = excluded.config_fingerprint, reason = excluded.reason, \
         opened_at_ms = excluded.opened_at_ms, retry_at_ms = excluded.retry_at_ms, \
         last_failure_at_ms = excluded.last_failure_at_ms, half_open_owner = NULL, \
         half_open_until_ms = NULL, consecutive_failures = provider_circuit.consecutive_failures + 1",
    )
    .bind(provider_key)
    .bind(state)
    .bind(config_fingerprint)
    .bind(reason)
    .bind(now_ms)
    .bind(retry_at_ms)
    .execute(executor)
    .await?;
    Ok(())
}

impl ControlStore {
    #[allow(clippy::too_many_lines)]
    pub async fn record_failure(
        &self,
        observation: FailureObservation<'_>,
    ) -> Result<FailureBucket, StoreError> {
        self.ensure_writer()?;
        if !observation.result.class.is_failure() {
            return Err(StoreError::Integrity(
                "completed result cannot be recorded as a failure".to_owned(),
            ));
        }
        let now_ms = millis(observation.occurred_at);
        let mut transaction = self.pool.begin().await?;
        let attempt = sqlx::query_as::<_, (String, i64, i64, String, i64, Option<i64>)>(
            "SELECT attempt.lane_id, attempt.lane_fence, lane_lease.fence, lane_lease.status, \
             lane_lease.supervisor_fence, supervisor_lease.fence \
             FROM attempt JOIN lane_lease ON lane_lease.lane_id = attempt.lane_id \
             LEFT JOIN supervisor_lease ON supervisor_lease.singleton = 1 \
             AND supervisor_lease.status = 'owned' \
             WHERE attempt.attempt_id = ?1",
        )
        .bind(observation.attempt_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((
            lane_id,
            attempt_fence,
            current_fence,
            lane_status,
            lane_supervisor_fence,
            current_supervisor_fence,
        )) = attempt
        else {
            return Err(StoreError::AttemptNotFound(
                observation.attempt_id.to_owned(),
            ));
        };
        if attempt_fence != current_fence
            || lane_status != "owned"
            || current_supervisor_fence != Some(lane_supervisor_fence)
        {
            return Err(StoreError::StaleFence {
                scope: format!("lane {lane_id}"),
                expected: attempt_fence,
                current: Some(current_fence),
            });
        }

        let attempt_state = if matches!(&observation.decision.action, RetryAction::Recover) {
            "recovery_required"
        } else {
            "failed"
        };
        sqlx::query(
            "UPDATE attempt SET state = ?1, result_class = ?2, session_id = ?3, exit_code = ?4, \
             structured_started = ?5, terminal_type = ?6, stable_error_hash = ?7, \
             failure_fingerprint = ?8, exemplar_ref = ?9, finished_at_ms = ?10 \
             WHERE attempt_id = ?11",
        )
        .bind(attempt_state)
        .bind(observation.result.class.as_str())
        .bind(&observation.result.session_id)
        .bind(observation.result.exit_code)
        .bind(i64::from(observation.result.structured_started))
        .bind(&observation.result.terminal_type)
        .bind(&observation.result.stable_error_hash)
        .bind(observation.fingerprint)
        .bind(observation.exemplar_ref)
        .bind(now_ms)
        .bind(observation.attempt_id)
        .execute(&mut *transaction)
        .await?;

        let bucket = record_failure_bucket(&mut transaction, &observation).await?;
        match &observation.decision.action {
            RetryAction::RetryAt(retry_at) => {
                upsert_provider_circuit(
                    &mut *transaction,
                    observation.provider_key,
                    "open",
                    observation.config_fingerprint,
                    observation.result.class.as_str(),
                    Some(millis(*retry_at)),
                    now_ms,
                )
                .await?;
            }
            RetryAction::UntilFingerprintChanges => {
                upsert_provider_circuit(
                    &mut *transaction,
                    observation.provider_key,
                    "disabled",
                    observation.config_fingerprint,
                    observation.result.class.as_str(),
                    None,
                    now_ms,
                )
                .await?;
            }
            RetryAction::Recover => {
                sqlx::query(
                    "UPDATE lane_lease SET status = 'recovery_required', \
                     recovery_reason = ?1, pid = NULL, heartbeat_at_ms = ?2, lease_until_ms = ?2 \
                     WHERE lane_id = ?3 AND fence = ?4 AND status = 'owned' \
                     AND supervisor_fence = (SELECT fence FROM supervisor_lease \
                         WHERE singleton = 1 AND status = 'owned')",
                )
                .bind(observation.result.class.as_str())
                .bind(now_ms)
                .bind(&lane_id)
                .bind(attempt_fence)
                .execute(&mut *transaction)
                .await?;
            }
            RetryAction::Complete | RetryAction::Reconcile | RetryAction::Never => {}
        }
        record_launch_failure_tx(
            &mut transaction,
            observation.provider_key,
            observation.fingerprint,
            observation.config_fingerprint,
            observation.occurred_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(bucket)
    }

    pub async fn record_launch_failure(
        &self,
        provider_key: &str,
        fingerprint: &str,
        occurred_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let mut transaction = self.pool.begin().await?;
        record_launch_failure_tx(&mut transaction, provider_key, fingerprint, "", occurred_at)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn record_suppression(
        &self,
        reason: SuppressionReason,
        provider_key: &str,
        fingerprint: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        let fingerprint = fingerprint.unwrap_or("");
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO suppression_counter \
             (reason, provider_key, fingerprint, count, first_seen_at_ms, last_seen_at_ms, retry_at_ms) \
             VALUES (?1, ?2, ?3, 1, ?4, ?4, ?5) \
             ON CONFLICT(reason, provider_key, fingerprint) DO UPDATE SET \
             count = suppression_counter.count + 1, last_seen_at_ms = excluded.last_seen_at_ms, \
             retry_at_ms = excluded.retry_at_ms",
        )
        .bind(suppression_reason(reason))
        .bind(provider_key)
        .bind(fingerprint)
        .bind(millis(now))
        .bind(retry_at.map(millis))
        .execute(&mut *transaction)
        .await?;
        if !fingerprint.is_empty() {
            sqlx::query(
                "UPDATE failure_bucket SET suppressed_count = suppressed_count + 1, \
                 last_seen_at_ms = ?1 WHERE fingerprint = ?2",
            )
            .bind(millis(now))
            .bind(fingerprint)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn failure_bucket(
        &self,
        fingerprint: &str,
    ) -> Result<Option<FailureBucket>, StoreError> {
        let row = sqlx::query_as::<_, (String, String, i64, i64, Option<i64>, Option<String>)>(
            "SELECT fingerprint, provider_key, failure_count, suppressed_count, \
             open_until_ms, exemplar_ref FROM failure_bucket WHERE fingerprint = ?1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(FailureBucket {
                fingerprint: row.0,
                provider_key: row.1,
                failure_count: row.2,
                suppressed_count: row.3,
                open_until: row.4.map(datetime).transpose()?,
                exemplar_ref: row.5,
            })
        })
        .transpose()
    }

    pub async fn suppression_row_count(&self) -> Result<i64, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM suppression_counter")
                .fetch_one(&self.pool)
                .await?,
        )
    }
}

async fn record_failure_bucket(
    transaction: &mut Transaction<'_, Sqlite>,
    observation: &FailureObservation<'_>,
) -> Result<FailureBucket, StoreError> {
    let thresholds = StormThresholds::default();
    let now_ms = millis(observation.occurred_at);
    let window_ms = thresholds.window.num_milliseconds();
    let existing = sqlx::query_as::<_, (i64, i64, i64, Option<i64>, Option<String>)>(
        "SELECT window_started_at_ms, last_seen_at_ms, failure_count, open_until_ms, exemplar_ref \
         FROM failure_bucket WHERE fingerprint = ?1",
    )
    .bind(observation.fingerprint)
    .fetch_optional(&mut **transaction)
    .await?;
    let (window_started_at, failure_count, previous_open, exemplar_ref) = match existing {
        Some((window_started, _, count, open_until, exemplar))
            if now_ms.saturating_sub(window_started) <= window_ms =>
        {
            (
                window_started,
                count.saturating_add(1),
                open_until,
                exemplar.or_else(|| observation.exemplar_ref.map(str::to_owned)),
            )
        }
        Some((_, _, _, open_until, exemplar)) => (
            now_ms,
            1,
            open_until,
            observation.exemplar_ref.map(str::to_owned).or(exemplar),
        ),
        None => (now_ms, 1, None, observation.exemplar_ref.map(str::to_owned)),
    };
    let threshold_open = (failure_count >= thresholds.fingerprint_failures)
        .then(|| millis(observation.occurred_at + thresholds.fingerprint_cooldown));
    let open_until = match (previous_open, threshold_open) {
        (Some(previous), Some(current)) => Some(previous.max(current)),
        (previous, current) => previous.or(current),
    };
    sqlx::query(
        "INSERT INTO failure_bucket \
         (fingerprint, provider_key, window_started_at_ms, last_seen_at_ms, failure_count, \
          suppressed_count, open_until_ms, exemplar_ref) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7) \
         ON CONFLICT(fingerprint) DO UPDATE SET provider_key = excluded.provider_key, \
         window_started_at_ms = excluded.window_started_at_ms, \
         last_seen_at_ms = excluded.last_seen_at_ms, failure_count = excluded.failure_count, \
         open_until_ms = excluded.open_until_ms, exemplar_ref = excluded.exemplar_ref",
    )
    .bind(observation.fingerprint)
    .bind(observation.provider_key)
    .bind(window_started_at)
    .bind(now_ms)
    .bind(failure_count)
    .bind(open_until)
    .bind(&exemplar_ref)
    .execute(&mut **transaction)
    .await?;
    let suppressed_count = sqlx::query_scalar::<_, i64>(
        "SELECT suppressed_count FROM failure_bucket WHERE fingerprint = ?1",
    )
    .bind(observation.fingerprint)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(FailureBucket {
        fingerprint: observation.fingerprint.to_owned(),
        provider_key: observation.provider_key.to_owned(),
        failure_count,
        suppressed_count,
        open_until: open_until.map(datetime).transpose()?,
        exemplar_ref,
    })
}

async fn record_launch_failure_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    provider_key: &str,
    fingerprint: &str,
    config_fingerprint: &str,
    occurred_at: DateTime<Utc>,
) -> Result<(), StoreError> {
    let thresholds = StormThresholds::default();
    let now_ms = millis(occurred_at);
    let window_start = millis(occurred_at - thresholds.window);
    sqlx::query("DELETE FROM launch_failure WHERE occurred_at_ms < ?1")
        .bind(window_start)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(
        "INSERT INTO launch_failure (provider_key, fingerprint, occurred_at_ms) VALUES (?1, ?2, ?3)",
    )
    .bind(provider_key)
    .bind(fingerprint)
    .bind(now_ms)
    .execute(&mut **transaction)
    .await?;
    let provider_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM launch_failure WHERE provider_key = ?1 AND occurred_at_ms >= ?2",
    )
    .bind(provider_key)
    .bind(window_start)
    .fetch_one(&mut **transaction)
    .await?;
    if provider_count >= thresholds.provider_failures {
        let existing = sqlx::query_as::<_, (String, String, Option<i64>)>(
            "SELECT state, config_fingerprint, retry_at_ms FROM provider_circuit \
             WHERE provider_key = ?1",
        )
        .bind(provider_key)
        .fetch_optional(&mut **transaction)
        .await?;
        if existing.as_ref().is_none_or(|row| row.0 != "disabled") {
            let stored_fingerprint = existing
                .as_ref()
                .map_or_else(|| config_fingerprint.to_owned(), |row| row.1.clone());
            let storm_retry = millis(occurred_at + thresholds.provider_cooldown);
            let retry_at = existing
                .as_ref()
                .and_then(|row| row.2)
                .map_or(storm_retry, |current| current.max(storm_retry));
            upsert_provider_circuit(
                &mut **transaction,
                provider_key,
                "open",
                &stored_fingerprint,
                "provider_storm",
                Some(retry_at),
                now_ms,
            )
            .await?;
        }
    }
    let system_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM launch_failure WHERE occurred_at_ms >= ?1",
    )
    .bind(window_start)
    .fetch_one(&mut **transaction)
    .await?;
    if system_count >= thresholds.system_failures {
        let retry_at = millis(occurred_at + thresholds.quiet_period);
        sqlx::query(
            "UPDATE system_circuit SET state = 'paused', reason = 'system_storm', \
             opened_at_ms = COALESCE(opened_at_ms, ?1), retry_at_ms = ?2, \
             last_failure_at_ms = ?1, diagnostics_passed_at_ms = NULL WHERE singleton = 1",
        )
        .bind(now_ms)
        .bind(retry_at)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

fn suppression_reason(reason: SuppressionReason) -> &'static str {
    match reason {
        SuppressionReason::ProviderCircuit => "provider_circuit",
        SuppressionReason::FailureFingerprint => "failure_fingerprint",
        SuppressionReason::SystemPause => "system_pause",
        SuppressionReason::PreflightWait => "preflight_wait",
        SuppressionReason::RecoveryRequired => "recovery_required",
        SuppressionReason::AttemptBudget => "attempt_budget",
    }
}

impl ControlStore {
    pub async fn put_probe(&self, entry: ProbeCacheEntry) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT INTO probe_cache \
             (probe_key, input_fingerprint, outcome, details_json, checked_at_ms, valid_until_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(probe_key) DO UPDATE SET \
             input_fingerprint = excluded.input_fingerprint, outcome = excluded.outcome, \
             details_json = excluded.details_json, checked_at_ms = excluded.checked_at_ms, \
             valid_until_ms = excluded.valid_until_ms",
        )
        .bind(entry.probe_key)
        .bind(entry.input_fingerprint)
        .bind(entry.outcome)
        .bind(entry.details_json)
        .bind(millis(entry.checked_at))
        .bind(millis(entry.valid_until))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn valid_probe(
        &self,
        probe_key: &str,
        input_fingerprint: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<ProbeCacheEntry>, StoreError> {
        let row = sqlx::query_as::<_, (String, String, String, String, i64, i64)>(
            "SELECT probe_key, input_fingerprint, outcome, details_json, checked_at_ms, valid_until_ms \
             FROM probe_cache WHERE probe_key = ?1 AND input_fingerprint = ?2 \
             AND valid_until_ms > ?3",
        )
        .bind(probe_key)
        .bind(input_fingerprint)
        .bind(millis(now))
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(ProbeCacheEntry {
                probe_key: row.0,
                input_fingerprint: row.1,
                outcome: row.2,
                details_json: row.3,
                checked_at: datetime(row.4)?,
                valid_until: datetime(row.5)?,
            })
        })
        .transpose()
    }

    pub async fn upsert_compatibility(
        &self,
        record: &CompatibilityRecord,
    ) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT INTO provider_compatibility \
             (provider_key, cli_version, model, config_fingerprint, compatibility_kind, \
              proven, proof_ref, checked_at_ms, valid_until_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(provider_key, cli_version, model, config_fingerprint, compatibility_kind) \
             DO UPDATE SET proven = excluded.proven, proof_ref = excluded.proof_ref, \
             checked_at_ms = excluded.checked_at_ms, valid_until_ms = excluded.valid_until_ms",
        )
        .bind(&record.provider_key)
        .bind(&record.cli_version)
        .bind(&record.model)
        .bind(&record.config_fingerprint)
        .bind(&record.compatibility_kind)
        .bind(i64::from(record.proven))
        .bind(&record.proof_ref)
        .bind(millis(record.checked_at))
        .bind(record.valid_until.map(millis))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn compatibility(
        &self,
        provider_key: &str,
        cli_version: &str,
        model: &str,
        config_fingerprint: &str,
        compatibility_kind: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<CompatibilityRecord>, StoreError> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                i64,
                Option<String>,
                i64,
                Option<i64>,
            ),
        >(
            "SELECT provider_key, cli_version, model, config_fingerprint, compatibility_kind, \
             proven, proof_ref, checked_at_ms, valid_until_ms FROM provider_compatibility \
             WHERE provider_key = ?1 AND cli_version = ?2 AND model = ?3 \
             AND config_fingerprint = ?4 AND compatibility_kind = ?5 \
             AND (valid_until_ms IS NULL OR valid_until_ms > ?6)",
        )
        .bind(provider_key)
        .bind(cli_version)
        .bind(model)
        .bind(config_fingerprint)
        .bind(compatibility_kind)
        .bind(millis(now))
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(CompatibilityRecord {
                provider_key: row.0,
                cli_version: row.1,
                model: row.2,
                config_fingerprint: row.3,
                compatibility_kind: row.4,
                proven: row.5 != 0,
                proof_ref: row.6,
                checked_at: datetime(row.7)?,
                valid_until: row.8.map(datetime).transpose()?,
            })
        })
        .transpose()
    }

    pub async fn upsert_receipt_mirror(&self, mirror: &ReceiptMirror) -> Result<(), StoreError> {
        self.ensure_writer()?;
        sqlx::query(
            "INSERT INTO integration_mirror \
             (receipt_id, state, branch, pull_request, candidate_sha, merge_sha, last_error, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(receipt_id) DO UPDATE SET state = excluded.state, branch = excluded.branch, \
             pull_request = excluded.pull_request, candidate_sha = excluded.candidate_sha, \
             merge_sha = excluded.merge_sha, last_error = excluded.last_error, \
             updated_at_ms = excluded.updated_at_ms",
        )
        .bind(&mirror.receipt_id)
        .bind(&mirror.state)
        .bind(&mirror.branch)
        .bind(mirror.pull_request)
        .bind(&mirror.candidate_sha)
        .bind(&mirror.merge_sha)
        .bind(&mirror.last_error)
        .bind(millis(mirror.updated_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn receipt_mirrors(&self) -> Result<Vec<ReceiptMirror>, StoreError> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                Option<i64>,
                Option<String>,
                Option<String>,
                Option<String>,
                i64,
            ),
        >(
            "SELECT receipt_id, state, branch, pull_request, candidate_sha, merge_sha, \
             last_error, updated_at_ms FROM integration_mirror ORDER BY updated_at_ms, receipt_id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(ReceiptMirror {
                    receipt_id: row.0,
                    state: row.1,
                    branch: row.2,
                    pull_request: row.3,
                    candidate_sha: row.4,
                    merge_sha: row.5,
                    last_error: row.6,
                    updated_at: datetime(row.7)?,
                })
            })
            .collect()
    }
}

fn writer_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .foreign_keys(true)
        .busy_timeout(BUSY_TIMEOUT)
}

fn create_parent(path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn writer_lock_path(database_path: &Path) -> PathBuf {
    let mut path = OsString::from(database_path.as_os_str());
    path.push(".writer.lock");
    PathBuf::from(path)
}

fn temporary_backup_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .map_or_else(|| OsString::from("control.sqlite3"), OsString::from);
    name.push(format!(".{}.tmp", Ulid::new()));
    destination.with_file_name(name)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn sync_parent(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

fn millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}

fn datetime(value: i64) -> Result<DateTime<Utc>, StoreError> {
    DateTime::from_timestamp_millis(value).ok_or(StoreError::InvalidTimestamp(value))
}

fn lease_until(now: DateTime<Utc>, ttl: Duration) -> Result<i64, StoreError> {
    if ttl <= Duration::zero() {
        return Err(StoreError::InvalidLeaseDuration);
    }
    Ok(millis(now + ttl))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::{Duration, TimeZone, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::model::{
        AttemptSpec, PromptKind, Provider, ProviderIdentity, ResultClass, SuppressionReason,
    };
    use crate::policy::{PolicyEngine, SystemClock};

    fn at(minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 12, minute, 0)
            .single()
            .expect("test timestamp should be valid")
    }

    async fn store(temp: &TempDir) -> ControlStore {
        ControlStore::open_writer(temp.path().join("control.sqlite3"))
            .await
            .expect("test control store should open")
    }

    fn attempt(lane: &LaneFence, id: &str, work_key: &str, root: &Path) -> AttemptSpec {
        AttemptSpec {
            id: id.to_owned(),
            lane_id: lane.lane_id.clone(),
            lane_fence: lane.fence,
            work_key: work_key.to_owned(),
            worktree: root.to_path_buf(),
            expected_branch: "goal/test".to_owned(),
            prompt: "fixture".to_owned(),
            required_root_receipt: None,
            required_root_reads: Vec::new(),
            prompt_kind: PromptKind::Normal,
            provider: ProviderIdentity {
                provider: Provider::Claude,
                executable: "claude".into(),
                cli_version: "1.2.3".to_owned(),
                model: Some("fixture".to_owned()),
                config_fingerprint: "config-a".to_owned(),
            },
            resume_session_id: None,
            preflight_signature: "preflight".to_owned(),
            git_head_before: "012345".to_owned(),
            journal_sha_before: "abcdef".to_owned(),
        }
    }

    async fn owned_lane(store: &ControlStore) -> (SupervisorFence, LaneFence) {
        let supervisor = store
            .acquire_supervisor("supervisor-a", at(0), Duration::minutes(1))
            .await
            .expect("supervisor lease should be acquired");
        let lane = store
            .acquire_lane(
                "lane-0",
                "lane-owner-a",
                &supervisor,
                at(0),
                Duration::minutes(1),
            )
            .await
            .expect("lane lease should be acquired");
        (supervisor, lane)
    }

    #[tokio::test]
    async fn writer_should_configure_durable_sqlite_and_reject_duplicate_os_owner() {
        let temp = TempDir::new().expect("temp directory should be created");
        let path = temp.path().join("control.sqlite3");
        let writer = ControlStore::open_writer(&path)
            .await
            .expect("first writer should open");

        assert_eq!(writer.pragma_i64("synchronous").await.expect("pragma"), 2);
        assert_eq!(writer.pragma_i64("foreign_keys").await.expect("pragma"), 1);
        assert_eq!(
            writer.pragma_i64("busy_timeout").await.expect("pragma"),
            5_000
        );
        assert_eq!(
            writer.pragma_text("journal_mode").await.expect("pragma"),
            "wal"
        );
        assert!(writer.integrity_check().await.is_ok());
        assert!(matches!(
            ControlStore::open_writer(&path).await,
            Err(StoreError::WriterAlreadyRunning(_))
        ));

        let monitor = ControlStore::open_monitor(&path)
            .await
            .expect("monitor should open without taking writer lock");
        assert!(monitor.integrity_check().await.is_ok());
    }

    #[tokio::test]
    async fn lane_fence_should_increase_and_reject_stale_writes() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (supervisor, first) = owned_lane(&store).await;
        store
            .release_lane(&first, at(0))
            .await
            .expect("first owner should release");
        let second = store
            .acquire_lane(
                "lane-0",
                "lane-owner-b",
                &supervisor,
                at(0),
                Duration::minutes(1),
            )
            .await
            .expect("second owner should acquire");

        assert!(second.fence > first.fence);
        assert!(matches!(
            store
                .reserve_initial_attempt(
                    &attempt(&first, "stale", "work-stale", temp.path()),
                    at(0)
                )
                .await,
            Err(StoreError::StaleFence { .. })
        ));
        store
            .reserve_initial_attempt(
                &attempt(&second, "current", "work-current", temp.path()),
                at(0),
            )
            .await
            .expect("current fence should reserve attempt");
    }

    #[tokio::test]
    async fn failover_reservation_allows_one_fresh_alternate_on_same_fence() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (_supervisor, lane) = owned_lane(&store).await;
        let initial = attempt(&lane, "initial", "work-failover", temp.path());
        store
            .reserve_initial_attempt(&initial, at(0))
            .await
            .expect("initial attempt should reserve");
        let mut alternate = attempt(&lane, "alternate", "work-failover", temp.path());
        alternate.provider.provider = Provider::Codex;
        alternate.provider.executable = "codex".into();
        alternate.provider.config_fingerprint = "config-codex".to_owned();
        alternate.prompt_kind = PromptKind::Recovery;
        store
            .reserve_alternate_attempt(&lane, "initial", &alternate, at(0))
            .await
            .expect("one fresh alternate should reserve");

        let mut third = alternate.clone();
        third.id = "third".to_owned();
        assert!(matches!(
            store
                .reserve_alternate_attempt(&lane, "initial", &third, at(0))
                .await,
            Err(StoreError::AttemptBudgetExhausted(_))
        ));
    }

    #[tokio::test]
    async fn failover_reservation_rejects_resume_same_provider_and_stale_fence() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (supervisor, lane) = owned_lane(&store).await;
        let initial = attempt(&lane, "initial", "work-failover", temp.path());
        store
            .reserve_initial_attempt(&initial, at(0))
            .await
            .expect("initial attempt should reserve");
        let mut invalid = attempt(&lane, "invalid", "work-failover", temp.path());
        invalid.prompt_kind = PromptKind::Recovery;
        invalid.resume_session_id = Some("forbidden-resume".to_owned());
        assert!(matches!(
            store
                .reserve_alternate_attempt(&lane, "initial", &invalid, at(0))
                .await,
            Err(StoreError::InvalidRecoveryAttempt(_))
        ));

        store
            .release_lane(&lane, at(0))
            .await
            .expect("lane should release");
        let newer = store
            .acquire_lane(
                "lane-0",
                "lane-owner-b",
                &supervisor,
                at(0),
                Duration::minutes(1),
            )
            .await
            .expect("lane should reacquire");
        invalid.resume_session_id = None;
        invalid.provider.provider = Provider::Codex;
        assert!(matches!(
            store
                .reserve_alternate_attempt(&lane, "initial", &invalid, at(0))
                .await,
            Err(StoreError::StaleFence { .. })
        ));
        store
            .release_lane(&newer, at(0))
            .await
            .expect("new owner should release");
    }

    #[tokio::test]
    async fn active_lane_and_supervisor_should_have_single_owner() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (supervisor, _lane) = owned_lane(&store).await;

        assert!(matches!(
            store
                .acquire_supervisor("supervisor-b", at(0), Duration::minutes(1))
                .await,
            Err(StoreError::LeaseBusy { .. })
        ));
        assert!(matches!(
            store
                .acquire_lane(
                    "lane-0",
                    "lane-owner-b",
                    &supervisor,
                    at(0),
                    Duration::minutes(1)
                )
                .await,
            Err(StoreError::LeaseBusy { .. })
        ));
    }

    #[tokio::test]
    async fn recovery_resolution_requires_supervisor_authority_and_next_owner_gets_new_fence() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (supervisor, lane) = owned_lane(&store).await;
        store
            .mark_lane_recovery_required(&lane, "dirty", at(0))
            .await
            .expect("lane should fence for recovery");
        store
            .resolve_lane_recovery(&supervisor, &lane.lane_id, at(0))
            .await
            .expect("current supervisor may clear verified recovery");
        let next = store
            .acquire_lane(
                &lane.lane_id,
                "lane-owner-next",
                &supervisor,
                at(0),
                Duration::minutes(1),
            )
            .await
            .expect("verified lane should reacquire");

        assert!(next.fence > lane.fence);
    }

    #[tokio::test]
    async fn only_one_half_open_probe_should_be_owned() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        store
            .open_provider_circuit("claude/default", "config-a", "quota", Some(at(1)), at(0))
            .await
            .expect("circuit should open");

        let first = store
            .try_acquire_half_open(
                "claude/default",
                "probe-a",
                "config-a",
                at(1),
                Duration::minutes(1),
            )
            .await
            .expect("probe acquisition should succeed");
        let second = store
            .try_acquire_half_open(
                "claude/default",
                "probe-b",
                "config-a",
                at(1),
                Duration::minutes(1),
            )
            .await
            .expect("probe contention should be a normal outcome");

        assert!(first);
        assert!(!second);
    }

    #[tokio::test]
    async fn ten_thousand_quota_ticks_should_keep_one_attempt_bucket_and_exemplar() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (_supervisor, lane) = owned_lane(&store).await;
        let spec = attempt(&lane, "attempt-1", "goal-108", temp.path());
        store
            .reserve_initial_attempt(&spec, at(0))
            .await
            .expect("initial attempt should reserve");
        let engine = PolicyEngine::new(SystemClock);
        let mut result = crate::model::NormalizedResult::spawn_failed("quota");
        result.class = ResultClass::QuotaExhausted;
        let decision = engine.decide_at(&result, 1, "quota-fingerprint", at(0));
        store
            .record_failure(FailureObservation {
                attempt_id: "attempt-1",
                provider_key: "claude/default",
                config_fingerprint: "config-a",
                fingerprint: "quota-fingerprint",
                exemplar_ref: Some("sha256:exemplar"),
                result: &result,
                decision: &decision,
                occurred_at: at(0),
            })
            .await
            .expect("failure should record");

        for _ in 1..10_000 {
            store
                .record_suppression(
                    SuppressionReason::ProviderCircuit,
                    "claude/default",
                    Some("quota-fingerprint"),
                    decision.retry_at(),
                    at(0),
                )
                .await
                .expect("suppression should aggregate");
        }

        assert_eq!(store.attempt_count().await.expect("attempt count"), 1);
        let bucket = store
            .failure_bucket("quota-fingerprint")
            .await
            .expect("bucket query")
            .expect("bucket should exist");
        assert_eq!(bucket.failure_count, 1);
        assert_eq!(bucket.suppressed_count, 9_999);
        assert_eq!(bucket.exemplar_ref.as_deref(), Some("sha256:exemplar"));
        assert_eq!(store.suppression_row_count().await.expect("row count"), 1);
    }

    #[tokio::test]
    async fn system_fuse_should_require_quiet_period_then_diagnostics() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;

        for failure in 0..10 {
            store
                .record_launch_failure("claude/default", &format!("failure-{failure}"), at(0))
                .await
                .expect("launch failure should record");
        }

        assert!(matches!(
            store.system_gate(at(14)).await.expect("system gate"),
            SystemGate::Paused { .. }
        ));
        store
            .mark_system_diagnostics_passed(at(14))
            .await
            .expect("early diagnostics should record");
        assert!(matches!(
            store.system_gate(at(15)).await.expect("system gate"),
            SystemGate::Paused { .. }
        ));
        store
            .mark_system_diagnostics_passed(at(15))
            .await
            .expect("post-quiet diagnostics should record");
        assert_eq!(
            store.system_gate(at(15)).await.expect("system gate"),
            SystemGate::Closed
        );
    }

    #[tokio::test]
    async fn backup_should_be_atomic_and_readable() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let destination = temp.path().join("backups").join("control-backup.sqlite3");

        store
            .backup_atomic(&destination)
            .await
            .expect("backup should succeed");

        assert!(destination.is_file());
        let monitor = ControlStore::open_monitor(&destination)
            .await
            .expect("backup should be a readable SQLite database");
        assert!(monitor.integrity_check().await.is_ok());
    }

    #[tokio::test]
    async fn probe_cache_should_require_matching_fingerprint_and_fresh_ttl() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        store
            .put_probe(ProbeCacheEntry {
                probe_key: "rust-link".to_owned(),
                input_fingerprint: "toolchain-a".to_owned(),
                outcome: "pass".to_owned(),
                details_json: "{}".to_owned(),
                checked_at: at(0),
                valid_until: at(5),
            })
            .await
            .expect("probe should cache");

        assert!(
            store
                .valid_probe("rust-link", "toolchain-a", at(4))
                .await
                .expect("probe query")
                .is_some()
        );
        assert!(
            store
                .valid_probe("rust-link", "toolchain-b", at(4))
                .await
                .expect("probe query")
                .is_none()
        );
        assert!(
            store
                .valid_probe("rust-link", "toolchain-a", at(5))
                .await
                .expect("probe query")
                .is_none()
        );
    }

    #[tokio::test]
    async fn compatibility_proof_should_be_keyed_by_exact_provider_identity() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let proof = CompatibilityRecord {
            provider_key: "codex/default".to_owned(),
            cli_version: "2.3.4".to_owned(),
            model: "gpt-fixture".to_owned(),
            config_fingerprint: "config-a".to_owned(),
            compatibility_kind: "resume".to_owned(),
            proven: true,
            proof_ref: Some("sha256:proof".to_owned()),
            checked_at: at(0),
            valid_until: Some(at(5)),
        };
        store
            .upsert_compatibility(&proof)
            .await
            .expect("compatibility proof should persist");

        assert_eq!(
            store
                .compatibility(
                    "codex/default",
                    "2.3.4",
                    "gpt-fixture",
                    "config-a",
                    "resume",
                    at(4),
                )
                .await
                .expect("compatibility query"),
            Some(proof)
        );
        assert!(
            store
                .compatibility(
                    "codex/default",
                    "2.3.5",
                    "gpt-fixture",
                    "config-a",
                    "resume",
                    at(4),
                )
                .await
                .expect("compatibility query")
                .is_none()
        );
    }

    #[tokio::test]
    async fn three_identical_failures_should_open_fingerprint_for_one_hour() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        let (_supervisor, lane) = owned_lane(&store).await;
        let engine = PolicyEngine::new(SystemClock);
        let result = crate::model::NormalizedResult {
            class: ResultClass::TransientTransport,
            terminal_type: Some("error".to_owned()),
            structured_started: true,
            session_id: None,
            provider_error_code: Some("transport".to_owned()),
            stable_error_hash: Some("stable".to_owned()),
            retry_at: None,
            exit_code: Some(1),
            summary: "transport".to_owned(),
        };
        let decision = engine.decide_at(&result, 1, "same-failure", at(0));

        for index in 0..3 {
            let attempt_id = format!("attempt-{index}");
            let work_key = format!("work-{index}");
            store
                .reserve_initial_attempt(
                    &attempt(&lane, &attempt_id, &work_key, temp.path()),
                    at(0),
                )
                .await
                .expect("attempt should reserve");
            store
                .record_failure(FailureObservation {
                    attempt_id: &attempt_id,
                    provider_key: "claude/default",
                    config_fingerprint: "config-a",
                    fingerprint: "same-failure",
                    exemplar_ref: Some("sha256:one-exemplar"),
                    result: &result,
                    decision: &decision,
                    occurred_at: at(0),
                })
                .await
                .expect("failure should record");
        }

        assert_eq!(
            store
                .fingerprint_gate("same-failure", at(59))
                .await
                .expect("fingerprint gate"),
            FingerprintGate::Open {
                retry_at: at(0) + Duration::hours(1)
            }
        );
    }

    #[tokio::test]
    async fn five_provider_failures_should_open_provider_circuit() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;

        for failure in 0..5 {
            store
                .record_launch_failure("codex/default", &format!("failure-{failure}"), at(0))
                .await
                .expect("launch failure should record");
        }

        assert_eq!(
            store
                .provider_gate("codex/default", "config-a", at(0))
                .await
                .expect("provider gate"),
            ProviderGate::Open {
                retry_at: Some(at(15))
            }
        );
    }

    #[tokio::test]
    async fn disabled_provider_should_reopen_only_after_fingerprint_change() {
        let temp = TempDir::new().expect("temp directory should be created");
        let store = store(&temp).await;
        store
            .disable_provider_until_fingerprint_change("claude/default", "config-a", "auth", at(0))
            .await
            .expect("provider should disable");

        assert_eq!(
            store
                .provider_gate("claude/default", "config-a", at(0))
                .await
                .expect("provider gate"),
            ProviderGate::DisabledUntilFingerprintChanges
        );
        assert_eq!(
            store
                .provider_gate("claude/default", "config-b", at(0))
                .await
                .expect("provider gate"),
            ProviderGate::Closed
        );
    }
}
