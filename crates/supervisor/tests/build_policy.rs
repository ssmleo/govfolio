#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::process::Command;

use chrono::{TimeZone as _, Utc};
use loop_supervisor::build_policy::{
    BuildPolicyStatus, POLICY_PATH, load_build_policy, parse_build_policy,
};
use sha2::{Digest as _, Sha256};

const POLICY: &str = "---\npolicy_id: govfolio-build-performance\nschema_version: 1\nstatus: advisory\n---\n\n# Policy\n";

#[test]
fn build_policy_parser_hashes_the_exact_canonical_bytes() {
    let loaded_at = Utc.timestamp_opt(1_720_000_000, 0).single().unwrap();
    let snapshot = parse_build_policy(POLICY.as_bytes(), "abc123", loaded_at).unwrap();

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.status, BuildPolicyStatus::Advisory);
    assert_eq!(snapshot.source_commit, "abc123");
    assert_eq!(snapshot.loaded_at, loaded_at);
    assert_eq!(snapshot.policy_sha256, hex::encode(Sha256::digest(POLICY)));
}

#[test]
fn build_policy_parser_rejects_unknown_or_duplicate_front_matter() {
    let now = Utc::now();
    let unknown = POLICY.replace("status: advisory", "owner: local\nstatus: advisory");
    assert!(parse_build_policy(unknown.as_bytes(), "abc", now).is_err());

    let duplicate = POLICY.replace("status: advisory", "status: advisory\nstatus: advisory");
    assert!(parse_build_policy(duplicate.as_bytes(), "abc", now).is_err());
}

#[test]
fn build_policy_loader_requires_a_tracked_authority_pinned_regular_file() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    fs::create_dir_all(repo.join("docs/decisions")).unwrap();
    fs::create_dir_all(repo.join("agents")).unwrap();
    fs::write(repo.join(POLICY_PATH), POLICY).unwrap();
    let digest = hex::encode(Sha256::digest(POLICY));
    fs::write(
        repo.join("agents/AUTHORITY.lock.json"),
        format!(r#"{{"version":1,"pinned":{{"{POLICY_PATH}":"{digest}"}}}}"#),
    )
    .unwrap();
    git(repo, &["init", "-q"]);
    git(repo, &["config", "core.autocrlf", "false"]);
    git(
        repo,
        &["config", "user.email", "build-policy@example.invalid"],
    );
    git(repo, &["config", "user.name", "Build Policy Test"]);
    git(
        repo,
        &["add", "--", POLICY_PATH, "agents/AUTHORITY.lock.json"],
    );
    git(repo, &["commit", "-qm", "fixture"]);

    let snapshot = load_build_policy(repo, Utc::now()).unwrap();
    assert_eq!(snapshot.policy_sha256, digest);

    fs::write(repo.join(POLICY_PATH), format!("{POLICY}\nchanged\n")).unwrap();
    assert!(load_build_policy(repo, Utc::now()).is_err());
}

fn git(repo: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}
