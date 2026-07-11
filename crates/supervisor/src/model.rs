use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Provider protocol implemented by a one-shot adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => Err(format!("unsupported provider {value:?}")),
        }
    }
}

/// Normalized terminal class. These values are persisted; rename only with a
/// control-store migration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultClass {
    Completed,
    OperatorStop,
    SpawnFailed,
    TransientTransport,
    RateLimited,
    QuotaExhausted,
    Auth,
    SessionInvalid,
    ProviderUnavailable,
    RunnerConfig,
    Policy,
    Ambiguous,
    PostconditionFailed,
}

impl ResultClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::OperatorStop => "operator_stop",
            Self::SpawnFailed => "spawn_failed",
            Self::TransientTransport => "transient_transport",
            Self::RateLimited => "rate_limited",
            Self::QuotaExhausted => "quota_exhausted",
            Self::Auth => "auth",
            Self::SessionInvalid => "session_invalid",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::RunnerConfig => "runner_config",
            Self::Policy => "policy",
            Self::Ambiguous => "ambiguous",
            Self::PostconditionFailed => "postcondition_failed",
        }
    }

    #[must_use]
    pub const fn is_failure(self) -> bool {
        !matches!(self, Self::Completed | Self::OperatorStop)
    }
}

impl fmt::Display for ResultClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ResultClass {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "completed" => Ok(Self::Completed),
            "operator_stop" => Ok(Self::OperatorStop),
            "spawn_failed" => Ok(Self::SpawnFailed),
            "transient_transport" => Ok(Self::TransientTransport),
            "rate_limited" => Ok(Self::RateLimited),
            "quota_exhausted" => Ok(Self::QuotaExhausted),
            "auth" => Ok(Self::Auth),
            "session_invalid" => Ok(Self::SessionInvalid),
            "provider_unavailable" => Ok(Self::ProviderUnavailable),
            "runner_config" => Ok(Self::RunnerConfig),
            "policy" => Ok(Self::Policy),
            "ambiguous" => Ok(Self::Ambiguous),
            "postcondition_failed" => Ok(Self::PostconditionFailed),
            _ => Err(format!("unknown result class {value:?}")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Reserved,
    Running,
    Completed,
    Failed,
    RecoveryRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Normal,
    Recovery,
    CompatibilityCanary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderIdentity {
    pub provider: Provider,
    pub executable: PathBuf,
    pub cli_version: String,
    pub model: Option<String>,
    pub config_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttemptSpec {
    pub id: String,
    pub lane_id: String,
    pub lane_fence: i64,
    pub work_key: String,
    pub worktree: PathBuf,
    pub expected_branch: String,
    pub prompt: String,
    pub prompt_kind: PromptKind,
    pub provider: ProviderIdentity,
    pub resume_session_id: Option<String>,
    pub preflight_signature: String,
    pub git_head_before: String,
    pub journal_sha_before: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NormalizedResult {
    pub class: ResultClass,
    pub terminal_type: Option<String>,
    pub structured_started: bool,
    pub session_id: Option<String>,
    pub provider_error_code: Option<String>,
    pub stable_error_hash: Option<String>,
    pub retry_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub summary: String,
}

impl NormalizedResult {
    #[must_use]
    pub fn spawn_failed(summary: impl Into<String>) -> Self {
        Self {
            class: ResultClass::SpawnFailed,
            terminal_type: None,
            structured_started: false,
            session_id: None,
            provider_error_code: None,
            stable_error_hash: None,
            retry_at: None,
            exit_code: None,
            summary: summary.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionReason {
    ProviderCircuit,
    FailureFingerprint,
    SystemPause,
    PreflightWait,
    RecoveryRequired,
    AttemptBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TickOutcome {
    Completed(String),
    Failed {
        attempt_id: String,
        class: ResultClass,
    },
    Suppressed {
        reason: SuppressionReason,
        retry_at: Option<DateTime<Utc>>,
    },
    RecoveryRequired {
        lane_id: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub stdin: Vec<u8>,
    pub env: Vec<(String, String)>,
    pub remove_env: Vec<String>,
}
