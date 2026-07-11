use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use tokio::process::Command;

use crate::model::ProviderIdentity;

const FIVE_GIB: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ProbeOutcome {
    Pass {
        fingerprint: String,
    },
    Wait {
        reason: String,
        retry_at: DateTime<Utc>,
    },
    Recover {
        reason: String,
    },
    Block {
        reason: String,
        until_fingerprint_changes: bool,
    },
}

impl ProbeOutcome {
    #[must_use]
    pub const fn is_pass(&self) -> bool {
        matches!(self, Self::Pass { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProbeRecord {
    pub name: String,
    pub outcome: ProbeOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreflightReport {
    pub records: Vec<ProbeRecord>,
    pub signature: String,
}

impl PreflightReport {
    #[must_use]
    pub fn all_pass(&self) -> bool {
        self.records.iter().all(|record| record.outcome.is_pass())
    }

    #[must_use]
    pub fn terminal_outcome(&self) -> Option<&ProbeOutcome> {
        self.records
            .iter()
            .find(|record| !record.outcome.is_pass())
            .map(|record| &record.outcome)
    }
}

#[async_trait]
pub trait Probe: Send + Sync {
    fn name(&self) -> &'static str;
    async fn check(&self, now: DateTime<Utc>) -> ProbeOutcome;
}

/// Ordered zero-spend checks. The first non-pass result stops evaluation so a
/// known-open gate cannot trigger more expensive diagnostics.
pub struct PreflightSuite {
    probes: Vec<Arc<dyn Probe>>,
}

impl PreflightSuite {
    #[must_use]
    pub fn new(probes: Vec<Arc<dyn Probe>>) -> Self {
        Self { probes }
    }

    pub async fn run(&self, now: DateTime<Utc>) -> PreflightReport {
        let mut records = Vec::new();
        for probe in &self.probes {
            let outcome = probe.check(now).await;
            let passed = outcome.is_pass();
            records.push(ProbeRecord {
                name: probe.name().to_owned(),
                outcome,
            });
            if !passed {
                break;
            }
        }
        let signature = report_signature(&records);
        PreflightReport { records, signature }
    }

    #[cfg(test)]
    pub(crate) fn probe_names(&self) -> Vec<&'static str> {
        self.probes.iter().map(|probe| probe.name()).collect()
    }
}

fn report_signature(records: &[ProbeRecord]) -> String {
    let bytes = serde_json::to_vec(records).unwrap_or_else(|_| b"serialization-error".to_vec());
    hex::encode(Sha256::digest(bytes))
}

pub struct AuthorityProbe {
    pub binary: PathBuf,
    pub repo: PathBuf,
}

#[async_trait]
impl Probe for AuthorityProbe {
    fn name(&self) -> &'static str {
        "authority"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        match command_output(&self.binary, &["--ci"], &self.repo, Duration::from_secs(30)).await {
            Ok(output) if output.status.success() => pass_hash(&output.stdout),
            Ok(output) => ProbeOutcome::Block {
                reason: bounded_detail("authority validator red", &output.stderr),
                until_fingerprint_changes: false,
            },
            Err(error) => block_error("authority validator unavailable", &error, false),
        }
    }
}

pub struct SkillContractProbe {
    pub node: PathBuf,
    pub worktree: PathBuf,
}

#[async_trait]
impl Probe for SkillContractProbe {
    fn name(&self) -> &'static str {
        "codex_skill_contract"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        let root = self.worktree.to_string_lossy().into_owned();
        let renderer = self
            .worktree
            .join("scripts")
            .join("agents")
            .join("render-codex-contract.mjs");
        let validator = self
            .worktree
            .join("scripts")
            .join("agents")
            .join("validate-codex-contract.mjs");
        let renderer = renderer.to_string_lossy().into_owned();
        let validator = validator.to_string_lossy().into_owned();
        let render_args = [renderer.as_str(), "--check", "--repo-root", root.as_str()];
        let render = match command_output(
            &self.node,
            &render_args,
            &self.worktree,
            Duration::from_mins(1),
        )
        .await
        {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                return ProbeOutcome::Block {
                    reason: bounded_detail("Codex skill projection drift", &output.stderr),
                    until_fingerprint_changes: false,
                };
            }
            Err(error) => {
                return block_error("Codex skill renderer unavailable", &error, false);
            }
        };
        let validate_args = [validator.as_str(), "--repo-root", root.as_str()];
        match command_output(
            &self.node,
            &validate_args,
            &self.worktree,
            Duration::from_mins(1),
        )
        .await
        {
            Ok(output) if output.status.success() => {
                let mut evidence = render.stdout;
                evidence.extend_from_slice(&output.stdout);
                pass_hash(&evidence)
            }
            Ok(output) => ProbeOutcome::Block {
                reason: bounded_detail("Codex skill contract invalid", &output.stderr),
                until_fingerprint_changes: false,
            },
            Err(error) => block_error("Codex skill validator unavailable", &error, false),
        }
    }
}

pub struct GitProbe {
    pub worktree: PathBuf,
    pub expected_branch: String,
    /// Recovery turns may inspect and repair a dirty worktree, but active Git
    /// operations and identity/branch mismatches remain hard failures.
    pub allow_dirty: bool,
}

#[async_trait]
impl Probe for GitProbe {
    fn name(&self) -> &'static str {
        "git_worktree"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        match check_git(&self.worktree, &self.expected_branch).await {
            Ok(GitState::Clean(fingerprint)) => ProbeOutcome::Pass { fingerprint },
            Ok(GitState::Dirty(detail)) if self.allow_dirty => pass_hash(detail.as_bytes()),
            Ok(GitState::Dirty(detail)) => ProbeOutcome::Recover { reason: detail },
            Err(error) => block_error("git preflight failed", &error, false),
        }
    }
}

enum GitState {
    Clean(String),
    Dirty(String),
}

async fn check_git(worktree: &Path, expected_branch: &str) -> anyhow::Result<GitState> {
    let top = git_text(worktree, &["rev-parse", "--show-toplevel"]).await?;
    let canonical_top = std::fs::canonicalize(top.trim())?;
    let canonical_expected = std::fs::canonicalize(worktree)?;
    if canonical_top != canonical_expected {
        anyhow::bail!(
            "registered worktree mismatch: expected {}, git reports {}",
            canonical_expected.display(),
            canonical_top.display()
        );
    }

    let branch = git_text(worktree, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    if branch.trim() != expected_branch {
        anyhow::bail!(
            "expected branch {expected_branch:?}, found {:?}",
            branch.trim()
        );
    }

    let git_dir = git_text(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-dir"],
    )
    .await?;
    let git_dir = PathBuf::from(git_dir.trim());
    for marker in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
        if git_dir.join(marker).exists() {
            anyhow::bail!("active git operation marker: {marker}");
        }
    }
    for marker in ["rebase-apply", "rebase-merge"] {
        if git_dir.join(marker).exists() {
            anyhow::bail!("active git operation directory: {marker}");
        }
    }

    let common = git_text(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await?;
    let common = PathBuf::from(common.trim());
    writable_directory_probe(&common, "git-common")?;

    let status = git_bytes(worktree, &["status", "--porcelain=v1", "-z"]).await?;
    if !status.is_empty() {
        return Ok(GitState::Dirty(format!(
            "worktree has {} porcelain bytes and requires recovery",
            status.len()
        )));
    }
    let head = git_text(worktree, &["rev-parse", "HEAD"]).await?;
    let fingerprint = hex::encode(Sha256::digest(
        format!("{}\n{}\n{}", head.trim(), branch.trim(), common.display()).as_bytes(),
    ));
    Ok(GitState::Clean(fingerprint))
}

pub struct ProviderCliProbe {
    pub identity: ProviderIdentity,
    pub worktree: PathBuf,
}

#[async_trait]
impl Probe for ProviderCliProbe {
    fn name(&self) -> &'static str {
        "provider_cli"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        match command_output(
            &self.identity.executable,
            &["--version"],
            &self.worktree,
            Duration::from_secs(15),
        )
        .await
        {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                if version.contains(&self.identity.cli_version) {
                    ProbeOutcome::Pass {
                        fingerprint: self.identity.config_fingerprint.clone(),
                    }
                } else {
                    ProbeOutcome::Block {
                        reason: "provider CLI version changed; compatibility probe required"
                            .to_owned(),
                        until_fingerprint_changes: true,
                    }
                }
            }
            Ok(output) => ProbeOutcome::Block {
                reason: bounded_detail("provider CLI unusable", &output.stderr),
                until_fingerprint_changes: true,
            },
            Err(error) => block_error("provider CLI unavailable", &error, true),
        }
    }
}

pub struct CompilerProbe {
    pub rustc: PathBuf,
    pub cache_dir: PathBuf,
}

#[async_trait]
impl Probe for CompilerProbe {
    fn name(&self) -> &'static str {
        "compiler_link"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        match compiler_link_canary(&self.rustc, &self.cache_dir).await {
            Ok(fingerprint) => ProbeOutcome::Pass { fingerprint },
            Err(error) => block_error("compiler/linker canary failed", &error, true),
        }
    }
}

async fn compiler_link_canary(rustc: &Path, cache_dir: &Path) -> anyhow::Result<String> {
    let cwd = std::env::current_dir()?;
    let version = command_output(rustc, &["-vV"], &cwd, Duration::from_secs(20)).await?;
    if !version.status.success() {
        anyhow::bail!("rustc -vV exited {}", version.status);
    }
    let mut material = version.stdout;
    for key in ["PATH", "RUSTFLAGS", "CARGO_BUILD_TARGET"] {
        material.extend_from_slice(key.as_bytes());
        material.extend_from_slice(
            std::env::var_os(key)
                .unwrap_or_default()
                .to_string_lossy()
                .as_bytes(),
        );
    }
    let fingerprint = hex::encode(Sha256::digest(&material));
    let dir = cache_dir.join("link-canary").join(&fingerprint);
    let marker = dir.join("passed");
    if marker.is_file() {
        return Ok(fingerprint);
    }
    std::fs::create_dir_all(&dir)?;
    let source = dir.join("main.rs");
    let executable = dir.join(if cfg!(windows) {
        "canary.exe"
    } else {
        "canary"
    });
    std::fs::write(
        &source,
        b"fn main() { println!(\"govfolio-link-canary\"); }\n",
    )?;
    let source_arg = source.to_string_lossy().into_owned();
    let output_arg = executable.to_string_lossy().into_owned();
    let args = [source_arg.as_str(), "-o", output_arg.as_str()];
    let output = command_output(rustc, &args, &dir, Duration::from_mins(2)).await?;
    if !output.status.success() {
        anyhow::bail!("{}", bounded_detail("link failed", &output.stderr));
    }
    let run = command_output(&executable, &[], &dir, Duration::from_secs(10)).await?;
    if !run.status.success() || run.stdout != b"govfolio-link-canary\n" {
        anyhow::bail!("linked canary did not execute with the expected output");
    }
    atomic_marker(&marker, fingerprint.as_bytes())?;
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
    Ok(fingerprint)
}

pub struct DataProbe {
    pub database_url: String,
    pub bronze_root: PathBuf,
}

pub struct RuntimeSeparationProbe {
    pub bronze_root: PathBuf,
    pub protected_paths: Vec<PathBuf>,
}

#[async_trait]
impl Probe for RuntimeSeparationProbe {
    fn name(&self) -> &'static str {
        "runtime_separation"
    }

    async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
        if let Err(error) = std::fs::create_dir_all(&self.bronze_root) {
            return block_error("Bronze root cannot be prepared", &error.into(), false);
        }
        let bronze = match std::fs::canonicalize(&self.bronze_root) {
            Ok(path) => path,
            Err(error) => {
                return block_error("Bronze root cannot be resolved", &error.into(), false);
            }
        };
        let mut material = bronze.to_string_lossy().into_owned();
        for protected in &self.protected_paths {
            let protected = match std::fs::canonicalize(protected) {
                Ok(path) => path,
                Err(error) => {
                    return block_error(
                        "protected runtime path cannot be resolved",
                        &error.into(),
                        false,
                    );
                }
            };
            if protected.starts_with(&bronze) || bronze.starts_with(&protected) {
                return ProbeOutcome::Block {
                    reason: format!(
                        "Bronze root and protected runtime path overlap: {}",
                        protected.display()
                    ),
                    until_fingerprint_changes: false,
                };
            }
            material.push('\0');
            material.push_str(&protected.to_string_lossy());
        }
        pass_hash(material.as_bytes())
    }
}

#[async_trait]
impl Probe for DataProbe {
    fn name(&self) -> &'static str {
        "data_stores"
    }

    async fn check(&self, now: DateTime<Utc>) -> ProbeOutcome {
        if let Err(error) = writable_directory_probe(&self.bronze_root, "bronze") {
            return block_error("Bronze root unavailable", &error.into(), false);
        }
        let connect = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&self.database_url);
        let pool = match tokio::time::timeout(Duration::from_secs(8), connect).await {
            Ok(Ok(pool)) => pool,
            Ok(Err(error)) => {
                return ProbeOutcome::Wait {
                    reason: format!(
                        "product DB unavailable: {}",
                        error
                            .as_database_error()
                            .map_or("connection failed", |_| "database error")
                    ),
                    retry_at: now + TimeDelta::minutes(5),
                };
            }
            Err(_) => {
                return ProbeOutcome::Wait {
                    reason: "product DB connection timed out".to_owned(),
                    retry_at: now + TimeDelta::minutes(5),
                };
            }
        };
        let probe = tokio::time::timeout(
            Duration::from_secs(5),
            sqlx::query_scalar::<_, i32>("select 1").fetch_one(&pool),
        )
        .await;
        pool.close().await;
        match probe {
            Ok(Ok(1)) => pass_hash(self.bronze_root.to_string_lossy().as_bytes()),
            Ok(Ok(_)) => ProbeOutcome::Block {
                reason: "product DB returned an impossible health value".to_owned(),
                until_fingerprint_changes: false,
            },
            Ok(Err(_)) | Err(_) => ProbeOutcome::Wait {
                reason: "product DB health query failed".to_owned(),
                retry_at: now + TimeDelta::minutes(5),
            },
        }
    }
}

pub struct FactoryProbe {
    pub epoch_gate: PathBuf,
    pub lease_bin: PathBuf,
    pub worktree: PathBuf,
    pub epoch: String,
    pub lane_id: String,
}

#[async_trait]
impl Probe for FactoryProbe {
    fn name(&self) -> &'static str {
        "factory_gate_and_claimable"
    }

    async fn check(&self, now: DateTime<Utc>) -> ProbeOutcome {
        let epoch_args = [self.epoch.as_str()];
        match command_output(
            &self.epoch_gate,
            &epoch_args,
            &self.worktree,
            Duration::from_mins(15),
        )
        .await
        {
            Ok(output) if output.status.success() => {}
            Ok(_) => {
                return ProbeOutcome::Wait {
                    reason: format!("epoch {} is red", self.epoch),
                    retry_at: now + TimeDelta::hours(1),
                };
            }
            Err(error) => return block_error("epoch gate unavailable", &error, true),
        }
        let args = ["claimable", "--epoch", self.epoch.as_str()];
        let mut command = Command::new(&self.lease_bin);
        command
            .args(args)
            .current_dir(&self.worktree)
            .env("GOVFOLIO_LANE_ID", &self.lane_id)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
            Ok(Ok(output)) if output.status.success() => pass_hash(&output.stdout),
            Ok(Ok(output))
                if output.status.code() == Some(1) && output.stdout.starts_with(b"none") =>
            {
                ProbeOutcome::Wait {
                    reason: format!("no claimable rows for {}", self.epoch),
                    retry_at: now + TimeDelta::hours(1),
                }
            }
            Ok(Ok(output)) => ProbeOutcome::Wait {
                reason: bounded_detail("claimability probe failed", &output.stderr),
                retry_at: now + TimeDelta::minutes(5),
            },
            Ok(Err(error)) => block_error("claimability binary unavailable", &error.into(), true),
            Err(_) => ProbeOutcome::Wait {
                reason: "claimability probe timed out".to_owned(),
                retry_at: now + TimeDelta::minutes(5),
            },
        }
    }
}

pub struct DiskProbe {
    pub runtime_root: PathBuf,
}

#[async_trait]
impl Probe for DiskProbe {
    fn name(&self) -> &'static str {
        "disk"
    }

    async fn check(&self, now: DateTime<Utc>) -> ProbeOutcome {
        match disk_capacity(&self.runtime_root) {
            Ok((available, required)) if available >= required => ProbeOutcome::Pass {
                fingerprint: format!("available={available};required={required}"),
            },
            Ok((available, required)) => ProbeOutcome::Wait {
                reason: format!("disk pressure: {available} bytes available, {required} required"),
                retry_at: now + TimeDelta::minutes(15),
            },
            Err(error) => block_error("disk capacity unavailable", &error, false),
        }
    }
}

fn disk_capacity(path: &Path) -> anyhow::Result<(u64, u64)> {
    std::fs::create_dir_all(path)?;
    let (available, total) = filesystem_space(path)?;
    Ok((available, FIVE_GIB.max(total / 10)))
}

#[cfg(windows)]
fn filesystem_space(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut free = 0_u64;
    // SAFETY: all pointers reference valid writable u64 values and `wide` is
    // NUL-terminated for the duration of the call.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &raw mut available,
            &raw mut total,
            &raw mut free,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok((available, total))
    }
}

#[cfg(unix)]
fn filesystem_space(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt as _;

    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stats` points to writable storage.
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: statvfs returned success and initialized the structure.
    let stats = unsafe { stats.assume_init() };
    let block = stats.f_frsize;
    Ok((
        stats.f_bavail.saturating_mul(block),
        stats.f_blocks.saturating_mul(block),
    ))
}

async fn git_text(worktree: &Path, args: &[&str]) -> anyhow::Result<String> {
    Ok(String::from_utf8(git_bytes(worktree, args).await?)?)
}

async fn git_bytes(worktree: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = command_output(Path::new("git"), args, worktree, Duration::from_secs(30)).await?;
    if !output.status.success() {
        anyhow::bail!("{}", bounded_detail("git command failed", &output.stderr));
    }
    Ok(output.stdout)
}

async fn command_output(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> anyhow::Result<std::process::Output> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match tokio::time::timeout(timeout, command.output()).await {
        Ok(result) => Ok(result?),
        Err(_) => anyhow::bail!("command timed out after {}s", timeout.as_secs()),
    }
}

fn writable_directory_probe(path: &Path, label: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    let probe = path.join(format!(".govfolio-loop-{label}-{}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    let file = options.open(&probe)?;
    file.sync_all()?;
    drop(file);
    std::fs::remove_file(probe)
}

fn atomic_marker(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("marker path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".marker-{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options.open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temp, path)
}

fn pass_hash(bytes: &[u8]) -> ProbeOutcome {
    ProbeOutcome::Pass {
        fingerprint: hex::encode(Sha256::digest(bytes)),
    }
}

fn block_error(
    prefix: &str,
    error: &anyhow::Error,
    until_fingerprint_changes: bool,
) -> ProbeOutcome {
    ProbeOutcome::Block {
        reason: format!("{prefix}: {error:#}"),
        until_fingerprint_changes,
    }
}

fn bounded_detail(prefix: &str, bytes: &[u8]) -> String {
    const MAX: usize = 2_048;
    let start = bytes.len().saturating_sub(MAX);
    let detail = String::from_utf8_lossy(&bytes[start..]);
    format!("{prefix}: {}", detail.trim())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tempfile::tempdir;

    use super::*;

    struct StaticProbe {
        name: &'static str,
        outcome: ProbeOutcome,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Probe for StaticProbe {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn check(&self, _now: DateTime<Utc>) -> ProbeOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcome.clone()
        }
    }

    #[tokio::test]
    async fn preflight_stops_after_first_non_pass() {
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let suite = PreflightSuite::new(vec![
            Arc::new(StaticProbe {
                name: "red",
                outcome: ProbeOutcome::Block {
                    reason: "missing compiler".to_owned(),
                    until_fingerprint_changes: true,
                },
                calls: Arc::clone(&first),
            }),
            Arc::new(StaticProbe {
                name: "paid-adjacent",
                outcome: ProbeOutcome::Pass {
                    fingerprint: "should-not-run".to_owned(),
                },
                calls: Arc::clone(&second),
            }),
        ]);

        let report = suite.run(Utc::now()).await;
        assert!(!report.all_pass());
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 0);
        assert_eq!(report.records.len(), 1);
    }

    #[test]
    fn disk_threshold_is_larger_of_five_gib_and_ten_percent() {
        assert_eq!(FIVE_GIB.max(20 * 1024 * 1024 * 1024 / 10), FIVE_GIB);
        assert_eq!(
            FIVE_GIB.max(100 * 1024 * 1024 * 1024 / 10),
            10 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn report_signature_is_stable() {
        let records = vec![ProbeRecord {
            name: "one".to_owned(),
            outcome: ProbeOutcome::Pass {
                fingerprint: "abc".to_owned(),
            },
        }];
        assert_eq!(report_signature(&records), report_signature(&records));
    }

    #[tokio::test]
    async fn runtime_separation_rejects_bronze_overlapping_trusted_binaries() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("target");
        let binary = target.join("debug").join("govfolio-loop");
        fs::create_dir_all(binary.parent().expect("binary parent")).expect("target directory");
        fs::write(&binary, b"fixture").expect("fixture binary");
        let overlapping = RuntimeSeparationProbe {
            bronze_root: target,
            protected_paths: vec![binary],
        };

        assert!(matches!(
            overlapping.check(Utc::now()).await,
            ProbeOutcome::Block { .. }
        ));
    }

    #[tokio::test]
    async fn runtime_separation_accepts_dedicated_bronze_root() {
        let temp = tempdir().expect("tempdir");
        let worktree = temp.path().join("worktree");
        let binary = worktree.join("target/debug/govfolio-loop");
        fs::create_dir_all(binary.parent().expect("binary parent")).expect("target directory");
        fs::write(&binary, b"fixture").expect("fixture binary");
        let separated = RuntimeSeparationProbe {
            bronze_root: temp.path().join("bronze"),
            protected_paths: vec![worktree, binary],
        };

        assert!(separated.check(Utc::now()).await.is_pass());
    }
}
