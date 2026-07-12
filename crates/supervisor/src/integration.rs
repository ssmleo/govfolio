//! Clean, singleton receipt integration.
//!
//! Producers supply immutable facts. This module owns every command that can
//! publish them, and it never executes validation commands from a receipt.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

use anyhow::{Context, anyhow, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use govfolio_core::integration::{HistoricalContractEvidence, historical_path_is_governed};

pub const REQUIRED_CHECKS: [&str; 4] = ["rust", "db", "web", "guardrails"];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReceiptCandidate {
    pub receipt_id: String,
    pub source_sha: String,
    pub base_sha: String,
    pub journal_summary: String,
    pub repair_ordinal: u8,
    #[serde(default)]
    pub historical_contract: Option<HistoricalContractEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareOutcome {
    AwaitingCi {
        branch: String,
        pull_request: u64,
        candidate_base_sha: String,
        candidate_sha: String,
    },
    ReworkRequired {
        reason: String,
    },
    Deferred {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinalizeOutcome {
    AwaitingCi,
    Merged { merge_sha: String },
    ReworkRequired { reason: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CheckState {
    pub name: String,
    pub state: String,
}

pub trait IntegrationBackend {
    fn fetch_main(&mut self) -> anyhow::Result<String>;
    fn base_is_ancestor(&mut self, base: &str, source: &str) -> anyhow::Result<bool>;
    fn source_touches_journal(&mut self, base: &str, source: &str) -> anyhow::Result<bool>;
    fn source_changed_paths(&mut self, base: &str, source: &str) -> anyhow::Result<Vec<String>>;
    fn build_policy_sha256_at(&mut self, revision: &str) -> anyhow::Result<String>;
    fn create_candidate(&mut self, branch: &str, main_sha: &str) -> anyhow::Result<()>;
    fn merge_source(&mut self, receipt: &ReceiptCandidate) -> anyhow::Result<()>;
    fn append_journal(&mut self, receipt_id: &str, summary: &str) -> anyhow::Result<()>;
    fn commit_candidate(&mut self, receipt_id: &str) -> anyhow::Result<String>;
    fn run_repository_matrix(&mut self) -> anyhow::Result<()>;
    fn abandon_candidate(&mut self) -> anyhow::Result<()>;
    fn push_candidate(&mut self, branch: &str) -> anyhow::Result<()>;
    fn open_pull_request(&mut self, branch: &str, receipt_id: &str) -> anyhow::Result<u64>;
    fn enable_merge_commit_auto_merge(&mut self, pull_request: u64) -> anyhow::Result<()>;
    fn required_checks(&mut self, pull_request: u64) -> anyhow::Result<Vec<CheckState>>;
    fn merged_sha(&mut self, pull_request: u64) -> anyhow::Result<Option<String>>;
    fn verify_merged_main(
        &mut self,
        merge_sha: &str,
        receipt: &ReceiptCandidate,
        candidate_sha: &str,
    ) -> anyhow::Result<bool>;
}

pub struct IntegrationEngine<B> {
    backend: B,
    moving_main_rebuilds: u8,
}

impl<B> IntegrationEngine<B>
where
    B: IntegrationBackend,
{
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            moving_main_rebuilds: 3,
        }
    }

    pub fn prepare(&mut self, receipt: &ReceiptCandidate) -> anyhow::Result<PrepareOutcome> {
        validate_receipt(receipt)?;
        if !self
            .backend
            .base_is_ancestor(&receipt.base_sha, &receipt.source_sha)?
        {
            return Ok(self.rework(receipt, "receipt base is not an ancestor of source SHA"));
        }
        if self
            .backend
            .source_touches_journal(&receipt.base_sha, &receipt.source_sha)?
        {
            return Ok(self.rework(receipt, "producer commit touches agents/JOURNAL.md"));
        }
        if let Some(historical) = &receipt.historical_contract {
            let actual = self
                .backend
                .source_changed_paths(&historical.merge_base_sha, &receipt.source_sha)?;
            if actual != historical.changed_paths {
                return Ok(self.rework(
                    receipt,
                    "historical changed-path manifest does not match the source tree",
                ));
            }
        }

        for _rebuild in 0..self.moving_main_rebuilds {
            let main_sha = self.backend.fetch_main()?;
            if let Some(historical) = &receipt.historical_contract
                && self.backend.build_policy_sha256_at(&main_sha)?
                    != historical.active_policy_sha256
            {
                return Ok(self.rework(
                    receipt,
                    "historical policy hash does not match the exact candidate base",
                ));
            }
            let branch = format!(
                "integrate/{}-{}",
                receipt.receipt_id.to_ascii_lowercase(),
                Ulid::new().to_string().to_ascii_lowercase()
            );
            self.backend.create_candidate(&branch, &main_sha)?;
            let candidate = self.build_candidate(receipt);
            let candidate_sha = match candidate {
                Ok(candidate_sha) => candidate_sha,
                Err(error) => {
                    let _cleanup = self.backend.abandon_candidate();
                    return Ok(self.rework(receipt, &format!("candidate failed: {error:#}")));
                }
            };
            let refreshed_main = self.backend.fetch_main()?;
            if refreshed_main != main_sha {
                self.backend.abandon_candidate()?;
                continue;
            }
            self.backend.push_candidate(&branch)?;
            let pull_request = self
                .backend
                .open_pull_request(&branch, &receipt.receipt_id)?;
            self.backend.enable_merge_commit_auto_merge(pull_request)?;
            return Ok(PrepareOutcome::AwaitingCi {
                branch,
                pull_request,
                candidate_base_sha: main_sha,
                candidate_sha,
            });
        }
        Ok(self.rework(
            receipt,
            "origin/main moved during every bounded candidate rebuild",
        ))
    }

    pub fn finalize(
        &mut self,
        receipt: &ReceiptCandidate,
        pull_request: u64,
        candidate_sha: &str,
    ) -> anyhow::Result<FinalizeOutcome> {
        let states = self.backend.required_checks(pull_request)?;
        let checks: BTreeMap<&str, &str> = states
            .iter()
            .map(|check| (check.name.as_str(), check.state.as_str()))
            .collect();
        for required in REQUIRED_CHECKS {
            let Some(state) = checks.get(required) else {
                return Ok(FinalizeOutcome::ReworkRequired {
                    reason: format!("required check {required:?} is absent"),
                });
            };
            match state.to_ascii_lowercase().as_str() {
                "success" | "successful" | "completed" => {}
                "failure" | "failed" | "cancelled" | "timed_out" => {
                    return Ok(FinalizeOutcome::ReworkRequired {
                        reason: format!("required check {required:?} is {state}"),
                    });
                }
                _ => return Ok(FinalizeOutcome::AwaitingCi),
            }
        }
        let Some(merge_sha) = self.backend.merged_sha(pull_request)? else {
            return Ok(FinalizeOutcome::AwaitingCi);
        };
        if !self
            .backend
            .verify_merged_main(&merge_sha, receipt, candidate_sha)?
        {
            return Ok(FinalizeOutcome::ReworkRequired {
                reason: "merged SHA, source ancestry, or canonical JOURNAL receipt proof failed"
                    .to_owned(),
            });
        }
        Ok(FinalizeOutcome::Merged { merge_sha })
    }

    fn build_candidate(&mut self, receipt: &ReceiptCandidate) -> anyhow::Result<String> {
        self.backend.merge_source(receipt)?;
        self.backend
            .append_journal(&receipt.receipt_id, &receipt.journal_summary)?;
        let candidate_sha = self.backend.commit_candidate(&receipt.receipt_id)?;
        self.backend.run_repository_matrix()?;
        Ok(candidate_sha)
    }

    fn rework(&self, receipt: &ReceiptCandidate, reason: &str) -> PrepareOutcome {
        if receipt.repair_ordinal >= 2 {
            PrepareOutcome::Deferred {
                reason: reason.to_owned(),
            }
        } else {
            PrepareOutcome::ReworkRequired {
                reason: reason.to_owned(),
            }
        }
    }

    #[must_use]
    pub fn into_backend(self) -> B {
        self.backend
    }

    pub fn current_main(&mut self) -> anyhow::Result<String> {
        self.backend.fetch_main()
    }
}

fn validate_receipt(receipt: &ReceiptCandidate) -> anyhow::Result<()> {
    for (label, value) in [
        ("receipt_id", receipt.receipt_id.as_str()),
        ("source_sha", receipt.source_sha.as_str()),
        ("base_sha", receipt.base_sha.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("{label} is empty");
        }
    }
    if receipt.journal_summary.lines().count() != 1 || receipt.journal_summary.trim().is_empty() {
        bail!("journal_summary must be exactly one non-empty line");
    }
    if receipt.repair_ordinal > 2 {
        bail!("repair_ordinal exceeds the bounded repair budget");
    }
    if let Some(historical) = &receipt.historical_contract
        && (historical.source_sha != receipt.source_sha
            || historical.merge_base_sha != receipt.base_sha
            || historical.active_policy_sha256.len() != 64
            || historical
                .changed_paths
                .iter()
                .any(|path| historical_path_is_governed(path))
            || !historical
                .changed_paths
                .windows(2)
                .all(|pair| pair[0] < pair[1]))
    {
        bail!("historical receipt identity or changed-path manifest is invalid");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct CommandIntegrationBackend {
    repo: PathBuf,
    candidates_root: PathBuf,
    candidate: Option<PathBuf>,
    gh: PathBuf,
    loop_binary: PathBuf,
    state_root: PathBuf,
    policy_sha256: String,
}

impl CommandIntegrationBackend {
    #[must_use]
    pub fn new(
        repo: PathBuf,
        candidates_root: PathBuf,
        gh: PathBuf,
        loop_binary: PathBuf,
        state_root: PathBuf,
        policy_sha256: String,
    ) -> Self {
        Self {
            repo,
            candidates_root,
            candidate: None,
            gh,
            loop_binary,
            state_root,
            policy_sha256,
        }
    }

    fn candidate(&self) -> anyhow::Result<&Path> {
        self.candidate
            .as_deref()
            .ok_or_else(|| anyhow!("integration candidate is not prepared"))
    }

    fn git(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<Output> {
        command_output(Path::new("git"), cwd, args)
    }

    fn run_checked(&self, program: &Path, cwd: &Path, args: &[&str]) -> anyhow::Result<Output> {
        let output = command_output(program, cwd, args)?;
        if output.status.success() {
            Ok(output)
        } else {
            bail!(
                "{} {:?} failed: {}",
                program.display(),
                args,
                bounded(&output.stderr)
            )
        }
    }

    fn run_admitted_cargo(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<Output> {
        let mut governed = vec![
            "cargo".to_owned(),
            "--class".to_owned(),
            "exclusive".to_owned(),
            "--category".to_owned(),
            "integration-validation".to_owned(),
            "--policy-sha".to_owned(),
            self.policy_sha256.clone(),
            "--".to_owned(),
        ];
        governed.extend(args.iter().map(|arg| (*arg).to_owned()));
        let output = Command::new(&self.loop_binary)
            .args(&governed)
            .current_dir(cwd)
            .env("GOVFOLIO_LOOP_STATE_DIR", &self.state_root)
            .env("GOVFOLIO_BUILD_OWNER", "interactive:integrator")
            .env_remove("GOVFOLIO_LOOP_LANE_ID")
            .env_remove("GOVFOLIO_LANE_FENCE")
            .output()
            .context("run integration Cargo through build admission")?;
        if output.status.success() {
            Ok(output)
        } else {
            bail!(
                "supervised Cargo {:?} failed: {}",
                args,
                bounded(&output.stderr)
            )
        }
    }
}

impl IntegrationBackend for CommandIntegrationBackend {
    fn fetch_main(&mut self) -> anyhow::Result<String> {
        self.run_checked(Path::new("git"), &self.repo, &["fetch", "origin", "main"])?;
        let output =
            self.run_checked(Path::new("git"), &self.repo, &["rev-parse", "origin/main"])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn base_is_ancestor(&mut self, base: &str, source: &str) -> anyhow::Result<bool> {
        let output = self.git(&self.repo, &["merge-base", "--is-ancestor", base, source])?;
        status_bool(output.status, "git merge-base --is-ancestor")
    }

    fn source_touches_journal(&mut self, base: &str, source: &str) -> anyhow::Result<bool> {
        let output = self.git(
            &self.repo,
            &["diff", "--quiet", base, source, "--", "agents/JOURNAL.md"],
        )?;
        status_bool(output.status, "git diff --quiet").map(|clean| !clean)
    }

    fn source_changed_paths(&mut self, base: &str, source: &str) -> anyhow::Result<Vec<String>> {
        let output = self.run_checked(
            Path::new("git"),
            &self.repo,
            &["diff", "--name-only", "-z", base, source, "--"],
        )?;
        let mut paths = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8(path.to_vec()).context("changed path is not UTF-8"))
            .collect::<anyhow::Result<Vec<_>>>()?;
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn build_policy_sha256_at(&mut self, revision: &str) -> anyhow::Result<String> {
        Ok(
            crate::build_policy::load_build_policy_at_revision(&self.repo, revision, Utc::now())?
                .policy_sha256,
        )
    }

    fn create_candidate(&mut self, branch: &str, main_sha: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.candidates_root)?;
        let path = self.candidates_root.join(branch.replace('/', "-"));
        if path.exists() {
            bail!("candidate path already exists: {}", path.display());
        }
        let path_arg = path.to_string_lossy().into_owned();
        self.run_checked(
            Path::new("git"),
            &self.repo,
            &["worktree", "add", "--detach", &path_arg, main_sha],
        )?;
        self.run_checked(Path::new("git"), &path, &["switch", "-c", branch])?;
        self.candidate = Some(path);
        Ok(())
    }

    fn merge_source(&mut self, receipt: &ReceiptCandidate) -> anyhow::Result<()> {
        let candidate = self.candidate()?.to_path_buf();
        if let Some(historical) = &receipt.historical_contract {
            let mut arguments = vec![
                "diff".to_owned(),
                "--binary".to_owned(),
                historical.merge_base_sha.clone(),
                receipt.source_sha.clone(),
                "--".to_owned(),
            ];
            arguments.extend(historical.changed_paths.iter().cloned());
            let argument_refs = arguments.iter().map(String::as_str).collect::<Vec<_>>();
            let patch = self.run_checked(Path::new("git"), &self.repo, &argument_refs)?;
            let mut child = Command::new("git")
                .args(["apply", "--index", "--binary", "-"])
                .current_dir(&candidate)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("spawn git apply for historical application patch")?;
            child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("git apply stdin is unavailable"))?
                .write_all(&patch.stdout)?;
            let output = child.wait_with_output()?;
            if !output.status.success() {
                bail!(
                    "historical application patch failed: {}",
                    bounded(&output.stderr)
                );
            }
            return Ok(());
        }
        self.run_checked(
            Path::new("git"),
            &candidate,
            &["merge", "--no-ff", "--no-commit", &receipt.source_sha],
        )?;
        Ok(())
    }

    fn append_journal(&mut self, receipt_id: &str, summary: &str) -> anyhow::Result<()> {
        if summary.lines().count() != 1 {
            bail!("canonical journal summary is not one line");
        }
        let path = self.candidate()?.join("agents").join("JOURNAL.md");
        let mut file = OpenOptions::new().append(true).open(&path)?;
        writeln!(
            file,
            "{} | receipt={} | {}",
            chrono::Utc::now().date_naive(),
            receipt_id,
            summary.trim()
        )?;
        file.sync_all()?;
        Ok(())
    }

    fn commit_candidate(&mut self, receipt_id: &str) -> anyhow::Result<String> {
        let candidate = self.candidate()?.to_path_buf();
        self.run_checked(
            Path::new("git"),
            &candidate,
            &["add", "--", "agents/JOURNAL.md"],
        )?;
        let message = format!("integrate receipt {receipt_id}");
        self.run_checked(
            Path::new("git"),
            &candidate,
            &["commit", "--no-gpg-sign", "-m", &message],
        )?;
        let output = self.run_checked(Path::new("git"), &candidate, &["rev-parse", "HEAD"])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn run_repository_matrix(&mut self) -> anyhow::Result<()> {
        let candidate = self.candidate()?.to_path_buf();
        let commands: [(&str, &[&str]); 16] = [
            ("cargo", &["fmt", "--check"]),
            (
                "cargo",
                &["clippy", "--all-targets", "--", "-D", "warnings"],
            ),
            ("cargo", &["test", "--workspace"]),
            ("cargo", &["test", "--workspace", "--", "--ignored"]),
            ("pnpm", &["install", "--offline", "--frozen-lockfile"]),
            ("pnpm", &["--filter", "web", "lint"]),
            ("pnpm", &["--filter", "web", "typecheck"]),
            ("pnpm", &["--filter", "web", "test"]),
            (
                "sh",
                &[
                    "scripts/check-migration-safety.sh",
                    "crates/core/migrations",
                ],
            ),
            ("git", &["diff", "--check", "origin/main...HEAD"]),
            (
                "cargo",
                &[
                    "run",
                    "-p",
                    "pipeline",
                    "--bin",
                    "validate-authority",
                    "--",
                    "--ci",
                ],
            ),
            (
                "node",
                &["--test", "scripts/agents/codex-contract.test.mjs"],
            ),
            (
                "node",
                &[
                    "scripts/agents/render-codex-contract.mjs",
                    "--check",
                    "--repo-root",
                    ".",
                ],
            ),
            (
                "node",
                &[
                    "scripts/agents/validate-codex-contract.mjs",
                    "--repo-root",
                    ".",
                ],
            ),
            ("bash", &["-n", "agents/run-loop-codex.sh"]),
            (
                "node",
                &["scripts/agents/check-codex-contract-clean-worktree.mjs"],
            ),
        ];
        for (program, args) in commands {
            if program == "cargo" {
                self.run_admitted_cargo(&candidate, args)
                    .with_context(|| "repository-owned supervised Cargo validation")?;
            } else {
                self.run_checked(Path::new(program), &candidate, args)
                    .with_context(|| format!("repository-owned validation {program}"))?;
            }
        }
        Ok(())
    }

    fn abandon_candidate(&mut self) -> anyhow::Result<()> {
        let Some(candidate) = self.candidate.take() else {
            return Ok(());
        };
        if !candidate.starts_with(&self.candidates_root) {
            bail!("refusing to remove candidate outside configured root");
        }
        let path_arg = candidate.to_string_lossy().into_owned();
        self.run_checked(
            Path::new("git"),
            &self.repo,
            &["worktree", "remove", "--force", &path_arg],
        )?;
        Ok(())
    }

    fn push_candidate(&mut self, branch: &str) -> anyhow::Result<()> {
        let candidate = self.candidate()?.to_path_buf();
        self.run_checked(Path::new("git"), &candidate, &["push", "origin", branch])?;
        Ok(())
    }

    fn open_pull_request(&mut self, branch: &str, receipt_id: &str) -> anyhow::Result<u64> {
        let title = format!("Integrate receipt {receipt_id}");
        let body = format!("Automated clean integration for immutable receipt `{receipt_id}`.");
        self.run_checked(
            &self.gh,
            &self.repo,
            &[
                "pr", "create", "--head", branch, "--base", "main", "--title", &title, "--body",
                &body,
            ],
        )?;
        let output = self.run_checked(
            &self.gh,
            &self.repo,
            &["pr", "view", branch, "--json", "number", "--jq", ".number"],
        )?;
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .context("parse pull request number")
    }

    fn enable_merge_commit_auto_merge(&mut self, pull_request: u64) -> anyhow::Result<()> {
        let pull_request = pull_request.to_string();
        self.run_checked(
            &self.gh,
            &self.repo,
            &["pr", "merge", &pull_request, "--auto", "--merge"],
        )?;
        Ok(())
    }

    fn required_checks(&mut self, pull_request: u64) -> anyhow::Result<Vec<CheckState>> {
        let pull_request = pull_request.to_string();
        let output = command_output(
            &self.gh,
            &self.repo,
            &[
                "pr",
                "checks",
                &pull_request,
                "--required",
                "--json",
                "name,state",
            ],
        )?;
        if !output.status.success() && output.status.code() != Some(8) {
            bail!("gh pr checks failed: {}", bounded(&output.stderr));
        }
        serde_json::from_slice(&output.stdout).context("parse required check states")
    }

    fn merged_sha(&mut self, pull_request: u64) -> anyhow::Result<Option<String>> {
        let pull_request = pull_request.to_string();
        let output = self.run_checked(
            &self.gh,
            &self.repo,
            &[
                "pr",
                "view",
                &pull_request,
                "--json",
                "mergeCommit",
                "--jq",
                ".mergeCommit.oid // empty",
            ],
        )?;
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok((!sha.is_empty()).then_some(sha))
    }

    fn verify_merged_main(
        &mut self,
        merge_sha: &str,
        receipt: &ReceiptCandidate,
        candidate_sha: &str,
    ) -> anyhow::Result<bool> {
        let _main = self.fetch_main()?;
        let mut ancestors = vec![merge_sha.to_owned()];
        if receipt.historical_contract.is_none() {
            ancestors.push(receipt.source_sha.clone());
        }
        for ancestor in ancestors {
            let output = self.git(
                &self.repo,
                &["merge-base", "--is-ancestor", &ancestor, "origin/main"],
            )?;
            if !status_bool(output.status, "verify origin/main ancestry")? {
                return Ok(false);
            }
        }
        let candidate_commit = format!("{merge_sha}^2");
        let resolved = self.run_checked(
            Path::new("git"),
            &self.repo,
            &["rev-parse", &candidate_commit],
        )?;
        if String::from_utf8_lossy(&resolved.stdout).trim() != candidate_sha {
            return Ok(false);
        }
        if let Some(historical) = &receipt.historical_contract {
            let output = self.git(
                &self.repo,
                &[
                    "merge-base",
                    "--is-ancestor",
                    &candidate_commit,
                    "origin/main",
                ],
            )?;
            if !status_bool(output.status, "verify historical candidate ancestry")? {
                return Ok(false);
            }
            let message = self.run_checked(
                Path::new("git"),
                &self.repo,
                &["log", "-1", "--format=%s", &candidate_commit],
            )?;
            if String::from_utf8_lossy(&message.stdout).trim()
                != format!("integrate receipt {}", receipt.receipt_id)
            {
                return Ok(false);
            }
            let output = self.run_checked(
                Path::new("git"),
                &self.repo,
                &[
                    "show",
                    "--format=",
                    "--name-only",
                    "-z",
                    &candidate_commit,
                    "--",
                ],
            )?;
            let mut actual = output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
                .map(|path| String::from_utf8(path.to_vec()))
                .collect::<Result<Vec<_>, _>>()?;
            actual.sort();
            actual.dedup();
            let mut expected = historical.changed_paths.clone();
            expected.push("agents/JOURNAL.md".to_owned());
            expected.sort();
            expected.dedup();
            if actual != expected {
                return Ok(false);
            }
        }
        let output = self.run_checked(
            Path::new("git"),
            &self.repo,
            &["show", "origin/main:agents/JOURNAL.md"],
        )?;
        let needle = format!("receipt={}", receipt.receipt_id);
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains(&needle))
            .count()
            == 1)
    }
}

fn command_output(program: &Path, cwd: &Path, args: &[&str]) -> anyhow::Result<Output> {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("spawn {} {:?}", program.display(), args))
}

fn status_bool(status: ExitStatus, operation: &str) -> anyhow::Result<bool> {
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("{operation} failed with {status}"),
    }
}

fn bounded(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.chars()
        .rev()
        .take(2_000)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    #[allow(clippy::struct_excessive_bools)]
    struct FakeBackend {
        mains: Vec<String>,
        next_main: usize,
        base_ok: bool,
        journal_touched: bool,
        changed_paths: Vec<String>,
        active_policy_sha256: Option<String>,
        policy_revisions: Vec<String>,
        build_fails: bool,
        abandoned: usize,
        pushed: usize,
        checks: Vec<CheckState>,
        merge_sha: Option<String>,
        merged_proof: bool,
    }

    impl IntegrationBackend for FakeBackend {
        fn fetch_main(&mut self) -> anyhow::Result<String> {
            let value = self
                .mains
                .get(self.next_main)
                .or_else(|| self.mains.last())
                .cloned()
                .ok_or_else(|| anyhow!("missing fake main"))?;
            self.next_main = self.next_main.saturating_add(1);
            Ok(value)
        }

        fn base_is_ancestor(&mut self, _base: &str, _source: &str) -> anyhow::Result<bool> {
            Ok(self.base_ok)
        }

        fn source_touches_journal(&mut self, _base: &str, _source: &str) -> anyhow::Result<bool> {
            Ok(self.journal_touched)
        }

        fn source_changed_paths(
            &mut self,
            _base: &str,
            _source: &str,
        ) -> anyhow::Result<Vec<String>> {
            Ok(self.changed_paths.clone())
        }

        fn build_policy_sha256_at(&mut self, revision: &str) -> anyhow::Result<String> {
            self.policy_revisions.push(revision.to_owned());
            Ok(self
                .active_policy_sha256
                .clone()
                .unwrap_or_else(|| "c".repeat(64)))
        }

        fn create_candidate(&mut self, _branch: &str, _main_sha: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn merge_source(&mut self, _receipt: &ReceiptCandidate) -> anyhow::Result<()> {
            if self.build_fails {
                bail!("conflict")
            }
            Ok(())
        }

        fn append_journal(&mut self, _receipt_id: &str, _summary: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn commit_candidate(&mut self, _receipt_id: &str) -> anyhow::Result<String> {
            Ok("candidate".to_owned())
        }

        fn run_repository_matrix(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn abandon_candidate(&mut self) -> anyhow::Result<()> {
            self.abandoned += 1;
            Ok(())
        }

        fn push_candidate(&mut self, _branch: &str) -> anyhow::Result<()> {
            self.pushed += 1;
            Ok(())
        }

        fn open_pull_request(&mut self, _branch: &str, _receipt_id: &str) -> anyhow::Result<u64> {
            Ok(7)
        }

        fn enable_merge_commit_auto_merge(&mut self, _pull_request: u64) -> anyhow::Result<()> {
            Ok(())
        }

        fn required_checks(&mut self, _pull_request: u64) -> anyhow::Result<Vec<CheckState>> {
            Ok(self.checks.clone())
        }

        fn merged_sha(&mut self, _pull_request: u64) -> anyhow::Result<Option<String>> {
            Ok(self.merge_sha.clone())
        }

        fn verify_merged_main(
            &mut self,
            _merge_sha: &str,
            _receipt: &ReceiptCandidate,
            _candidate_sha: &str,
        ) -> anyhow::Result<bool> {
            Ok(self.merged_proof)
        }
    }

    fn receipt(repair_ordinal: u8) -> ReceiptCandidate {
        ReceiptCandidate {
            receipt_id: "01KRECEIPT".to_owned(),
            source_sha: "source".to_owned(),
            base_sha: "base".to_owned(),
            journal_summary: "one canonical summary".to_owned(),
            repair_ordinal,
            historical_contract: None,
        }
    }

    fn successful_checks() -> Vec<CheckState> {
        REQUIRED_CHECKS
            .into_iter()
            .map(|name| CheckState {
                name: name.to_owned(),
                state: "success".to_owned(),
            })
            .collect()
    }

    #[test]
    fn integration_moving_main_rebuilds_before_any_push() {
        let backend = FakeBackend {
            mains: vec![
                "main-a".to_owned(),
                "main-b".to_owned(),
                "main-b".to_owned(),
            ],
            base_ok: true,
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        let outcome = engine.prepare(&receipt(0)).expect("prepare");
        assert!(matches!(outcome, PrepareOutcome::AwaitingCi { .. }));
        let backend = engine.into_backend();
        assert_eq!(backend.abandoned, 1);
        assert_eq!(backend.pushed, 1);
    }

    #[test]
    fn integration_rejects_producer_journal_and_bounds_repairs() {
        let backend = FakeBackend {
            mains: vec!["main".to_owned()],
            base_ok: true,
            journal_touched: true,
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        assert!(matches!(
            engine.prepare(&receipt(2)).expect("prepare"),
            PrepareOutcome::Deferred { .. }
        ));
    }

    #[test]
    fn integration_historical_manifest_must_match_and_exclude_governed_paths() {
        let mut candidate = receipt(0);
        candidate.source_sha = "a".repeat(40);
        candidate.base_sha = "b".repeat(40);
        candidate.historical_contract = Some(HistoricalContractEvidence {
            merge_base_sha: candidate.base_sha.clone(),
            active_policy_sha256: "c".repeat(64),
            source_sha: candidate.source_sha.clone(),
            changed_paths: vec!["crates/core/src/useful.rs".to_owned()],
        });
        let backend = FakeBackend {
            mains: vec!["main".to_owned()],
            base_ok: true,
            changed_paths: vec!["crates/core/src/different.rs".to_owned()],
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        assert!(matches!(
            engine.prepare(&candidate).expect("prepare"),
            PrepareOutcome::ReworkRequired { .. }
        ));

        let backend = FakeBackend {
            mains: vec!["main".to_owned()],
            base_ok: true,
            changed_paths: vec!["crates/core/src/useful.rs".to_owned()],
            active_policy_sha256: Some("d".repeat(64)),
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        assert!(matches!(
            engine.prepare(&candidate).expect("prepare"),
            PrepareOutcome::ReworkRequired { .. }
        ));
        let backend = engine.into_backend();
        assert_eq!(backend.policy_revisions, ["main"]);
        assert_eq!(backend.next_main, 1);

        candidate
            .historical_contract
            .as_mut()
            .expect("historical")
            .changed_paths = vec!["agents/goals/999-stale.md".to_owned()];
        assert!(validate_receipt(&candidate).is_err());
    }

    #[test]
    fn integration_requires_all_four_checks_and_exact_merged_proof() {
        let backend = FakeBackend {
            checks: successful_checks(),
            merge_sha: Some("merge".to_owned()),
            merged_proof: true,
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        assert_eq!(
            engine
                .finalize(&receipt(0), 7, "candidate")
                .expect("finalize"),
            FinalizeOutcome::Merged {
                merge_sha: "merge".to_owned()
            }
        );
    }

    #[test]
    fn integration_missing_required_check_never_applies() {
        let backend = FakeBackend {
            checks: successful_checks().into_iter().take(3).collect(),
            merge_sha: Some("merge".to_owned()),
            merged_proof: true,
            ..FakeBackend::default()
        };
        let mut engine = IntegrationEngine::new(backend);
        assert!(matches!(
            engine
                .finalize(&receipt(0), 7, "candidate")
                .expect("finalize"),
            FinalizeOutcome::ReworkRequired { .. }
        ));
    }
}
