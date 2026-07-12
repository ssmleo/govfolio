#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use chrono::{Duration, TimeZone as _, Utc};
use loop_supervisor::build_classifier::ResourceClass;
use loop_supervisor::build_policy::{BuildPolicySnapshot, BuildPolicyStatus};
use loop_supervisor::build_store::{BuildRequestSpec, BuildRequestState, ProcessIdentity};
use loop_supervisor::process::observed_process_identity;
use loop_supervisor::store::ControlStore;

fn at(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_720_000_000 + seconds, 0)
        .single()
        .unwrap()
}

fn request(id: &str, policy: &str) -> BuildRequestSpec {
    BuildRequestSpec {
        request_id: id.to_owned(),
        lane_id: None,
        lane_fence: None,
        owner_identity: "recovery-test".to_owned(),
        policy_sha256: policy.to_owned(),
        resource_class: ResourceClass::Focused,
        category: None,
        worktree: PathBuf::from("C:/repo/lane"),
        target_dir: PathBuf::from("C:/repo/lane/target"),
        command_sha256: "b".repeat(64),
        effective_jobs: 6,
        deadline: at(0) + Duration::minutes(30),
    }
}

#[tokio::test]
async fn build_recovery_requires_proof_the_exact_recorded_process_is_dead() {
    let temp = tempfile::tempdir().unwrap();
    let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
        .await
        .unwrap();
    let old = store
        .acquire_supervisor("old", at(0), Duration::seconds(1))
        .await
        .unwrap();
    let policy = BuildPolicySnapshot {
        schema_version: 1,
        policy_sha256: "a".repeat(64),
        status: BuildPolicyStatus::Advisory,
        source_commit: "abc".to_owned(),
        loaded_at: at(0),
    };
    store.record_build_policy_snapshot(&policy).await.unwrap();

    let live = observed_process_identity(std::process::id())
        .unwrap()
        .unwrap();
    store
        .enqueue_build(&old, &request("live", &policy.policy_sha256), at(0))
        .await
        .unwrap();
    store.start_build(&old, "live", &live, at(0)).await.unwrap();
    store
        .enqueue_build(&old, &request("dead", &policy.policy_sha256), at(0))
        .await
        .unwrap();
    store
        .start_build(
            &old,
            "dead",
            &ProcessIdentity {
                pid: u32::MAX,
                started_at_ms: 1,
            },
            at(0),
        )
        .await
        .unwrap();

    let current = store
        .acquire_supervisor("current", at(2), Duration::minutes(2))
        .await
        .unwrap();
    let reconciled = store
        .reconcile_build_requests(&current, at(2))
        .await
        .unwrap();
    assert_eq!(reconciled.recovery_required, 2);
    assert!(store.has_build_recovery_required().await.unwrap());
    assert!(store.recover_build(&current, "live", at(2)).await.is_err());

    store.recover_build(&current, "dead", at(2)).await.unwrap();
    assert_eq!(
        store.build_request("dead").await.unwrap().unwrap().state,
        BuildRequestState::Cancelled
    );
    assert!(store.has_build_recovery_required().await.unwrap());
}
