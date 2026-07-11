use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use crate::model::Provider;

#[derive(Clone, Debug)]
pub struct RuntimePaths {
    pub root: PathBuf,
    pub control_db: PathBuf,
    pub writer_lock: PathBuf,
    pub runs: PathBuf,
    pub attempts: PathBuf,
    pub blobs: PathBuf,
    pub backups: PathBuf,
}

impl RuntimePaths {
    pub fn discover() -> anyhow::Result<Self> {
        let root = if let Some(path) = std::env::var_os("GOVFOLIO_LOOP_STATE_DIR") {
            PathBuf::from(path)
        } else {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or_else(|| anyhow::anyhow!("HOME/USERPROFILE is unavailable"))?;
            PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("govfolio-loop")
        };
        Ok(Self::under(root))
    }

    #[must_use]
    pub fn under(root: PathBuf) -> Self {
        Self {
            control_db: root.join("control.sqlite3"),
            writer_lock: root.join("control.lock"),
            runs: root.join("runs"),
            attempts: root.join("attempts"),
            blobs: root.join("blobs").join("sha256"),
            backups: root.join("backups"),
            root,
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for path in [
            &self.root,
            &self.runs,
            &self.attempts,
            &self.blobs,
            &self.backups,
        ] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct LoopConfig {
    pub paths: RuntimePaths,
    pub repo: PathBuf,
    pub worktree: PathBuf,
    pub expected_branch: String,
    pub lane_id: String,
    pub role: String,
    pub provider: Provider,
    pub provider_executable: PathBuf,
    pub model: Option<String>,
    pub prompt_file: PathBuf,
    pub authority_bin: PathBuf,
    pub database_url: String,
    pub bronze_root: PathBuf,
    pub epoch_gate_bin: PathBuf,
    pub lease_bin: PathBuf,
    pub epoch: String,
    pub poll_interval: Duration,
}

impl LoopConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let repo = std::env::var_os("GOVFOLIO_REPO")
            .map_or_else(std::env::current_dir, |path| Ok(PathBuf::from(path)))?;
        let worktree =
            std::env::var_os("GOVFOLIO_LOOP_WORKTREE").map_or_else(|| repo.clone(), PathBuf::from);
        let provider = Provider::from_str(
            &std::env::var("GOVFOLIO_LOOP_PROVIDER").unwrap_or_else(|_| "codex".to_owned()),
        )
        .map_err(anyhow::Error::msg)?;
        let provider_executable = match provider {
            Provider::Claude => std::env::var_os("GOVFOLIO_CLAUDE_BIN")
                .map_or_else(|| PathBuf::from("claude"), PathBuf::from),
            Provider::Codex => std::env::var_os("GOVFOLIO_CODEX_BIN")
                .map_or_else(|| PathBuf::from("codex"), PathBuf::from),
        };
        let target = target_dir(&repo);
        let debug = target.join("debug");
        let prompt_file = std::env::var_os("GOVFOLIO_LOOP_PROMPT")
            .map_or_else(|| worktree.join("agents").join("PROMPT.md"), PathBuf::from);
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/govfolio".to_owned());
        let bronze_root = std::env::var_os("GOVFOLIO_BRONZE_ROOT")
            .map_or_else(|| repo.join("target"), PathBuf::from);
        let poll_seconds = std::env::var("GOVFOLIO_LOOP_POLL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(300)
            .max(5);
        let lane_id =
            std::env::var("GOVFOLIO_LOOP_LANE_ID").unwrap_or_else(|_| "orchestrator-0".to_owned());
        Ok(Self {
            paths: RuntimePaths::discover()?,
            repo,
            worktree,
            expected_branch: std::env::var("GOVFOLIO_LOOP_BRANCH")
                .unwrap_or_else(|_| format!("loop/{lane_id}")),
            lane_id,
            role: std::env::var("GOVFOLIO_LOOP_ROLE").unwrap_or_else(|_| "orchestrator".to_owned()),
            provider,
            provider_executable,
            model: std::env::var("GOVFOLIO_LOOP_MODEL")
                .ok()
                .filter(|s| !s.is_empty()),
            prompt_file,
            authority_bin: executable(&debug, "validate-authority"),
            database_url,
            bronze_root,
            epoch_gate_bin: executable(&debug, "epoch-gate"),
            lease_bin: executable(&debug, "jurisdiction-lease"),
            epoch: std::env::var("GOVFOLIO_EPOCH").unwrap_or_else(|_| "E2".to_owned()),
            poll_interval: Duration::from_secs(poll_seconds),
        })
    }
}

fn target_dir(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || repo.join("target"),
        |path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                repo.join(path)
            }
        },
    )
}

fn executable(directory: &Path, stem: &str) -> PathBuf {
    directory.join(if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_layout_is_outside_the_repository_shape() {
        let paths = RuntimePaths::under(PathBuf::from("state"));
        assert_eq!(paths.control_db, PathBuf::from("state/control.sqlite3"));
        assert_eq!(paths.blobs, PathBuf::from("state/blobs/sha256"));
        assert_eq!(paths.attempts, PathBuf::from("state/attempts"));
    }
}
