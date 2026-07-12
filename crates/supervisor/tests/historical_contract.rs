#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use loop_supervisor::historical_contract::{
    assess_historical_contract, validate_historical_continuation,
};
use sha2::{Digest as _, Sha256};

#[test]
fn historical_contract_accepts_committed_application_work_without_mutation() {
    let fixture = Fixture::new();
    let before = fixture.identity();
    let receipt = assess_historical_contract(&fixture.lane, &fixture.policy_sha()).unwrap();
    let after = fixture.identity();

    assert_eq!(before, after);
    assert_eq!(receipt.source_sha, before.head);
    assert_eq!(receipt.changed_paths, vec!["crates/core/src/useful.rs"]);
    assert_ne!(receipt.merge_base_sha, fixture.origin_main());
}

#[test]
fn historical_contract_preserves_and_fences_dirty_implementation_or_policy() {
    let fixture = Fixture::new();
    let useful = fixture.lane.join("crates/core/src/useful.rs");
    std::fs::write(&useful, "useful implementation\nstill here\n").unwrap();
    assert!(assess_historical_contract(&fixture.lane, &fixture.policy_sha()).is_err());
    assert!(
        std::fs::read_to_string(&useful)
            .unwrap()
            .contains("still here")
    );

    git(
        &fixture.lane,
        &["restore", "--", "crates/core/src/useful.rs"],
    );
    let policy = fixture
        .lane
        .join("docs/decisions/build-performance-policy.md");
    std::fs::write(&policy, "locally modified policy\n").unwrap();
    assert!(assess_historical_contract(&fixture.lane, &fixture.policy_sha()).is_err());
    assert_eq!(
        std::fs::read_to_string(policy).unwrap(),
        "locally modified policy\n"
    );
}

#[test]
fn historical_contract_rejects_committed_governed_paths() {
    let fixture = Fixture::new();
    let goal = fixture.lane.join("agents/goals/999-stale-claim.md");
    std::fs::write(&goal, "stale authority claim\n").unwrap();
    git(&fixture.lane, &["add", "."]);
    git(&fixture.lane, &["commit", "-m", "stale claim"]);

    assert!(assess_historical_contract(&fixture.lane, &fixture.policy_sha()).is_err());
}

#[test]
fn historical_contract_allows_active_policy_blobs_but_excludes_them_from_application_manifest() {
    let fixture = Fixture::new();
    let active_policy = output(
        &fixture.repo,
        &[
            "show",
            "origin/main:docs/decisions/build-performance-policy.md",
        ],
    );
    std::fs::write(
        fixture
            .lane
            .join("docs/decisions/build-performance-policy.md"),
        active_policy,
    )
    .unwrap();
    git(&fixture.lane, &["add", "."]);
    git(&fixture.lane, &["commit", "-m", "refresh policy blob"]);

    let receipt = assess_historical_contract(&fixture.lane, &fixture.policy_sha()).unwrap();
    assert_eq!(receipt.changed_paths, vec!["crates/core/src/useful.rs"]);
}

#[test]
fn historical_continuation_must_preserve_admitted_application_paths_and_trust_root() {
    let fixture = Fixture::new();
    let admitted = assess_historical_contract(&fixture.lane, &fixture.policy_sha()).unwrap();
    let mut completed = admitted.clone();
    completed.source_sha = "f".repeat(40);
    completed
        .changed_paths
        .push("crates/core/src/continued.rs".to_owned());
    completed.changed_paths.sort();
    validate_historical_continuation(&admitted, &completed).unwrap();

    completed.changed_paths = vec!["crates/core/src/continued.rs".to_owned()];
    assert!(validate_historical_continuation(&admitted, &completed).is_err());
    completed.changed_paths = admitted.changed_paths.clone();
    completed.active_policy_sha256 = "f".repeat(64);
    assert!(validate_historical_continuation(&admitted, &completed).is_err());
}

#[derive(Debug, Eq, PartialEq)]
struct Identity {
    head: String,
    index: String,
    status: Vec<u8>,
}

struct Fixture {
    _temp: tempfile::TempDir,
    repo: PathBuf,
    lane: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let lane = temp.path().join("lane");
        std::fs::create_dir_all(repo.join("docs/decisions")).unwrap();
        std::fs::create_dir_all(repo.join(".claude/rules")).unwrap();
        std::fs::create_dir_all(repo.join("agents/goals")).unwrap();
        std::fs::create_dir_all(repo.join("crates/core/src")).unwrap();
        git(
            temp.path(),
            &["init", "--initial-branch=main", repo.to_str().unwrap()],
        );
        git(
            &repo,
            &["config", "user.email", "historical@example.invalid"],
        );
        git(&repo, &["config", "user.name", "Historical Test"]);
        for (path, contents) in [
            ("AGENTS.md", "@CLAUDE.md\n"),
            ("CLAUDE.md", "historical instructions\n"),
            (".claude/rules/build-performance.md", "policy pointer\n"),
            (
                "docs/decisions/build-performance-policy.md",
                "---\npolicy_id: govfolio-build-performance\nschema_version: 1\nstatus: advisory\n---\nbase policy\n",
            ),
            ("agents/AUTHORITY.lock.json", "{}\n"),
            ("agents/goals/000-INDEX.md", "# queue\n"),
            (
                "agents/goals/114-build-resource-admission.md",
                "# goal 114\n",
            ),
            ("crates/core/src/useful.rs", "base\n"),
        ] {
            let destination = repo.join(path);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(destination, contents).unwrap();
        }
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "base"]);
        git(&repo, &["branch", "lane"]);
        git(&repo, &["worktree", "add", lane.to_str().unwrap(), "lane"]);
        std::fs::write(
            lane.join("crates/core/src/useful.rs"),
            "useful implementation\n",
        )
        .unwrap();
        git(&lane, &["add", "."]);
        git(&lane, &["commit", "-m", "useful app work"]);
        std::fs::write(
            repo.join("docs/decisions/build-performance-policy.md"),
            "---\npolicy_id: govfolio-build-performance\nschema_version: 1\nstatus: advisory\n---\nactive policy\n",
        )
        .unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "active policy"]);
        let main = text(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["update-ref", "refs/remotes/origin/main", &main]);
        Self {
            _temp: temp,
            repo,
            lane,
        }
    }

    fn policy_sha(&self) -> String {
        let bytes =
            std::fs::read(self.repo.join("docs/decisions/build-performance-policy.md")).unwrap();
        hex::encode(Sha256::digest(bytes))
    }

    fn origin_main(&self) -> String {
        text(&self.repo, &["rev-parse", "origin/main"])
    }

    fn identity(&self) -> Identity {
        Identity {
            head: text(&self.lane, &["rev-parse", "HEAD"]),
            index: text(&self.lane, &["write-tree"]),
            status: output(
                &self.lane,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            ),
        }
    }
}

fn text(cwd: &Path, args: &[&str]) -> String {
    String::from_utf8(output(cwd, args))
        .unwrap()
        .trim()
        .to_owned()
}

fn output(cwd: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git(cwd: &Path, args: &[&str]) {
    let _stdout = output(cwd, args);
}
