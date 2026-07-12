#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use chrono::{Duration, TimeZone as _, Utc};
use loop_supervisor::build_classifier::ResourceClass;
use loop_supervisor::build_policy::{BuildPolicySnapshot, BuildPolicyStatus};
use loop_supervisor::build_store::{
    BuildRequestSpec, BuildRequestState, BuildTerminal, ProcessIdentity,
};
use loop_supervisor::store::ControlStore;

fn at(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_720_000_000 + seconds, 0)
        .single()
        .unwrap()
}

fn policy() -> BuildPolicySnapshot {
    BuildPolicySnapshot {
        schema_version: 1,
        policy_sha256: "a".repeat(64),
        status: BuildPolicyStatus::Advisory,
        source_commit: "abc123".to_owned(),
        loaded_at: at(0),
    }
}

fn request(policy_sha256: &str) -> BuildRequestSpec {
    BuildRequestSpec {
        request_id: "request-1".to_owned(),
        lane_id: None,
        lane_fence: None,
        owner_identity: "interactive:test".to_owned(),
        policy_sha256: policy_sha256.to_owned(),
        resource_class: ResourceClass::Focused,
        category: Some("verification".to_owned()),
        worktree: PathBuf::from("C:/repo/lane-a"),
        target_dir: PathBuf::from("C:/repo/lane-a/target"),
        command_sha256: "b".repeat(64),
        effective_jobs: 6,
        deadline: at(0) + Duration::minutes(30),
    }
}

#[tokio::test]
async fn build_store_migrates_and_enforces_fenced_request_transitions() {
    let temp = tempfile::tempdir().unwrap();
    let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
        .await
        .unwrap();
    let supervisor = store
        .acquire_supervisor("supervisor-a", at(0), Duration::minutes(2))
        .await
        .unwrap();
    let snapshot = policy();
    store.record_build_policy_snapshot(&snapshot).await.unwrap();

    let queued = store
        .enqueue_build(&supervisor, &request(&snapshot.policy_sha256), at(0))
        .await
        .unwrap();
    assert_eq!(queued.queue_sequence, 1);
    assert_eq!(queued.state, BuildRequestState::Queued);
    assert_eq!(queued.command_sha256, "b".repeat(64));

    let identity = ProcessIdentity {
        pid: 4242,
        started_at_ms: 99,
    };
    store
        .start_build(&supervisor, "request-1", &identity, at(1))
        .await
        .unwrap();
    let retry_identity = ProcessIdentity {
        pid: 4343,
        started_at_ms: 100,
    };
    store
        .retry_build(&supervisor, "request-1", &retry_identity, at(2))
        .await
        .unwrap();
    assert!(
        store
            .retry_build(&supervisor, "request-1", &retry_identity, at(2))
            .await
            .is_err()
    );
    store
        .heartbeat_build(&supervisor, "request-1", at(2))
        .await
        .unwrap();
    store
        .finish_build(
            &supervisor,
            "request-1",
            BuildTerminal::Completed { exit_code: 0 },
            Some(&"c".repeat(64)),
            at(3),
        )
        .await
        .unwrap();

    let record = store.build_request("request-1").await.unwrap().unwrap();
    assert_eq!(record.state, BuildRequestState::Completed);
    assert_eq!(record.exit_code, Some(0));
    assert_eq!(record.evidence_sha256, Some("c".repeat(64)));
    assert_eq!(record.process_identity, Some(retry_identity));
    assert_eq!(store.control_schema_version().await.unwrap(), 3);
}

#[tokio::test]
async fn build_store_rejects_old_fence_start_and_reconciles_restart_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
        .await
        .unwrap();
    let old = store
        .acquire_supervisor("old", at(0), Duration::seconds(1))
        .await
        .unwrap();
    let snapshot = policy();
    store.record_build_policy_snapshot(&snapshot).await.unwrap();
    store
        .enqueue_build(&old, &request(&snapshot.policy_sha256), at(0))
        .await
        .unwrap();

    let current = store
        .acquire_supervisor("current", at(2), Duration::minutes(2))
        .await
        .unwrap();
    assert!(
        store
            .start_build(
                &current,
                "request-1",
                &ProcessIdentity {
                    pid: 1,
                    started_at_ms: 1,
                },
                at(2),
            )
            .await
            .is_err()
    );
    let reconciled = store
        .reconcile_build_requests(&current, at(2))
        .await
        .unwrap();
    assert_eq!(reconciled.cancelled_queued, 1);
    assert_eq!(reconciled.recovery_required, 0);
    assert_eq!(
        store
            .build_request("request-1")
            .await
            .unwrap()
            .unwrap()
            .state,
        BuildRequestState::Cancelled
    );
}

#[tokio::test]
async fn build_store_records_a_pre_pid_launch_failure_as_terminal() {
    let temp = tempfile::tempdir().unwrap();
    let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
        .await
        .unwrap();
    let supervisor = store
        .acquire_supervisor("supervisor", at(0), Duration::minutes(2))
        .await
        .unwrap();
    let snapshot = policy();
    store.record_build_policy_snapshot(&snapshot).await.unwrap();
    store
        .enqueue_build(&supervisor, &request(&snapshot.policy_sha256), at(0))
        .await
        .unwrap();

    store
        .fail_queued_build(
            &supervisor,
            "request-1",
            "cargo_launch_failed",
            Some(&"d".repeat(64)),
            at(1),
        )
        .await
        .unwrap();

    let record = store.build_request("request-1").await.unwrap().unwrap();
    assert_eq!(record.state, BuildRequestState::Failed);
    assert_eq!(record.exit_code, Some(1));
    assert_eq!(record.outcome.as_deref(), Some("cargo_launch_failed"));
    assert!(record.process_identity.is_none());
}

#[tokio::test]
async fn build_store_revalidates_lane_fence_before_starting_queued_work() {
    let temp = tempfile::tempdir().unwrap();
    let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
        .await
        .unwrap();
    let supervisor = store
        .acquire_supervisor("supervisor", at(0), Duration::minutes(2))
        .await
        .unwrap();
    let lane = store
        .acquire_lane(
            "lane-a",
            "owner-a",
            &supervisor,
            at(0),
            Duration::minutes(2),
        )
        .await
        .unwrap();
    let snapshot = policy();
    store.record_build_policy_snapshot(&snapshot).await.unwrap();
    let mut spec = request(&snapshot.policy_sha256);
    spec.request_id = "lane-request".to_owned();
    spec.lane_id = Some(lane.lane_id.clone());
    spec.lane_fence = Some(lane.fence);
    spec.owner_identity = lane.owner_id.clone();
    let mut wrong_owner = spec.clone();
    wrong_owner.request_id = "wrong-owner".to_owned();
    wrong_owner.owner_identity = "not-the-lane-owner".to_owned();
    assert!(
        store
            .enqueue_build(&supervisor, &wrong_owner, at(0))
            .await
            .is_err()
    );
    store
        .enqueue_build(&supervisor, &spec, at(0))
        .await
        .unwrap();

    store.release_lane(&lane, at(1)).await.unwrap();
    let replacement = store
        .acquire_lane(
            "lane-a",
            "owner-b",
            &supervisor,
            at(1),
            Duration::minutes(2),
        )
        .await
        .unwrap();
    assert_ne!(replacement.fence, lane.fence);
    assert!(
        store
            .start_build(
                &supervisor,
                "lane-request",
                &ProcessIdentity {
                    pid: 55,
                    started_at_ms: 55,
                },
                at(1),
            )
            .await
            .is_err()
    );
}
