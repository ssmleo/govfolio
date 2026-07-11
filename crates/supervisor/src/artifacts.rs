use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ulid::Ulid;

const FIVE_GIB: u64 = 5 * 1024 * 1024 * 1024;
const DEFAULT_ROTATION_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_GENERATIONS: usize = 5;
const DEFAULT_RETENTION: Duration = Duration::from_hours(14 * 24);

/// Bounded-artifact defaults from the Release-0 design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactPolicy {
    pub supervisor_rotation_bytes: u64,
    pub supervisor_generations: usize,
    pub normal_retention: Duration,
    pub total_cap_bytes: u64,
}

impl Default for ArtifactPolicy {
    fn default() -> Self {
        Self {
            supervisor_rotation_bytes: DEFAULT_ROTATION_BYTES,
            supervisor_generations: DEFAULT_GENERATIONS,
            normal_retention: DEFAULT_RETENTION,
            total_cap_bytes: FIVE_GIB,
        }
    }
}

/// External runtime directories. Callers choose the state root; no path is
/// derived from the repository worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    root: PathBuf,
    runs: PathBuf,
    attempts: PathBuf,
    blobs: PathBuf,
}

impl RuntimePaths {
    #[must_use]
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            runs: root.join("runs"),
            attempts: root.join("attempts"),
            blobs: root.join("blobs").join("sha256"),
            root,
        }
    }

    #[must_use]
    pub fn from_home(home: impl AsRef<Path>) -> Self {
        Self::new(
            home.as_ref()
                .join(".local")
                .join("state")
                .join("govfolio-loop")
                .join("runtime"),
        )
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn runs(&self) -> &Path {
        &self.runs
    }

    #[must_use]
    pub fn attempts(&self) -> &Path {
        &self.attempts
    }

    #[must_use]
    pub fn blobs(&self) -> &Path {
        &self.blobs
    }

    #[must_use]
    pub fn run_log(&self, run_id: &str) -> PathBuf {
        self.runs.join(run_id).join("supervisor.jsonl")
    }

    fn create(&self) -> io::Result<()> {
        fs::create_dir_all(&self.runs)?;
        fs::create_dir_all(&self.attempts)?;
        fs::create_dir_all(&self.blobs)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptArtifactPolicy {
    Persist,
    Suppressed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptArtifacts {
    directory: PathBuf,
}

impl AttemptArtifacts {
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    #[must_use]
    pub fn attempt_path(&self) -> PathBuf {
        self.directory.join("attempt.json")
    }

    #[must_use]
    pub fn events_path(&self) -> PathBuf {
        self.directory.join("events.jsonl")
    }

    #[must_use]
    pub fn stderr_path(&self) -> PathBuf {
        self.directory.join("stderr.log")
    }

    #[must_use]
    pub fn result_path(&self) -> PathBuf {
        self.directory.join("result.json")
    }

    #[must_use]
    pub fn handoff_path(&self) -> PathBuf {
        self.directory.join("handoff.json")
    }
}

#[derive(Clone, Debug)]
pub struct ArtifactStore {
    paths: RuntimePaths,
    policy: ArtifactPolicy,
}

impl ArtifactStore {
    #[must_use]
    pub fn new(root: impl AsRef<Path>, policy: ArtifactPolicy) -> Self {
        Self {
            paths: RuntimePaths::new(root),
            policy,
        }
    }

    #[must_use]
    pub const fn policy(&self) -> &ArtifactPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    /// Suppressed ticks return before creating even the runtime root.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe attempt identifier or when the external
    /// attempt directory cannot be durably created.
    pub fn begin_attempt(
        &self,
        attempt_id: &str,
        emission: AttemptArtifactPolicy,
    ) -> io::Result<Option<AttemptArtifacts>> {
        if emission == AttemptArtifactPolicy::Suppressed {
            return Ok(None);
        }

        validate_component(attempt_id)?;
        self.paths.create()?;
        let directory = self.paths.attempts.join(attempt_id);
        fs::create_dir(&directory)?;
        sync_parent_directory(&directory)?;
        Ok(Some(AttemptArtifacts { directory }))
    }

    /// Atomically creates one JSON artifact beneath the runtime root.
    ///
    /// # Errors
    ///
    /// Returns an error for paths outside the store, serialization failures,
    /// existing destinations, or filesystem failures.
    pub fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> io::Result<()> {
        if !path.starts_with(self.paths.root()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing to write artifact outside runtime root: {}",
                    path.display()
                ),
            ));
        }
        let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
        atomic_write_new(path, &bytes)
    }

    /// Stores deterministic gzip bytes under the SHA-256 of the original
    /// content. Repeated evidence reuses the existing blob.
    ///
    /// # Errors
    ///
    /// Returns an error when compression, durable storage, or verification of
    /// an existing content-addressed blob fails.
    pub fn write_gzip_blob(&self, content: &[u8]) -> io::Result<BlobReference> {
        self.paths.create()?;
        let digest = hex::encode(Sha256::digest(content));
        let path = self.paths.blobs.join(&digest);

        if !path.exists() {
            let compressed = gzip(content)?;
            match atomic_write_new(&path, &compressed) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }

        validate_blob(&path, &digest)?;
        Ok(BlobReference {
            sha256: digest,
            original_bytes: u64::try_from(content.len()).unwrap_or(u64::MAX),
            stored_bytes: fs::metadata(&path)?.len(),
            path,
        })
    }

    /// Appends and flushes one supervisor event, rotating before the configured
    /// bound would be exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe run identifiers or any durable I/O failure.
    pub fn append_supervisor_event(&self, run_id: &str, event: &[u8]) -> io::Result<PathBuf> {
        validate_component(run_id)?;
        self.paths.create()?;
        let path = self.paths.run_log(run_id);
        let parent = parent_of(&path)?;
        fs::create_dir_all(parent)?;

        let current_bytes = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        if current_bytes > 0
            && current_bytes.saturating_add(u64::try_from(event.len()).unwrap_or(u64::MAX))
                > self.policy.supervisor_rotation_bytes
        {
            rotate_log(&path, self.policy.supervisor_generations)?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(event)?;
        file.sync_all()?;
        sync_parent_directory(&path)?;
        Ok(path)
    }

    /// Applies age and size retention only to resolved normal attempts.
    /// Open evidence remains pinned even when it keeps the store over cap.
    ///
    /// # Errors
    ///
    /// Returns an error when retention duration conversion fails, a candidate
    /// escapes the attempt root, or eligible evidence cannot be removed.
    pub fn prune(
        &self,
        entries: &[RetentionEntry],
        now: DateTime<Utc>,
    ) -> io::Result<RetentionReport> {
        let bytes_before = entries
            .iter()
            .fold(0_u64, |total, entry| total.saturating_add(entry.bytes));
        let mut bytes_after = bytes_before;
        let mut removed = BTreeSet::new();
        let retention =
            chrono::TimeDelta::from_std(self.policy.normal_retention).map_err(io::Error::other)?;

        for entry in entries {
            if entry.disposition == EvidenceDisposition::ResolvedNormal
                && entry.completed_at <= now - retention
                && removed.insert(entry.path.clone())
            {
                bytes_after = bytes_after.saturating_sub(entry.bytes);
            }
        }

        let mut cap_candidates: Vec<&RetentionEntry> = entries
            .iter()
            .filter(|entry| {
                entry.disposition == EvidenceDisposition::ResolvedNormal
                    && !removed.contains(&entry.path)
            })
            .collect();
        cap_candidates.sort_by_key(|entry| entry.completed_at);
        for entry in cap_candidates {
            if bytes_after <= self.policy.total_cap_bytes {
                break;
            }
            if removed.insert(entry.path.clone()) {
                bytes_after = bytes_after.saturating_sub(entry.bytes);
            }
        }

        for path in &removed {
            self.remove_attempt_path(path)?;
        }

        let preserved = entries
            .iter()
            .filter(|entry| entry.disposition.must_preserve())
            .map(|entry| entry.path.clone())
            .collect();
        Ok(RetentionReport {
            removed: removed.into_iter().collect(),
            preserved,
            bytes_before,
            bytes_after,
        })
    }

    /// Evaluates the launch gate against caller-available filesystem space.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime root cannot be created or filesystem
    /// capacity cannot be read from the operating system.
    pub fn check_disk_space(&self) -> io::Result<DiskGate> {
        self.paths.create()?;
        let (total, available) = filesystem_space(self.paths.root())?;
        Ok(disk_gate_from_space(total, available))
    }

    fn remove_attempt_path(&self, path: &Path) -> io::Result<()> {
        if !safe_attempt_path(self.paths.attempts(), path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing to prune path outside attempt store: {}",
                    path.display()
                ),
            ));
        }
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else if path.exists() {
            fs::remove_file(path)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlobReference {
    pub sha256: String,
    pub path: PathBuf,
    pub original_bytes: u64,
    pub stored_bytes: u64,
}

/// A control-store bucket may point at exactly one complete blob exemplar.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExemplarReference {
    pub fingerprint: String,
    pub attempt_id: String,
    pub blob_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDisposition {
    ResolvedNormal,
    Unresolved,
    Ambiguous,
    Conflict,
    Unapplied,
}

impl EvidenceDisposition {
    #[must_use]
    pub const fn must_preserve(self) -> bool {
        !matches!(self, Self::ResolvedNormal)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionEntry {
    pub path: PathBuf,
    pub completed_at: DateTime<Utc>,
    pub bytes: u64,
    pub disposition: EvidenceDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionReport {
    pub removed: Vec<PathBuf>,
    pub preserved: Vec<PathBuf>,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiskGate {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub required_bytes: u64,
    pub has_capacity: bool,
}

#[must_use]
pub fn disk_gate_from_space(total_bytes: u64, available_bytes: u64) -> DiskGate {
    let required_bytes = FIVE_GIB.max(total_bytes / 10);
    DiskGate {
        total_bytes,
        available_bytes,
        required_bytes,
        has_capacity: available_bytes >= required_bytes,
    }
}

/// Redacts only human-facing summaries. Complete raw evidence remains in the
/// content-addressed restricted artifact store.
///
/// # Errors
///
/// Returns an error if a built-in redaction expression is invalid.
pub fn redact_human_summary(
    summary: &str,
    environment: &[(String, String)],
) -> Result<String, regex::Error> {
    let mut redacted = summary.to_owned();
    for (name, value) in environment {
        if secret_shaped_name(name) && !value.is_empty() {
            redacted = redacted.replace(value, "[REDACTED]");
        }
    }

    let authorization = Regex::new(r"(?im)(authorization\s*:\s*)(?:bearer\s+)?[^\s,\r\n]+")?;
    redacted = authorization
        .replace_all(&redacted, "$1[REDACTED]")
        .into_owned();

    let assignments = Regex::new(
        r#"(?i)(\b(?:access[_-]?token|api[_-]?key|secret|password|passwd|database_url|connection_string)\b\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;&]+)"#,
    )?;
    redacted = assignments
        .replace_all(&redacted, "$1[REDACTED]")
        .into_owned();

    let connection =
        Regex::new(r"(?i)\b((?:postgres(?:ql)?|mysql|mssql|mongodb(?:\+srv)?|redis)://)[^@\s]+@")?;
    Ok(connection
        .replace_all(&redacted, "$1[REDACTED]@")
        .into_owned())
}

/// Creates an artifact through a same-directory temporary file, full file
/// synchronization, and atomic rename.
///
/// # Errors
///
/// Returns an error when the destination has no existing parent, already
/// exists, or any write, sync, or rename operation fails.
pub fn atomic_write_new(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = parent_of(destination)?;
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("artifact parent does not exist: {}", parent.display()),
        ));
    }
    if destination.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("artifact already exists: {}", destination.display()),
        ));
    }

    let mut temporary_name = destination
        .file_name()
        .map_or_else(|| OsString::from("artifact"), OsString::from);
    temporary_name.push(format!(".{}.tmp", Ulid::new()));
    let temporary = parent.join(temporary_name);

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, destination)?;
        sync_parent_directory(destination)
    })();

    if result.is_err() {
        let _ignored = fs::remove_file(&temporary);
    }
    result
}

fn gzip(content: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    encoder.write_all(content)?;
    encoder.finish()
}

fn validate_blob(path: &Path, expected_digest: &str) -> io::Result<()> {
    let file = File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = decoder.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = hex::encode(digest.finalize());
    if actual == expected_digest {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "content-addressed blob digest mismatch at {}",
                path.display()
            ),
        ))
    }
}

fn validate_component(value: &str) -> io::Result<()> {
    let valid = !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsafe artifact identifier: {value:?}"),
        ))
    }
}

fn safe_attempt_path(root: &Path, target: &Path) -> bool {
    target != root
        && target.starts_with(root)
        && !target
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

fn parent_of(path: &Path) -> io::Result<&Path> {
    path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })
}

fn rotate_log(path: &Path, generations: usize) -> io::Result<()> {
    if generations == 0 {
        fs::remove_file(path)?;
        return Ok(());
    }

    let oldest = generation_path(path, generations);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }
    for generation in (1..generations).rev() {
        let source = generation_path(path, generation);
        if source.exists() {
            fs::rename(source, generation_path(path, generation + 1))?;
        }
    }
    fs::rename(path, generation_path(path, 1))?;
    sync_parent_directory(path)
}

fn generation_path(path: &Path, generation: usize) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("supervisor.jsonl"), OsString::from);
    name.push(format!(".{generation}"));
    path.with_file_name(name)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    File::open(parent_of(path)?)?.sync_all()
}

#[cfg(windows)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    // Rust's standard library cannot open NTFS directory handles with the
    // backup-semantics flag. The file itself is flushed before every rename.
    let _metadata = fs::metadata(parent_of(path)?)?;
    Ok(())
}

fn secret_shaped_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "DATABASE_URL",
        "CONNECTION_STRING",
        "PRIVATE_KEY",
        "AUTHORIZATION",
    ]
    .iter()
    .any(|fragment| upper.contains(fragment))
}

#[cfg(windows)]
fn filesystem_space(path: &Path) -> io::Result<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut free = 0_u64;
    // SAFETY: `wide` is NUL-terminated and all output pointers refer to valid
    // stack-owned `u64` values for the duration of the call.
    let succeeded = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &raw mut available,
            &raw mut total,
            &raw mut free,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok((total, available))
    }
}

#[cfg(unix)]
fn filesystem_space(path: &Path) -> io::Result<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "filesystem path contains an interior NUL",
        )
    })?;
    // SAFETY: `statvfs` is a plain C aggregate and all-zero is a valid
    // initialization before the operating system fills every field.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is NUL-terminated and `stats` is a valid writable pointer.
    let succeeded = unsafe { libc::statvfs(path.as_ptr(), &mut stats) };
    if succeeded != 0 {
        return Err(io::Error::last_os_error());
    }
    let fragment_size = stats.f_frsize;
    Ok((
        stats.f_blocks.saturating_mul(fragment_size),
        stats.f_bavail.saturating_mul(fragment_size),
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;
    use std::time::Duration;

    use chrono::{TimeDelta, Utc};
    use flate2::read::GzDecoder;
    use tempfile::tempdir;

    use super::{
        ArtifactPolicy, ArtifactStore, AttemptArtifactPolicy, EvidenceDisposition, RetentionEntry,
        disk_gate_from_space, redact_human_summary,
    };

    #[test]
    fn suppressed_attempt_should_not_create_runtime_root() {
        let temp = tempdir().expect("tempdir should be available");
        let root = temp.path().join("runtime");
        let store = ArtifactStore::new(&root, ArtifactPolicy::default());

        let attempt = store
            .begin_attempt("attempt-1", AttemptArtifactPolicy::Suppressed)
            .expect("suppression should not fail");

        assert!(attempt.is_none());
        assert!(!root.exists());
    }

    #[test]
    fn persisted_attempt_should_create_only_the_documented_layout() {
        let temp = tempdir().expect("tempdir should be available");
        let store = ArtifactStore::new(temp.path(), ArtifactPolicy::default());

        let attempt = store
            .begin_attempt("attempt-1", AttemptArtifactPolicy::Persist)
            .expect("attempt directory should be created")
            .expect("persisted attempts have paths");

        assert!(attempt.directory().is_dir());
        assert_eq!(
            attempt.events_path(),
            attempt.directory().join("events.jsonl")
        );
        assert_eq!(
            attempt.stderr_path(),
            attempt.directory().join("stderr.log")
        );
        assert!(store.paths().runs().is_dir());
        assert!(store.paths().blobs().is_dir());
    }

    #[test]
    fn atomic_write_should_leave_complete_content_and_no_temporary_file() {
        let temp = tempdir().expect("tempdir should be available");
        let destination = temp.path().join("result.json");

        super::atomic_write_new(&destination, br#"{"class":"completed"}"#)
            .expect("atomic write should succeed");

        assert_eq!(
            fs::read(&destination).expect("result should be readable"),
            br#"{"class":"completed"}"#
        );
        assert_eq!(
            fs::read_dir(temp.path())
                .expect("directory should be readable")
                .count(),
            1
        );
    }

    #[test]
    fn gzip_blob_should_be_content_addressed_and_deduplicated() {
        let temp = tempdir().expect("tempdir should be available");
        let store = ArtifactStore::new(temp.path(), ArtifactPolicy::default());
        let evidence = b"same complete provider transcript";

        let first = store
            .write_gzip_blob(evidence)
            .expect("first blob write should succeed");
        let second = store
            .write_gzip_blob(evidence)
            .expect("duplicate blob write should succeed");

        assert_eq!(first, second);
        assert_eq!(
            fs::read_dir(store.paths().blobs())
                .expect("blob directory should be readable")
                .count(),
            1
        );

        let mut decoded = Vec::new();
        GzDecoder::new(fs::File::open(&first.path).expect("blob should exist"))
            .read_to_end(&mut decoded)
            .expect("blob should be valid gzip");
        assert_eq!(decoded, evidence);
    }

    #[test]
    fn human_summary_should_redact_credentials_and_secret_environment_values() {
        let summary = "Authorization: Bearer provider-token\nDATABASE_URL=postgres://user:pass@db/govfolio\napi_key=abc123\nopaque-value";
        let secrets = vec![("CUSTOM_SECRET".to_owned(), "opaque-value".to_owned())];

        let redacted =
            redact_human_summary(summary, &secrets).expect("configured redactors should compile");

        assert!(!redacted.contains("provider-token"));
        assert!(!redacted.contains("user:pass"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("opaque-value"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn supervisor_log_should_rotate_to_five_generations() {
        let temp = tempdir().expect("tempdir should be available");
        let policy = ArtifactPolicy {
            supervisor_rotation_bytes: 4,
            supervisor_generations: 5,
            ..ArtifactPolicy::default()
        };
        let store = ArtifactStore::new(temp.path(), policy);

        for index in 0..7 {
            store
                .append_supervisor_event("run-1", format!("{index:04}\n").as_bytes())
                .expect("supervisor event should be durable");
        }

        let log = store.paths().run_log("run-1");
        assert!(log.exists());
        for generation in 1..=5 {
            assert!(super::generation_path(&log, generation).exists());
        }
        assert!(!super::generation_path(&log, 6).exists());
    }

    #[test]
    fn retention_should_delete_expired_normal_evidence_but_preserve_open_evidence() {
        let temp = tempdir().expect("tempdir should be available");
        let policy = ArtifactPolicy {
            normal_retention: Duration::from_hours(14 * 24),
            total_cap_bytes: 100,
            ..ArtifactPolicy::default()
        };
        let store = ArtifactStore::new(temp.path(), policy);
        let now = Utc::now();
        let expired = make_attempt(&store, "expired");
        let unresolved = make_attempt(&store, "unresolved");
        let entries = vec![
            RetentionEntry {
                path: expired.clone(),
                completed_at: now - TimeDelta::days(15),
                bytes: 10,
                disposition: EvidenceDisposition::ResolvedNormal,
            },
            RetentionEntry {
                path: unresolved.clone(),
                completed_at: now - TimeDelta::days(30),
                bytes: 200,
                disposition: EvidenceDisposition::Unresolved,
            },
        ];

        let report = store
            .prune(&entries, now)
            .expect("retention should only remove eligible evidence");

        assert_eq!(report.removed, vec![expired.clone()]);
        assert!(!expired.exists());
        assert!(unresolved.exists());
        assert!(report.bytes_after > store.policy().total_cap_bytes);
    }

    #[test]
    fn retention_cap_should_remove_oldest_resolved_normal_first() {
        let temp = tempdir().expect("tempdir should be available");
        let policy = ArtifactPolicy {
            normal_retention: Duration::from_hours(365 * 24),
            total_cap_bytes: 15,
            ..ArtifactPolicy::default()
        };
        let store = ArtifactStore::new(temp.path(), policy);
        let now = Utc::now();
        let old = make_attempt(&store, "old");
        let new = make_attempt(&store, "new");
        let entries = vec![
            RetentionEntry {
                path: new.clone(),
                completed_at: now - TimeDelta::hours(1),
                bytes: 10,
                disposition: EvidenceDisposition::ResolvedNormal,
            },
            RetentionEntry {
                path: old.clone(),
                completed_at: now - TimeDelta::hours(2),
                bytes: 10,
                disposition: EvidenceDisposition::ResolvedNormal,
            },
        ];

        let report = store
            .prune(&entries, now)
            .expect("cap pruning should succeed");

        assert_eq!(report.removed, vec![old.clone()]);
        assert!(!old.exists());
        assert!(new.exists());
        assert_eq!(report.bytes_after, 10);
    }

    #[test]
    fn disk_gate_should_require_the_greater_of_five_gib_and_ten_percent() {
        let five_gib = 5 * 1024 * 1024 * 1024;

        let small_volume = disk_gate_from_space(20 * 1024 * 1024 * 1024, five_gib - 1);
        let large_volume = disk_gate_from_space(100 * 1024 * 1024 * 1024, 9 * 1024 * 1024 * 1024);

        assert_eq!(small_volume.required_bytes, five_gib);
        assert!(!small_volume.has_capacity);
        assert_eq!(large_volume.required_bytes, 10 * 1024 * 1024 * 1024);
        assert!(!large_volume.has_capacity);
    }

    fn make_attempt(store: &ArtifactStore, attempt_id: &str) -> std::path::PathBuf {
        store
            .begin_attempt(attempt_id, AttemptArtifactPolicy::Persist)
            .expect("attempt creation should succeed")
            .expect("attempt should be persisted")
            .directory()
            .to_path_buf()
    }
}
