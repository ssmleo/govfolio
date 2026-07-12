use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

pub const POLICY_PATH: &str = "docs/decisions/build-performance-policy.md";
const AUTHORITY_LOCK_PATH: &str = "agents/AUTHORITY.lock.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPolicyStatus {
    Advisory,
    Shadow,
    Enforced,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildPolicySnapshot {
    pub schema_version: u32,
    pub policy_sha256: String,
    pub status: BuildPolicyStatus,
    pub source_commit: String,
    pub loaded_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum BuildPolicyError {
    #[error("canonical build policy I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("canonical build policy is not UTF-8")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("canonical build policy front matter is invalid: {0}")]
    FrontMatter(String),
    #[error("canonical build policy authority lock is invalid: {0}")]
    Authority(#[from] serde_json::Error),
    #[error("canonical build policy trust check failed: {0}")]
    Trust(String),
}

#[derive(Deserialize)]
struct AuthorityLock {
    pinned: BTreeMap<String, String>,
}

pub fn load_build_policy(
    repository: &Path,
    loaded_at: DateTime<Utc>,
) -> Result<BuildPolicySnapshot, BuildPolicyError> {
    let path = repository.join(POLICY_PATH);
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(BuildPolicyError::Trust(
            "policy must be a regular non-symlinked file".to_owned(),
        ));
    }
    git_success(
        repository,
        &["ls-files", "--error-unmatch", "--", POLICY_PATH],
        "policy is not tracked",
    )?;
    let authority_path = repository.join(AUTHORITY_LOCK_PATH);
    let authority_metadata = std::fs::symlink_metadata(&authority_path)?;
    if !authority_metadata.file_type().is_file() || authority_metadata.file_type().is_symlink() {
        return Err(BuildPolicyError::Trust(
            "authority lock must be a regular non-symlinked file".to_owned(),
        ));
    }
    git_success(
        repository,
        &[
            "diff",
            "--quiet",
            "HEAD",
            "--",
            POLICY_PATH,
            AUTHORITY_LOCK_PATH,
        ],
        "policy or authority lock differs from HEAD",
    )?;
    let source_commit = git_stdout(repository, &["rev-parse", "HEAD"])?;
    let bytes = std::fs::read(path)?;
    let snapshot = parse_build_policy(&bytes, &source_commit, loaded_at)?;
    let authority: AuthorityLock = serde_json::from_slice(&std::fs::read(authority_path)?)?;
    match authority.pinned.get(POLICY_PATH) {
        Some(pinned) if pinned == &snapshot.policy_sha256 => Ok(snapshot),
        Some(_) => Err(BuildPolicyError::Trust(
            "policy bytes do not match the authority pin".to_owned(),
        )),
        None => Err(BuildPolicyError::Trust(
            "policy is absent from the authority pin set".to_owned(),
        )),
    }
}

/// Loads the immutable policy and authority pin directly from a trusted Git
/// revision. Working-tree bytes are deliberately not consulted.
pub fn load_build_policy_at_revision(
    repository: &Path,
    revision: &str,
    loaded_at: DateTime<Utc>,
) -> Result<BuildPolicySnapshot, BuildPolicyError> {
    let source_commit = git_stdout(repository, &["rev-parse", "--verify", revision])?;
    require_regular_git_blob(repository, &source_commit, POLICY_PATH)?;
    require_regular_git_blob(repository, &source_commit, AUTHORITY_LOCK_PATH)?;
    let policy = git_blob(repository, &source_commit, POLICY_PATH)?;
    let snapshot = parse_build_policy(&policy, &source_commit, loaded_at)?;
    let authority: AuthorityLock =
        serde_json::from_slice(&git_blob(repository, &source_commit, AUTHORITY_LOCK_PATH)?)?;
    match authority.pinned.get(POLICY_PATH) {
        Some(pinned) if pinned == &snapshot.policy_sha256 => Ok(snapshot),
        Some(_) => Err(BuildPolicyError::Trust(
            "policy bytes do not match the authority pin".to_owned(),
        )),
        None => Err(BuildPolicyError::Trust(
            "policy is absent from the authority pin set".to_owned(),
        )),
    }
}

pub fn parse_build_policy(
    bytes: &[u8],
    source_commit: &str,
    loaded_at: DateTime<Utc>,
) -> Result<BuildPolicySnapshot, BuildPolicyError> {
    let text = std::str::from_utf8(bytes)?;
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err(BuildPolicyError::FrontMatter(
            "missing opening delimiter".to_owned(),
        ));
    }
    let mut values = BTreeMap::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(BuildPolicyError::FrontMatter(format!(
                "invalid metadata line {line:?}"
            )));
        };
        let key = key.trim();
        let value = value.trim();
        if !matches!(key, "policy_id" | "schema_version" | "status") {
            return Err(BuildPolicyError::FrontMatter(format!(
                "unknown key {key:?}"
            )));
        }
        if values.insert(key, value).is_some() {
            return Err(BuildPolicyError::FrontMatter(format!(
                "duplicate key {key:?}"
            )));
        }
    }
    if !closed || values.len() != 3 {
        return Err(BuildPolicyError::FrontMatter(
            "metadata must contain exactly policy_id, schema_version, and status".to_owned(),
        ));
    }
    if values.get("policy_id") != Some(&"govfolio-build-performance") {
        return Err(BuildPolicyError::FrontMatter(
            "policy_id must be govfolio-build-performance".to_owned(),
        ));
    }
    let schema_version = values
        .get("schema_version")
        .ok_or_else(|| BuildPolicyError::FrontMatter("missing schema_version".to_owned()))?
        .parse::<u32>()
        .map_err(|_| BuildPolicyError::FrontMatter("schema_version must be u32".to_owned()))?;
    let status = match values.get("status").copied() {
        Some("advisory") => BuildPolicyStatus::Advisory,
        Some("shadow") => BuildPolicyStatus::Shadow,
        Some("enforced") => BuildPolicyStatus::Enforced,
        _ => {
            return Err(BuildPolicyError::FrontMatter(
                "status must be advisory, shadow, or enforced".to_owned(),
            ));
        }
    };
    if source_commit.trim().is_empty() {
        return Err(BuildPolicyError::Trust("source commit is empty".to_owned()));
    }
    Ok(BuildPolicySnapshot {
        schema_version,
        policy_sha256: hex::encode(Sha256::digest(bytes)),
        status,
        source_commit: source_commit.trim().to_owned(),
        loaded_at,
    })
}

fn git_success(repository: &Path, args: &[&str], failure: &str) -> Result<(), BuildPolicyError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(BuildPolicyError::Trust(failure.to_owned()))
    }
}

fn git_stdout(repository: &Path, args: &[&str]) -> Result<String, BuildPolicyError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        return Err(BuildPolicyError::Trust(format!(
            "git {} failed",
            args.join(" ")
        )));
    }
    let stdout = std::str::from_utf8(&output.stdout)?.trim().to_owned();
    if stdout.is_empty() {
        return Err(BuildPolicyError::Trust(format!(
            "git {} returned no value",
            args.join(" ")
        )));
    }
    Ok(stdout)
}

fn require_regular_git_blob(
    repository: &Path,
    revision: &str,
    path: &str,
) -> Result<(), BuildPolicyError> {
    let entry = git_stdout(repository, &["ls-tree", revision, "--", path])?;
    let mode = entry
        .split_ascii_whitespace()
        .next()
        .ok_or_else(|| BuildPolicyError::Trust(format!("{path} is absent from {revision}")))?;
    if !matches!(mode, "100644" | "100755") {
        return Err(BuildPolicyError::Trust(format!(
            "{path} is not a regular file at {revision}"
        )));
    }
    Ok(())
}

fn git_blob(repository: &Path, revision: &str, path: &str) -> Result<Vec<u8>, BuildPolicyError> {
    let object = format!("{revision}:{path}");
    let output = Command::new("git")
        .args(["show", &object])
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        return Err(BuildPolicyError::Trust(format!(
            "failed to read {path} at {revision}"
        )));
    }
    Ok(output.stdout)
}
