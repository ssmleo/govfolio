//! Native-host discovery and capability proofs used before any WSL fallback.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncReadExt as _;
use tokio::process::Command;
use tokio::time::timeout;
use ulid::Ulid;

use crate::model::{Provider, ProviderIdentity};
use crate::store::{ControlStore, ProbeCacheEntry, StoreError};

pub const NATIVE_CODEX_IDENTITY_PROBE: &str = "native_codex_identity_v1";
pub const NATIVE_UNSUPPORTED_PROOF_SCHEMA: &str = "govfolio.native-unsupported/v1";
const MAX_CANDIDATES: usize = 128;
const DEFAULT_COMMAND_TIMEOUT: StdDuration = StdDuration::from_secs(30);
const LINK_MARKER: &str = "govfolio-native-link-ok";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCandidateSource {
    Explicit,
    Path,
    LocalAppData,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeExecutableIdentity {
    pub path: PathBuf,
    pub cli_version: String,
    pub sha256: String,
    pub source: NativeCandidateSource,
}

impl NativeExecutableIdentity {
    #[must_use]
    pub fn fingerprint(&self) -> String {
        digest_fields(&[
            "native-codex-identity-v1",
            &self.path.to_string_lossy(),
            &self.cli_version,
            &self.sha256,
            source_name(self.source),
        ])
    }

    #[must_use]
    pub fn provider_identity(
        &self,
        model: Option<String>,
        base_config_fingerprint: &str,
    ) -> ProviderIdentity {
        ProviderIdentity {
            provider: Provider::Codex,
            executable: self.path.clone(),
            cli_version: self.cli_version.clone(),
            model,
            config_fingerprint: digest_fields(&[
                "codex-provider-config-v1",
                &self.fingerprint(),
                base_config_fingerprint,
            ]),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeResolverInputs {
    pub explicit: Option<PathBuf>,
    pub path_candidates: Vec<PathBuf>,
    pub local_appdata_candidates: Vec<PathBuf>,
    pub local_discovery_error: Option<String>,
}

impl NativeResolverInputs {
    /// Discovers candidates without executing any of them.
    ///
    /// # Errors
    ///
    /// Returns an error when PATH contains more entries than the bounded
    /// resolver is willing to inspect.
    pub fn from_environment() -> Result<Self, NativeResolverError> {
        Self::from_values(
            std::env::var_os("GOVFOLIO_CODEX_BIN"),
            std::env::var_os("PATH"),
            std::env::var_os("LOCALAPPDATA"),
        )
    }

    /// Builds resolver inputs from injected environment values.
    ///
    /// # Errors
    ///
    /// Returns an error for an overlarge PATH candidate set.
    pub fn from_values(
        explicit: Option<OsString>,
        path: Option<OsString>,
        local_appdata: Option<OsString>,
    ) -> Result<Self, NativeResolverError> {
        let path_candidates = discover_path_candidates(path.as_deref())?;
        let (local_appdata_candidates, local_discovery_error) =
            discover_local_candidates(local_appdata.as_deref());
        Ok(Self {
            explicit: explicit.map(PathBuf::from),
            path_candidates,
            local_appdata_candidates,
            local_discovery_error,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableInspection {
    pub canonical_path: PathBuf,
    pub cli_version: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum NativeProbeError {
    #[error("candidate is not a regular file")]
    NotAFile,
    #[error("candidate I/O failed ({kind})")]
    Io { kind: String },
    #[error("candidate version command timed out")]
    TimedOut,
    #[error("candidate cannot execute on this native platform")]
    UnsupportedPlatform,
    #[error("candidate version command exited unsuccessfully")]
    VersionFailed,
    #[error("candidate version output is empty")]
    EmptyVersion,
}

#[async_trait]
pub trait NativeExecutableProbe: Send + Sync {
    async fn inspect(&self, path: &Path) -> Result<ExecutableInspection, NativeProbeError>;
}

#[derive(Clone, Debug)]
pub struct SystemNativeExecutableProbe {
    timeout: StdDuration,
}

impl Default for SystemNativeExecutableProbe {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_COMMAND_TIMEOUT,
        }
    }
}

impl SystemNativeExecutableProbe {
    #[must_use]
    pub fn new(timeout: StdDuration) -> Self {
        Self {
            timeout: timeout.max(StdDuration::from_secs(1)),
        }
    }
}

#[async_trait]
impl NativeExecutableProbe for SystemNativeExecutableProbe {
    async fn inspect(&self, path: &Path) -> Result<ExecutableInspection, NativeProbeError> {
        let metadata = tokio::fs::metadata(path).await.map_err(probe_io_error)?;
        if !metadata.is_file() {
            return Err(NativeProbeError::NotAFile);
        }
        let canonical_path = tokio::fs::canonicalize(path)
            .await
            .map_err(probe_io_error)?;
        let sha256 = Box::pin(hash_file(&canonical_path)).await?;
        let version = run_version(&canonical_path, self.timeout).await?;
        Ok(ExecutableInspection {
            canonical_path,
            cli_version: version,
            sha256,
        })
    }
}

pub struct NativeCodexResolver<P> {
    probe: P,
}

impl<P> NativeCodexResolver<P>
where
    P: NativeExecutableProbe,
{
    #[must_use]
    pub const fn new(probe: P) -> Self {
        Self { probe }
    }

    /// Resolves Codex in strict explicit, PATH, `LocalAppData` order.
    ///
    /// # Errors
    ///
    /// An explicit override is authoritative and fails closed. PATH chooses
    /// the first successful candidate. `LocalAppData` succeeds only when exactly
    /// one distinct candidate passes inspection.
    pub async fn resolve(
        &self,
        inputs: &NativeResolverInputs,
    ) -> Result<NativeExecutableIdentity, NativeResolverError> {
        if let Some(explicit) = &inputs.explicit {
            return self.resolve_explicit(explicit).await;
        }

        ensure_bounded(&inputs.path_candidates)?;
        for candidate in dedupe_paths(&inputs.path_candidates) {
            if let Ok(inspection) = self.probe.inspect(&candidate).await {
                return Ok(identity(inspection, NativeCandidateSource::Path));
            }
        }

        ensure_bounded(&inputs.local_appdata_candidates)?;
        let local_candidates = dedupe_paths(&inputs.local_appdata_candidates);
        let mut successful = Vec::new();
        let mut unsupported = Vec::new();
        for candidate in &local_candidates {
            match self.probe.inspect(candidate).await {
                Ok(inspection) => successful.push(inspection),
                Err(NativeProbeError::UnsupportedPlatform) => {
                    unsupported.push(candidate.clone());
                }
                Err(_) => {}
            }
        }
        dedupe_inspections(&mut successful);
        match successful.as_slice() {
            [inspection] => Ok(identity(
                inspection.clone(),
                NativeCandidateSource::LocalAppData,
            )),
            [] if inputs.path_candidates.is_empty()
                && local_candidates.len() == 1
                && unsupported.len() == 1 =>
            {
                Err(NativeResolverError::NativeUnsupported {
                    path: unsupported.remove(0),
                })
            }
            [] => Err(NativeResolverError::NotFound {
                local_discovery_error: inputs.local_discovery_error.clone(),
            }),
            _ => Err(NativeResolverError::AmbiguousLocalCandidates(
                successful
                    .into_iter()
                    .map(|inspection| inspection.canonical_path)
                    .collect(),
            )),
        }
    }

    async fn resolve_explicit(
        &self,
        path: &Path,
    ) -> Result<NativeExecutableIdentity, NativeResolverError> {
        match self.probe.inspect(path).await {
            Ok(inspection) => Ok(identity(inspection, NativeCandidateSource::Explicit)),
            Err(NativeProbeError::UnsupportedPlatform) => {
                Err(NativeResolverError::NativeUnsupported {
                    path: path.to_path_buf(),
                })
            }
            Err(source) => Err(NativeResolverError::ExplicitRejected {
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

#[derive(Debug, Error)]
pub enum NativeResolverError {
    #[error("explicit Codex candidate {path} was rejected: {source}", path = path.display())]
    ExplicitRejected {
        path: PathBuf,
        source: NativeProbeError,
    },
    #[error("native Codex executable format is unsupported: {path}", path = path.display())]
    NativeUnsupported { path: PathBuf },
    #[error("no successful native Codex candidate was found")]
    NotFound {
        local_discovery_error: Option<String>,
    },
    #[error("multiple successful LocalAppData Codex candidates: {0:?}")]
    AmbiguousLocalCandidates(Vec<PathBuf>),
    #[error("native resolver candidate count exceeds {MAX_CANDIDATES}")]
    TooManyCandidates,
}

impl NativeResolverError {
    #[must_use]
    pub const fn permits_wsl_bootstrap(&self) -> bool {
        matches!(self, Self::NativeUnsupported { .. })
    }

    #[must_use]
    pub fn unsupported_proof(&self, checked_at: DateTime<Utc>) -> Option<NativeUnsupportedProof> {
        match self {
            Self::NativeUnsupported { path } => {
                Some(NativeUnsupportedProof::new(path.clone(), None, checked_at))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum NativeIdentityPersistenceError {
    #[error("native identity validity must be positive")]
    InvalidValidity,
    #[error("native identity timestamp overflow")]
    TimestampOverflow,
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Persists the exact path/version/hash identity in the existing probe store.
///
/// # Errors
///
/// Returns an error for invalid validity, serialization, or store failures.
pub async fn persist_native_identity(
    store: &ControlStore,
    identity: &NativeExecutableIdentity,
    checked_at: DateTime<Utc>,
    valid_for: Duration,
) -> Result<(), NativeIdentityPersistenceError> {
    if valid_for <= Duration::zero() {
        return Err(NativeIdentityPersistenceError::InvalidValidity);
    }
    let valid_until = checked_at
        .checked_add_signed(valid_for)
        .ok_or(NativeIdentityPersistenceError::TimestampOverflow)?;
    store
        .put_probe(ProbeCacheEntry {
            probe_key: NATIVE_CODEX_IDENTITY_PROBE.to_owned(),
            input_fingerprint: identity.fingerprint(),
            outcome: "pass".to_owned(),
            details_json: serde_json::to_string(identity)?,
            checked_at,
            valid_until,
        })
        .await?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub remove_env: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl HostCommandOutput {
    #[must_use]
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum HostCommandError {
    #[error("host command I/O failed ({kind}, os={raw_os_error:?})")]
    Io {
        kind: String,
        raw_os_error: Option<i32>,
    },
    #[error("host command timed out")]
    TimedOut,
    #[error("host command executable format is unsupported")]
    UnsupportedPlatform,
}

#[async_trait]
pub trait HostCommandRunner: Send + Sync {
    async fn run(
        &self,
        specification: &HostCommandSpec,
    ) -> Result<HostCommandOutput, HostCommandError>;
}

#[derive(Clone, Debug)]
pub struct SystemHostCommandRunner {
    timeout: StdDuration,
}

impl Default for SystemHostCommandRunner {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_COMMAND_TIMEOUT,
        }
    }
}

impl SystemHostCommandRunner {
    #[must_use]
    pub fn new(timeout: StdDuration) -> Self {
        Self {
            timeout: timeout.max(StdDuration::from_secs(1)),
        }
    }
}

#[async_trait]
impl HostCommandRunner for SystemHostCommandRunner {
    async fn run(
        &self,
        specification: &HostCommandSpec,
    ) -> Result<HostCommandOutput, HostCommandError> {
        let mut command = Command::new(&specification.program);
        command
            .args(&specification.args)
            .current_dir(&specification.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for key in &specification.remove_env {
            command.env_remove(key);
        }
        let output = timeout(self.timeout, command.output())
            .await
            .map_err(|_| HostCommandError::TimedOut)?
            .map_err(host_io_error)?;
        Ok(HostCommandOutput {
            exit_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

#[derive(Clone, Debug)]
pub struct NativeSmokeRequest {
    pub repo: PathBuf,
    pub scratch_root: PathBuf,
    pub git_executable: PathBuf,
    pub rustc_executable: PathBuf,
    pub codex: NativeExecutableIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeSmokeReport {
    pub source_head: String,
    pub git_common_dir: PathBuf,
    pub codex_identity_fingerprint: String,
    pub linked_worktree: PathBuf,
    pub report_fingerprint: String,
}

#[derive(Debug, Error)]
pub enum NativeSmokeError {
    #[error("smoke scratch root must be outside the repository")]
    ScratchInsideRepository,
    #[error("native smoke I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("native smoke command {stage} failed: {source}")]
    Command {
        stage: &'static str,
        source: HostCommandError,
    },
    #[error("native smoke command {stage} exited {exit_code:?}: {summary}")]
    CommandFailed {
        stage: &'static str,
        exit_code: Option<i32>,
        summary: String,
    },
    #[error("native Codex executable format is unsupported")]
    NativeUnsupported,
    #[error("native smoke output for {stage} is invalid: {reason}")]
    InvalidOutput { stage: &'static str, reason: String },
    #[error("linked worktree resolves a different Git common directory")]
    GitCommonMismatch,
    #[error("native smoke cleanup failed: {0}")]
    Cleanup(String),
}

impl NativeSmokeError {
    #[must_use]
    pub const fn permits_wsl_bootstrap(&self) -> bool {
        matches!(self, Self::NativeUnsupported)
    }

    #[must_use]
    pub fn unsupported_proof(
        &self,
        codex: &NativeExecutableIdentity,
        checked_at: DateTime<Utc>,
    ) -> Option<NativeUnsupportedProof> {
        self.permits_wsl_bootstrap().then(|| {
            NativeUnsupportedProof::new(codex.path.clone(), Some(codex.sha256.clone()), checked_at)
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeUnsupportedProof {
    pub schema: String,
    pub outcome: String,
    pub reason: String,
    pub executable: PathBuf,
    pub executable_sha256: Option<String>,
    pub checked_at: DateTime<Utc>,
}

impl NativeUnsupportedProof {
    fn new(
        executable: PathBuf,
        executable_sha256: Option<String>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            schema: NATIVE_UNSUPPORTED_PROOF_SCHEMA.to_owned(),
            outcome: "native_unsupported".to_owned(),
            reason: "codex_bad_executable_format".to_owned(),
            executable,
            executable_sha256,
            checked_at,
        }
    }
}

/// Executes the native linked-worktree/Git-common/real-link smoke.
///
/// # Errors
///
/// Returns [`NativeSmokeError::NativeUnsupported`] only for the operating
/// system's specific bad-executable-format condition. Every permission,
/// linker, Git, timeout, and cleanup failure remains ineligible for WSL.
pub async fn run_native_smoke<R: HostCommandRunner>(
    runner: &R,
    request: &NativeSmokeRequest,
) -> Result<NativeSmokeReport, NativeSmokeError> {
    let repo = std::fs::canonicalize(&request.repo)?;
    std::fs::create_dir_all(&request.scratch_root)?;
    let scratch_root = std::fs::canonicalize(&request.scratch_root)?;
    if scratch_root.starts_with(&repo) {
        return Err(NativeSmokeError::ScratchInsideRepository);
    }

    let smoke_root = scratch_root.join(format!("native-smoke-{}", Ulid::new()));
    std::fs::create_dir(&smoke_root)?;
    let worktree = smoke_root.join("worktree");
    let preparation = prepare_linked_worktree(runner, request, &repo, &worktree).await;
    let (head, common_dir) = match preparation {
        Ok(prepared) => prepared,
        Err(error) => {
            remove_validated_tree(&smoke_root, &scratch_root)?;
            return Err(error);
        }
    };

    let smoke_result = run_linked_smoke(runner, request, &worktree, &common_dir, &head).await;
    let cleanup_result = cleanup_worktree(runner, request, &repo, &worktree).await;
    let tree_cleanup = remove_validated_tree(&smoke_root, &scratch_root);

    if let Err(error) = cleanup_result {
        return Err(NativeSmokeError::Cleanup(error));
    }
    tree_cleanup?;
    smoke_result
}

async fn prepare_linked_worktree<R: HostCommandRunner>(
    runner: &R,
    request: &NativeSmokeRequest,
    repo: &Path,
    worktree: &Path,
) -> Result<(String, PathBuf), NativeSmokeError> {
    let head = successful_text(
        runner,
        host_command(
            &request.git_executable,
            repo,
            &["-C", &path_text(repo), "rev-parse", "HEAD"],
        ),
        "git_head",
    )
    .await?;
    let common_text = successful_text(
        runner,
        host_command(
            &request.git_executable,
            repo,
            &[
                "-C",
                &path_text(repo),
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ],
        ),
        "git_common",
    )
    .await?;
    let common_dir = canonical_output_path(repo, &common_text, "git_common")?;

    successful(
        runner,
        host_command(
            &request.git_executable,
            repo,
            &[
                "-C",
                &path_text(repo),
                "worktree",
                "add",
                "--detach",
                &path_text(worktree),
                &head,
            ],
        ),
        "worktree_add",
    )
    .await?;
    Ok((head, common_dir))
}

async fn run_linked_smoke<R: HostCommandRunner>(
    runner: &R,
    request: &NativeSmokeRequest,
    worktree: &Path,
    common_dir: &Path,
    head: &str,
) -> Result<NativeSmokeReport, NativeSmokeError> {
    let linked_common_text = successful_text(
        runner,
        host_command(
            &request.git_executable,
            worktree,
            &[
                "-C",
                &path_text(worktree),
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ],
        ),
        "linked_git_common",
    )
    .await?;
    let linked_common = canonical_output_path(worktree, &linked_common_text, "linked_git_common")?;
    if linked_common != common_dir {
        return Err(NativeSmokeError::GitCommonMismatch);
    }
    prove_common_writable(common_dir)?;

    let version = successful_text(
        runner,
        host_command(&request.codex.path, worktree, &["--version"]),
        "codex_version",
    )
    .await
    .map_err(map_codex_unsupported)?;
    if version != request.codex.cli_version {
        return Err(NativeSmokeError::InvalidOutput {
            stage: "codex_version",
            reason: "version differs from resolved identity".to_owned(),
        });
    }

    let source = worktree.join("govfolio-native-link-smoke.rs");
    let executable = worktree.join(if cfg!(windows) {
        "govfolio-native-link-smoke.exe"
    } else {
        "govfolio-native-link-smoke"
    });
    std::fs::write(
        &source,
        format!("fn main() {{ println!(\"{LINK_MARKER}\"); }}\n"),
    )?;
    successful(
        runner,
        host_command(
            &request.rustc_executable,
            worktree,
            &[&path_text(&source), "-o", &path_text(&executable)],
        ),
        "rustc_link",
    )
    .await?;
    let linked_output = successful_text(
        runner,
        host_command(&executable, worktree, &[]),
        "linked_executable",
    )
    .await?;
    if linked_output != LINK_MARKER {
        return Err(NativeSmokeError::InvalidOutput {
            stage: "linked_executable",
            reason: "linked canary emitted unexpected output".to_owned(),
        });
    }

    let report_fingerprint = digest_fields(&[
        "native-smoke-report-v1",
        head,
        &common_dir.to_string_lossy(),
        &request.codex.fingerprint(),
    ]);
    Ok(NativeSmokeReport {
        source_head: head.to_owned(),
        git_common_dir: common_dir.to_path_buf(),
        codex_identity_fingerprint: request.codex.fingerprint(),
        linked_worktree: worktree.to_path_buf(),
        report_fingerprint,
    })
}

async fn cleanup_worktree<R: HostCommandRunner>(
    runner: &R,
    request: &NativeSmokeRequest,
    repo: &Path,
    worktree: &Path,
) -> Result<(), String> {
    let command = host_command(
        &request.git_executable,
        repo,
        &[
            "-C",
            &path_text(repo),
            "worktree",
            "remove",
            "--force",
            &path_text(worktree),
        ],
    );
    runner
        .run(&command)
        .await
        .map_err(|error| error.to_string())
        .and_then(|output| {
            output.success().then_some(()).ok_or_else(|| {
                format!(
                    "git worktree remove exited {:?}: {}",
                    output.exit_code,
                    stderr_summary(&output.stderr)
                )
            })
        })
}

fn prove_common_writable(common_dir: &Path) -> Result<(), NativeSmokeError> {
    let marker = common_dir.join(format!(".govfolio-native-smoke-{}.tmp", Ulid::new()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)?;
    file.write_all(LINK_MARKER.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::remove_file(marker)?;
    Ok(())
}

async fn successful<R: HostCommandRunner>(
    runner: &R,
    specification: HostCommandSpec,
    stage: &'static str,
) -> Result<HostCommandOutput, NativeSmokeError> {
    let output = runner
        .run(&specification)
        .await
        .map_err(|source| NativeSmokeError::Command { stage, source })?;
    if output.success() {
        Ok(output)
    } else {
        Err(NativeSmokeError::CommandFailed {
            stage,
            exit_code: output.exit_code,
            summary: stderr_summary(&output.stderr),
        })
    }
}

async fn successful_text<R: HostCommandRunner>(
    runner: &R,
    specification: HostCommandSpec,
    stage: &'static str,
) -> Result<String, NativeSmokeError> {
    let output = successful(runner, specification, stage).await?;
    first_line(&output.stdout).ok_or_else(|| NativeSmokeError::InvalidOutput {
        stage,
        reason: "stdout is empty".to_owned(),
    })
}

fn map_codex_unsupported(error: NativeSmokeError) -> NativeSmokeError {
    match error {
        NativeSmokeError::Command {
            stage: "codex_version",
            source: HostCommandError::UnsupportedPlatform,
        } => NativeSmokeError::NativeUnsupported,
        other => other,
    }
}

fn host_command(program: &Path, cwd: &Path, args: &[&str]) -> HostCommandSpec {
    HostCommandSpec {
        program: program.to_path_buf(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: cwd.to_path_buf(),
        remove_env: vec![
            "GIT_DIR".to_owned(),
            "GIT_WORK_TREE".to_owned(),
            "RUSTC_WRAPPER".to_owned(),
            "RUSTFLAGS".to_owned(),
        ],
    }
}

fn canonical_output_path(
    cwd: &Path,
    output: &str,
    stage: &'static str,
) -> Result<PathBuf, NativeSmokeError> {
    let path = PathBuf::from(output);
    let resolved = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    std::fs::canonicalize(resolved).map_err(|error| NativeSmokeError::InvalidOutput {
        stage,
        reason: format!("path cannot be canonicalized ({:?})", error.kind()),
    })
}

fn remove_validated_tree(path: &Path, parent: &Path) -> Result<(), std::io::Error> {
    if path.parent() != Some(parent) || !path.starts_with(parent) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "refusing to remove an unvalidated smoke path",
        ));
    }
    if path.exists() {
        std::fs::remove_dir_all(path)
    } else {
        Ok(())
    }
}

async fn run_version(
    path: &Path,
    command_timeout: StdDuration,
) -> Result<String, NativeProbeError> {
    let mut command = Command::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(command_timeout, command.output())
        .await
        .map_err(|_| NativeProbeError::TimedOut)?
        .map_err(probe_spawn_error)?;
    if !output.status.success() {
        return Err(NativeProbeError::VersionFailed);
    }
    first_line(&output.stdout)
        .or_else(|| first_line(&output.stderr))
        .ok_or(NativeProbeError::EmptyVersion)
}

async fn hash_file(path: &Path) -> Result<String, NativeProbeError> {
    let mut file = tokio::fs::File::open(path).await.map_err(probe_io_error)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = file.read(&mut buffer).await.map_err(probe_io_error)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn discover_path_candidates(
    path: Option<&std::ffi::OsStr>,
) -> Result<Vec<PathBuf>, NativeResolverError> {
    let executable = if cfg!(windows) { "codex.exe" } else { "codex" };
    let candidates = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .map(|directory| directory.join(executable))
        .collect::<Vec<_>>();
    ensure_bounded(&candidates)?;
    Ok(candidates)
}

fn discover_local_candidates(
    local_appdata: Option<&std::ffi::OsStr>,
) -> (Vec<PathBuf>, Option<String>) {
    let Some(local_appdata) = local_appdata else {
        return (Vec::new(), None);
    };
    let root = PathBuf::from(local_appdata)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), None),
        Err(error) => {
            return (
                Vec::new(),
                Some(format!(
                    "LocalAppData discovery failed ({:?})",
                    error.kind()
                )),
            );
        }
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("codex.exe"))
        .collect::<Vec<_>>();
    candidates.sort();
    if candidates.len() > MAX_CANDIDATES {
        candidates.clear();
        return (
            candidates,
            Some("LocalAppData candidate bound exceeded".to_owned()),
        );
    }
    (candidates, None)
}

fn ensure_bounded(candidates: &[PathBuf]) -> Result<(), NativeResolverError> {
    if candidates.len() > MAX_CANDIDATES {
        Err(NativeResolverError::TooManyCandidates)
    } else {
        Ok(())
    }
}

fn dedupe_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .iter()
        .filter(|path| seen.insert(path_key(path)))
        .cloned()
        .collect()
}

fn dedupe_inspections(inspections: &mut Vec<ExecutableInspection>) {
    let mut seen = HashSet::new();
    inspections.retain(|inspection| seen.insert(path_key(&inspection.canonical_path)));
}

fn path_key(path: &Path) -> String {
    let rendered = path.to_string_lossy();
    if cfg!(windows) {
        rendered.to_ascii_lowercase()
    } else {
        rendered.into_owned()
    }
}

fn identity(
    inspection: ExecutableInspection,
    source: NativeCandidateSource,
) -> NativeExecutableIdentity {
    NativeExecutableIdentity {
        path: inspection.canonical_path,
        cli_version: inspection.cli_version,
        sha256: inspection.sha256,
        source,
    }
}

fn source_name(source: NativeCandidateSource) -> &'static str {
    match source {
        NativeCandidateSource::Explicit => "explicit",
        NativeCandidateSource::Path => "path",
        NativeCandidateSource::LocalAppData => "local_appdata",
    }
}

fn digest_fields(fields: &[&str]) -> String {
    let mut digest = Sha256::new();
    for field in fields {
        digest.update(field.as_bytes());
        digest.update([0]);
    }
    hex::encode(digest.finalize())
}

fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(256).collect())
}

fn path_text(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        windows_child_path_text(&text)
    } else {
        text
    }
}

fn windows_child_path_text(text: &str) -> String {
    if let Some(path) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{path}");
    }
    if let Some(path) = text.strip_prefix(r"\\?\") {
        return path.to_owned();
    }
    text.to_owned()
}

fn stderr_summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let total = text.chars().count();
    text.chars()
        .skip(total.saturating_sub(512))
        .collect::<String>()
        .trim()
        .to_owned()
}

fn probe_io_error(error: std::io::Error) -> NativeProbeError {
    NativeProbeError::Io {
        kind: format!("{:?}", error.kind()),
    }
}

fn probe_spawn_error(error: std::io::Error) -> NativeProbeError {
    if executable_format_unsupported(&error) {
        NativeProbeError::UnsupportedPlatform
    } else {
        probe_io_error(error)
    }
}

fn host_io_error(error: std::io::Error) -> HostCommandError {
    if executable_format_unsupported(&error) {
        HostCommandError::UnsupportedPlatform
    } else {
        HostCommandError::Io {
            kind: format!("{:?}", error.kind()),
            raw_os_error: error.raw_os_error(),
        }
    }
}

fn executable_format_unsupported(error: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(193)
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ENOEXEC)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = error;
        false
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    use chrono::{TimeZone as _, Utc};
    use tempfile::tempdir;

    use super::*;

    #[derive(Default)]
    struct FakeProbe {
        results: HashMap<PathBuf, Result<ExecutableInspection, NativeProbeError>>,
        calls: Mutex<Vec<PathBuf>>,
    }

    #[async_trait]
    impl NativeExecutableProbe for FakeProbe {
        async fn inspect(&self, path: &Path) -> Result<ExecutableInspection, NativeProbeError> {
            self.calls
                .lock()
                .expect("probe call lock")
                .push(path.to_path_buf());
            self.results
                .get(path)
                .cloned()
                .unwrap_or(Err(NativeProbeError::NotAFile))
        }
    }

    #[tokio::test]
    async fn native_resolver_explicit_candidate_is_authoritative() {
        let explicit = PathBuf::from("C:/explicit/codex.exe");
        let path = PathBuf::from("C:/path/codex.exe");
        let probe = FakeProbe {
            results: HashMap::from([
                (explicit.clone(), Ok(inspection(&explicit, "codex 1", "aa"))),
                (path.clone(), Ok(inspection(&path, "codex 2", "bb"))),
            ]),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = NativeCodexResolver::new(probe);
        let identity = resolver
            .resolve(&NativeResolverInputs {
                explicit: Some(explicit.clone()),
                path_candidates: vec![path],
                ..NativeResolverInputs::default()
            })
            .await
            .expect("explicit candidate should resolve");

        assert_eq!(identity.source, NativeCandidateSource::Explicit);
        assert_eq!(identity.path, explicit);
        assert_eq!(
            resolver.probe.calls.lock().expect("probe call lock").len(),
            1
        );
    }

    #[tokio::test]
    async fn native_resolver_rejects_bad_explicit_without_falling_back() {
        let explicit = PathBuf::from("C:/explicit/codex.exe");
        let fallback = PathBuf::from("C:/path/codex.exe");
        let probe = FakeProbe {
            results: HashMap::from([
                (explicit.clone(), Err(NativeProbeError::VersionFailed)),
                (
                    fallback.clone(),
                    Ok(inspection(&fallback, "codex fallback", "bb")),
                ),
            ]),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = NativeCodexResolver::new(probe);

        let error = resolver
            .resolve(&NativeResolverInputs {
                explicit: Some(explicit),
                path_candidates: vec![fallback],
                ..NativeResolverInputs::default()
            })
            .await
            .expect_err("bad explicit candidate must fail closed");

        assert!(matches!(
            error,
            NativeResolverError::ExplicitRejected { .. }
        ));
        assert_eq!(
            resolver.probe.calls.lock().expect("probe call lock").len(),
            1
        );
    }

    #[tokio::test]
    async fn native_resolver_uses_first_successful_path_candidate() {
        let first = PathBuf::from("C:/first/codex.exe");
        let second = PathBuf::from("C:/second/codex.exe");
        let local = PathBuf::from("C:/local/codex.exe");
        let probe = FakeProbe {
            results: HashMap::from([
                (first.clone(), Err(NativeProbeError::NotAFile)),
                (second.clone(), Ok(inspection(&second, "codex path", "bb"))),
                (local.clone(), Ok(inspection(&local, "codex local", "cc"))),
            ]),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = NativeCodexResolver::new(probe);
        let identity = resolver
            .resolve(&NativeResolverInputs {
                path_candidates: vec![first.clone(), second.clone()],
                local_appdata_candidates: vec![local],
                ..NativeResolverInputs::default()
            })
            .await
            .expect("second PATH candidate should resolve");

        assert_eq!(identity.path, second);
        assert_eq!(identity.source, NativeCandidateSource::Path);
        assert_eq!(
            *resolver.probe.calls.lock().expect("probe call lock"),
            vec![first, identity.path]
        );
    }

    #[tokio::test]
    async fn native_resolver_requires_unique_successful_local_candidate() {
        let first = PathBuf::from("C:/local/one/codex.exe");
        let second = PathBuf::from("C:/local/two/codex.exe");
        let probe = FakeProbe {
            results: HashMap::from([
                (first.clone(), Ok(inspection(&first, "codex one", "aa"))),
                (second.clone(), Ok(inspection(&second, "codex two", "bb"))),
            ]),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = NativeCodexResolver::new(probe);

        let error = resolver
            .resolve(&NativeResolverInputs {
                local_appdata_candidates: vec![first, second],
                ..NativeResolverInputs::default()
            })
            .await
            .expect_err("two valid local candidates must be ambiguous");

        assert!(matches!(
            error,
            NativeResolverError::AmbiguousLocalCandidates(_)
        ));
    }

    #[tokio::test]
    async fn native_resolver_persists_exact_identity() {
        let temp = tempdir().expect("tempdir");
        let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
            .await
            .expect("store opens");
        let identity = NativeExecutableIdentity {
            path: PathBuf::from("C:/codex/codex.exe"),
            cli_version: "codex 1.2.3".to_owned(),
            sha256: "abcdef".to_owned(),
            source: NativeCandidateSource::Explicit,
        };
        let now = at(0);

        persist_native_identity(&store, &identity, now, Duration::hours(1))
            .await
            .expect("identity persists");
        let stored = store
            .valid_probe(NATIVE_CODEX_IDENTITY_PROBE, &identity.fingerprint(), now)
            .await
            .expect("probe lookup")
            .expect("identity probe exists");

        assert_eq!(
            serde_json::from_str::<NativeExecutableIdentity>(&stored.details_json)
                .expect("identity JSON"),
            identity
        );
    }

    #[test]
    fn native_resolver_only_bad_executable_format_permits_wsl() {
        let unsupported = NativeResolverError::NativeUnsupported {
            path: PathBuf::from("C:/codex.exe"),
        };
        let denied = NativeResolverError::ExplicitRejected {
            path: PathBuf::from("C:/codex.exe"),
            source: NativeProbeError::Io {
                kind: "PermissionDenied".to_owned(),
            },
        };

        assert!(unsupported.permits_wsl_bootstrap());
        assert!(!denied.permits_wsl_bootstrap());
    }

    struct FakeRunner {
        responses: Mutex<VecDeque<Result<HostCommandOutput, HostCommandError>>>,
        calls: Mutex<Vec<HostCommandSpec>>,
    }

    #[async_trait]
    impl HostCommandRunner for FakeRunner {
        async fn run(
            &self,
            specification: &HostCommandSpec,
        ) -> Result<HostCommandOutput, HostCommandError> {
            self.calls
                .lock()
                .expect("call lock")
                .push(specification.clone());
            if let Some(index) = specification.args.iter().position(|arg| arg == "add")
                && specification
                    .args
                    .get(index.wrapping_sub(1))
                    .map(String::as_str)
                    == Some("worktree")
                && let Some(path) = specification.args.get(index + 2)
            {
                std::fs::create_dir_all(path).expect("fake worktree directory");
            }
            self.responses
                .lock()
                .expect("response lock")
                .pop_front()
                .expect("scripted response")
        }
    }

    #[test]
    fn windows_child_paths_drop_verbatim_prefixes() {
        assert_eq!(
            windows_child_path_text(r"\\?\C:\tmp\govfolio"),
            r"C:\tmp\govfolio"
        );
        assert_eq!(
            windows_child_path_text(r"\\?\UNC\server\share\govfolio"),
            r"\\server\share\govfolio"
        );
        assert_eq!(
            windows_child_path_text(r"C:\tmp\govfolio"),
            r"C:\tmp\govfolio"
        );
    }

    #[tokio::test]
    async fn native_resolver_smoke_proves_linked_common_and_real_link() {
        let temp = tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        let scratch = temp.path().join("scratch");
        let common = temp.path().join("common");
        std::fs::create_dir_all(&repo).expect("repo dir");
        std::fs::create_dir_all(&common).expect("common dir");
        let runner = FakeRunner {
            responses: Mutex::new(VecDeque::from([
                ok("abc123\n"),
                ok(&format!("{}\n", common.display())),
                ok(""),
                ok(&format!("{}\n", common.display())),
                ok("codex 1.2.3\n"),
                ok(""),
                ok(&format!("{LINK_MARKER}\n")),
                ok(""),
            ])),
            calls: Mutex::new(Vec::new()),
        };

        let report = run_native_smoke(&runner, &smoke_request(&repo, &scratch))
            .await
            .expect("native smoke passes");

        assert_eq!(report.source_head, "abc123");
        assert_eq!(
            report.git_common_dir,
            common.canonicalize().expect("common")
        );
        assert_eq!(runner.calls.lock().expect("call lock").len(), 8);
    }

    #[tokio::test]
    async fn native_resolver_smoke_only_codex_bad_format_permits_wsl() {
        let temp = tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        let scratch = temp.path().join("scratch");
        let common = temp.path().join("common");
        std::fs::create_dir_all(&repo).expect("repo dir");
        std::fs::create_dir_all(&common).expect("common dir");
        let runner = FakeRunner {
            responses: Mutex::new(VecDeque::from([
                ok("abc123\n"),
                ok(&format!("{}\n", common.display())),
                ok(""),
                ok(&format!("{}\n", common.display())),
                Err(HostCommandError::UnsupportedPlatform),
                ok(""),
            ])),
            calls: Mutex::new(Vec::new()),
        };

        let error = run_native_smoke(&runner, &smoke_request(&repo, &scratch))
            .await
            .expect_err("bad Codex format fails smoke");

        assert!(error.permits_wsl_bootstrap());
        assert_eq!(runner.calls.lock().expect("call lock").len(), 6);
    }

    fn inspection(path: &Path, version: &str, sha256: &str) -> ExecutableInspection {
        ExecutableInspection {
            canonical_path: path.to_path_buf(),
            cli_version: version.to_owned(),
            sha256: sha256.to_owned(),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn ok(stdout: &str) -> Result<HostCommandOutput, HostCommandError> {
        Ok(HostCommandOutput {
            exit_code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    fn smoke_request(repo: &Path, scratch: &Path) -> NativeSmokeRequest {
        NativeSmokeRequest {
            repo: repo.to_path_buf(),
            scratch_root: scratch.to_path_buf(),
            git_executable: PathBuf::from("git"),
            rustc_executable: PathBuf::from("rustc"),
            codex: NativeExecutableIdentity {
                path: PathBuf::from("codex"),
                cli_version: "codex 1.2.3".to_owned(),
                sha256: "abc".to_owned(),
                source: NativeCandidateSource::Path,
            },
        }
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000 + seconds, 0)
            .single()
            .expect("test timestamp")
    }
}
