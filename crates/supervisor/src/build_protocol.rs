use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt as _, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
};
use ulid::Ulid;

use crate::build_classifier::ResourceClass;
use crate::build_policy::BuildPolicySnapshot;
use crate::build_store::BuildRequestState;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_JSON_LINE_BYTES: usize = 1024 * 1024;
pub const CONTROL_TOKEN_FILE: &str = "control.token";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClientEnvelope {
    pub protocol_version: u32,
    pub control_token: String,
    pub request: BuildControlRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildControlRequest {
    Build(BuildRequestMessage),
    Policy,
    Status,
    Recover {
        request_id: String,
        supervisor_fence: i64,
        owner_identity: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildRequestMessage {
    pub supervisor_fence: i64,
    pub lane_id: Option<String>,
    pub lane_fence: Option<i64>,
    pub owner_identity: String,
    pub policy_sha256: String,
    pub explicit_class: Option<ResourceClass>,
    pub category: Option<String>,
    pub worktree: PathBuf,
    pub target_dir: PathBuf,
    pub cargo_args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    QueueHeartbeat {
        request_id: String,
        position: usize,
    },
    Admission {
        request_id: String,
        resource_class: ResourceClass,
        effective_jobs: usize,
        policy_sha256: String,
    },
    Stdout {
        request_id: String,
        bytes: Vec<u8>,
    },
    Stderr {
        request_id: String,
        bytes: Vec<u8>,
    },
    Terminal {
        request_id: String,
        state: BuildRequestState,
        exit_code: Option<i32>,
    },
    Policy {
        snapshot: BuildPolicySnapshot,
        bounded_policy: String,
        supervisor_fence: i64,
    },
    Error {
        code: String,
        message: String,
        active_policy_sha256: Option<String>,
        bounded_policy: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlEndpoint {
    display: String,
}

impl ControlEndpoint {
    pub fn for_state_root(root: &Path) -> Result<Self, ProtocolError> {
        let canonical = root.canonicalize()?;
        let mut identity = canonical.to_string_lossy().replace('\\', "/");
        if cfg!(windows) {
            identity.make_ascii_lowercase();
        }
        let hash = hex::encode(Sha256::digest(identity.as_bytes()));
        let display = if cfg!(windows) {
            format!(r"\\.\pipe\govfolio-loop-{}", &hash[..16])
        } else {
            root.join("control.sock").to_string_lossy().into_owned()
        };
        Ok(Self { display })
    }

    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("build control I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("build control JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("build control frame exceeds {MAX_JSON_LINE_BYTES} bytes")]
    FrameTooLarge,
    #[error("build control token is invalid")]
    InvalidToken,
    #[error("unsupported build control protocol version {0}")]
    InvalidVersion(u32),
    #[error("build control owner identity is invalid")]
    InvalidOwner,
    #[error("stale supervisor fence: expected {expected}, received {received}")]
    StaleFence { expected: i64, received: i64 },
    #[error("policy refresh required; active policy is {active}")]
    PolicyRefreshRequired { active: String },
    #[error("build request is invalid: {0}")]
    InvalidRequest(String),
}

pub fn validate_envelope(
    envelope: &ClientEnvelope,
    expected_token: &str,
    supervisor_fence: i64,
    active_policy_sha256: &str,
) -> Result<(), ProtocolError> {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::InvalidVersion(envelope.protocol_version));
    }
    if !constant_time_eq(envelope.control_token.as_bytes(), expected_token.as_bytes()) {
        return Err(ProtocolError::InvalidToken);
    }
    match &envelope.request {
        BuildControlRequest::Build(build) => {
            validate_owner(&build.owner_identity)?;
            if build.supervisor_fence != supervisor_fence {
                return Err(ProtocolError::StaleFence {
                    expected: supervisor_fence,
                    received: build.supervisor_fence,
                });
            }
            if build.policy_sha256 != active_policy_sha256 {
                return Err(ProtocolError::PolicyRefreshRequired {
                    active: active_policy_sha256.to_owned(),
                });
            }
            if build.cargo_args.is_empty()
                || build.policy_sha256.len() != 64
                || build.worktree.as_os_str().is_empty()
                || build.target_dir.as_os_str().is_empty()
                || build.lane_id.is_some() != build.lane_fence.is_some()
            {
                return Err(ProtocolError::InvalidRequest(
                    "missing Cargo, path, policy, or lane identity".to_owned(),
                ));
            }
            if build.lane_id.is_none() && !build.owner_identity.starts_with("interactive:") {
                return Err(ProtocolError::InvalidOwner);
            }
            Ok(())
        }
        BuildControlRequest::Recover {
            request_id,
            supervisor_fence: received,
            owner_identity,
        } => {
            validate_owner(owner_identity)?;
            if request_id.trim().is_empty() {
                return Err(ProtocolError::InvalidRequest(
                    "recovery request id is empty".to_owned(),
                ));
            }
            if *received != supervisor_fence {
                return Err(ProtocolError::StaleFence {
                    expected: supervisor_fence,
                    received: *received,
                });
            }
            Ok(())
        }
        BuildControlRequest::Policy | BuildControlRequest::Status => Ok(()),
    }
}

pub fn load_or_create_control_token(state_root: &Path) -> Result<String, ProtocolError> {
    std::fs::create_dir_all(state_root)?;
    let path = state_root.join(CONTROL_TOKEN_FILE);
    if path.exists() {
        return read_token(&path);
    }
    let entropy = format!("{}:{}:{}", Ulid::new(), Ulid::new(), Ulid::new());
    let token = hex::encode(Sha256::digest(entropy.as_bytes()));
    let created = create_token_file(&path, token.as_bytes());
    match created {
        Ok(()) => Ok(token),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => read_token(&path),
        Err(error) => Err(ProtocolError::Io(error)),
    }
}

pub async fn read_json_line<R, T>(reader: &mut R) -> Result<Option<T>, ProtocolError>
where
    R: AsyncBufRead + Unpin,
    T: DeserializeOwned,
{
    let mut bytes = Vec::new();
    let read = reader
        .take((MAX_JSON_LINE_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_JSON_LINE_BYTES || bytes.last() != Some(&b'\n') {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

pub async fn write_json_line<W, T>(writer: &mut W, value: &T) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let mut bytes = serde_json::to_vec(value)?;
    if bytes.len() + 1 > MAX_JSON_LINE_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

fn validate_owner(owner: &str) -> Result<(), ProtocolError> {
    if owner.trim().is_empty() || owner.len() > 256 || owner.chars().any(char::is_control) {
        Err(ProtocolError::InvalidOwner)
    } else {
        Ok(())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

fn read_token(path: &Path) -> Result<String, ProtocolError> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let mut token = String::new();
    file.read_to_string(&mut token)?;
    let token = token.trim().to_owned();
    if token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(token)
    } else {
        Err(ProtocolError::InvalidToken)
    }
}

#[cfg(unix)]
fn create_token_file(path: &Path, token: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(token)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn create_token_file(path: &Path, token: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(token)?;
    file.sync_all()
}
