use std::path::{Component, Path};
use std::process::{Command, Output};

use anyhow::{Context as _, bail};
use sha2::{Digest as _, Sha256};

use crate::build_policy::POLICY_PATH;
pub use govfolio_core::integration::HistoricalContractEvidence;

pub const GOVERNED_POLICY_PATHS: [&str; 7] = [
    "AGENTS.md",
    "CLAUDE.md",
    ".claude/rules/build-performance.md",
    POLICY_PATH,
    "agents/AUTHORITY.lock.json",
    "agents/goals/000-INDEX.md",
    "agents/goals/114-build-resource-admission.md",
];

pub fn assess_historical_contract(
    worktree: &Path,
    active_policy_sha256: &str,
) -> anyhow::Result<HistoricalContractEvidence> {
    require_sha256(active_policy_sha256)?;
    let dirty = git(
        worktree,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if !dirty.stdout.is_empty() {
        bail!("historical worktree is dirty and remains recovery_required");
    }
    let source_sha = git_text(worktree, &["rev-parse", "HEAD"])?;
    let active_sha = git_text(worktree, &["rev-parse", "origin/main"])?;
    let merge_base_sha = git_text(worktree, &["merge-base", "HEAD", "origin/main"])?;
    let active_policy = git_blob(worktree, &active_sha, POLICY_PATH)?
        .ok_or_else(|| anyhow::anyhow!("active main has no canonical build policy"))?;
    let active_digest = hex::encode(Sha256::digest(&active_policy));
    if active_digest != active_policy_sha256 {
        bail!("active policy hash does not match trusted origin/main");
    }

    for path in GOVERNED_POLICY_PATHS {
        let source = git_blob(worktree, &source_sha, path)?;
        let merge_base = git_blob(worktree, &merge_base_sha, path)?;
        let active = git_blob(worktree, &active_sha, path)?;
        if source != merge_base && source != active {
            bail!("historical governed file {path:?} matches neither merge base nor active main");
        }
    }

    let mut changed_paths = changed_paths(worktree, &merge_base_sha, &source_sha)?;
    changed_paths.retain(|path| !GOVERNED_POLICY_PATHS.contains(&path.as_str()));
    if let Some(path) = changed_paths
        .iter()
        .find(|path| is_forbidden_historical_path(path))
    {
        bail!("historical application manifest touches governed path {path:?}");
    }
    Ok(HistoricalContractEvidence {
        merge_base_sha,
        active_policy_sha256: active_policy_sha256.to_owned(),
        source_sha,
        changed_paths,
    })
}

pub fn validate_historical_continuation(
    admitted: &HistoricalContractEvidence,
    completed: &HistoricalContractEvidence,
) -> anyhow::Result<()> {
    if admitted.merge_base_sha != completed.merge_base_sha
        || admitted.active_policy_sha256 != completed.active_policy_sha256
    {
        bail!("historical continuation changed its trusted base or active policy");
    }
    if let Some(path) = admitted
        .changed_paths
        .iter()
        .find(|path| completed.changed_paths.binary_search(path).is_err())
    {
        bail!("historical continuation discarded admitted application path {path:?}");
    }
    Ok(())
}

#[must_use]
pub fn is_forbidden_historical_path(path: &str) -> bool {
    govfolio_core::integration::historical_path_is_governed(path)
}

pub fn validate_changed_path(path: &str) -> anyhow::Result<()> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("historical changed path is not a normalized repository-relative path: {path:?}");
    }
    Ok(())
}

fn changed_paths(worktree: &Path, base: &str, source: &str) -> anyhow::Result<Vec<String>> {
    let output = git(worktree, &["diff", "--name-only", "-z", base, source, "--"])?;
    let mut paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = std::str::from_utf8(path).context("historical changed path is not UTF-8")?;
            validate_changed_path(path)?;
            Ok(path.to_owned())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn git_blob(worktree: &Path, revision: &str, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let object = format!("{revision}:{path}");
    let output = Command::new("git")
        .args(["show", &object])
        .current_dir(worktree)
        .output()
        .with_context(|| format!("spawn git show {object}"))?;
    match output.status.code() {
        Some(0) => Ok(Some(output.stdout)),
        Some(1 | 128) => Ok(None),
        _ => bail!(
            "git show {object:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    }
}

fn git_text(worktree: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = git(worktree, args)?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn git(worktree: &Path, args: &[&str]) -> anyhow::Result<Output> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()
        .with_context(|| format!("spawn git {args:?}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn require_sha256(value: &str) -> anyhow::Result<()> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        bail!("active policy hash is not 64 hexadecimal characters")
    }
}
