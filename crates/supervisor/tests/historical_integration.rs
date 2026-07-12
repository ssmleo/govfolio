#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use govfolio_core::integration::HistoricalContractEvidence;
use loop_supervisor::integration::{
    CommandIntegrationBackend, IntegrationBackend, ReceiptCandidate,
};
use sha2::{Digest as _, Sha256};

#[test]
fn historical_candidate_uses_fresh_main_and_has_valid_post_merge_proof() {
    let temp = tempfile::tempdir().unwrap();
    let origin = temp.path().join("origin.git");
    let repo = temp.path().join("repo");
    let candidates = temp.path().join("candidates");
    git(temp.path(), &["init", "--bare", origin.to_str().unwrap()]);
    git(
        temp.path(),
        &["init", "--initial-branch=main", repo.to_str().unwrap()],
    );
    git(
        &repo,
        &["config", "user.email", "integration@example.invalid"],
    );
    git(&repo, &["config", "user.name", "Integration Test"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    for (path, contents) in [
        ("agents/JOURNAL.md", "# Journal\n"),
        ("crates/core/src/useful.rs", "base\n"),
    ] {
        write(&repo, path, contents);
    }
    write_policy(&repo, "base policy");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let merge_base = text(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["branch", "historical-source"]);

    git(&repo, &["switch", "historical-source"]);
    write(
        &repo,
        "crates/core/src/useful.rs",
        "useful historical work\n",
    );
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "historical application"]);
    let source_sha = text(&repo, &["rev-parse", "HEAD"]);

    git(&repo, &["switch", "main"]);
    let active_policy = write_policy(&repo, "active policy");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "active policy"]);
    git(&repo, &["push", "-u", "origin", "main"]);
    let active_main = text(&repo, &["rev-parse", "HEAD"]);

    let receipt = ReceiptCandidate {
        receipt_id: "01HISTORICALRECEIPT".to_owned(),
        source_sha: source_sha.clone(),
        base_sha: merge_base.clone(),
        journal_summary: "historical application integrated".to_owned(),
        repair_ordinal: 0,
        historical_contract: Some(HistoricalContractEvidence {
            merge_base_sha: merge_base,
            active_policy_sha256: active_policy,
            source_sha: source_sha.clone(),
            changed_paths: vec!["crates/core/src/useful.rs".to_owned()],
        }),
    };
    let mut backend = CommandIntegrationBackend::new(
        repo.clone(),
        candidates,
        PathBuf::from("gh"),
        PathBuf::from("unused-loop"),
        temp.path().join("state"),
        receipt
            .historical_contract
            .as_ref()
            .unwrap()
            .active_policy_sha256
            .clone(),
    );
    assert_eq!(backend.fetch_main().unwrap(), active_main);
    backend
        .create_candidate("integration/historical", &active_main)
        .unwrap();
    backend.merge_source(&receipt).unwrap();
    backend
        .append_journal(&receipt.receipt_id, &receipt.journal_summary)
        .unwrap();
    let candidate_sha = backend.commit_candidate(&receipt.receipt_id).unwrap();

    git(&repo, &["merge", "--no-ff", "--no-edit", &candidate_sha]);
    let merge_sha = text(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["push", "origin", "main"]);
    assert!(!is_ancestor(&repo, &source_sha, &merge_sha));
    assert!(
        !backend
            .verify_merged_main(&merge_sha, &receipt, &"f".repeat(40))
            .unwrap()
    );
    assert!(
        backend
            .verify_merged_main(&merge_sha, &receipt, &candidate_sha)
            .unwrap()
    );
}

fn write_policy(repo: &Path, body: &str) -> String {
    let policy = format!(
        "---\npolicy_id: govfolio-build-performance\nschema_version: 1\nstatus: advisory\n---\n{body}\n"
    );
    write(repo, "docs/decisions/build-performance-policy.md", &policy);
    let digest = hex::encode(Sha256::digest(policy.as_bytes()));
    write(
        repo,
        "agents/AUTHORITY.lock.json",
        &format!(
            r#"{{"version":1,"pinned":{{"docs/decisions/build-performance-policy.md":"{digest}"}}}}"#
        ),
    );
    digest
}

fn write(repo: &Path, path: &str, contents: &str) {
    let destination = repo.join(path);
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    std::fs::write(destination, contents).unwrap();
}

fn text(repo: &Path, args: &[&str]) -> String {
    String::from_utf8(output(repo, args))
        .unwrap()
        .trim()
        .to_owned()
}

fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .current_dir(repo)
        .status()
        .unwrap()
        .success()
}

fn git(repo: &Path, args: &[&str]) {
    let _output = output(repo, args);
}

fn output(repo: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
