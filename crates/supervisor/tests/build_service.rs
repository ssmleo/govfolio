#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Duration, Utc};
use loop_supervisor::build_classifier::ResourceClass;
use loop_supervisor::build_policy::{BuildPolicySnapshot, BuildPolicyStatus};
use loop_supervisor::build_protocol::{
    BuildControlRequest, BuildRequestMessage, ClientEnvelope, ControlEndpoint, PROTOCOL_VERSION,
    ServerFrame, load_or_create_control_token, read_json_line, write_json_line,
};
use loop_supervisor::build_scheduler::{BuildAdmissionConfig, ResourceSnapshot};
use loop_supervisor::build_service::{
    BuildAdmissionServer, BuildServerOptions, execute_control_request,
};
use loop_supervisor::build_store::BuildRequestState;
use loop_supervisor::build_transport::connect_local_control;
use loop_supervisor::store::ControlStore;
use tokio::io::BufReader;
use tokio::sync::watch;
use tokio::time::{Duration as TokioDuration, timeout};

static SERVICE_TEST_RUNNING: AtomicBool = AtomicBool::new(false);

struct ServiceTestGuard;

impl Drop for ServiceTestGuard {
    fn drop(&mut self) {
        SERVICE_TEST_RUNNING.store(false, Ordering::Release);
    }
}

async fn acquire_service_test() -> ServiceTestGuard {
    while SERVICE_TEST_RUNNING
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        tokio::time::sleep(TokioDuration::from_millis(10)).await;
    }
    ServiceTestGuard
}

#[tokio::test(flavor = "current_thread")]
async fn build_protocol_server_executes_cargo_and_records_real_terminal_result() {
    let _serial = acquire_service_test().await;
    let temp = tempfile::tempdir().unwrap();
    let state_root = temp.path().join("state");
    let worktree = temp.path().join("worktree");
    let target = worktree.join("target-private");
    std::fs::create_dir_all(&state_root).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    let (program, prefix) = cargo_stub(temp.path());
    let store = Arc::new(
        ControlStore::open_writer(state_root.join("control.sqlite3"))
            .await
            .unwrap(),
    );
    let supervisor = store
        .acquire_supervisor("build-server", Utc::now(), Duration::minutes(5))
        .await
        .unwrap();
    let snapshot = BuildPolicySnapshot {
        schema_version: 1,
        policy_sha256: "a".repeat(64),
        status: BuildPolicyStatus::Advisory,
        source_commit: "abc".to_owned(),
        loaded_at: Utc::now(),
    };
    store.record_build_policy_snapshot(&snapshot).await.unwrap();
    let token = load_or_create_control_token(&state_root).unwrap();
    let config = BuildAdmissionConfig::from_values(16, &std::collections::BTreeMap::new()).unwrap();
    let server = BuildAdmissionServer::new(BuildServerOptions {
        state_root: state_root.clone(),
        repository: worktree.clone(),
        bronze_roots: Vec::new(),
        cargo_program: program,
        cargo_prefix_args: prefix,
        policy: snapshot.clone(),
        bounded_policy: "policy".to_owned(),
        control_token: token.clone(),
        config,
        resource_override: Some(ResourceSnapshot {
            available_memory_bytes: 16 * 1024 * 1024 * 1024,
            target_free_bytes: 30 * 1024 * 1024 * 1024,
            target_total_bytes: 100 * 1024 * 1024 * 1024,
        }),
        store: Arc::clone(&store),
        supervisor: supervisor.clone(),
    });
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_task = tokio::spawn(server.serve(shutdown_rx));

    let frames = execute_control_request(
        &state_root,
        &ClientEnvelope {
            protocol_version: PROTOCOL_VERSION,
            control_token: token,
            request: BuildControlRequest::Build(BuildRequestMessage {
                supervisor_fence: supervisor.fence,
                lane_id: None,
                lane_fence: None,
                owner_identity: "interactive:test".to_owned(),
                policy_sha256: snapshot.policy_sha256,
                explicit_class: Some(ResourceClass::Focused),
                category: Some("contract".to_owned()),
                worktree: worktree.clone(),
                target_dir: target,
                cargo_args: vec!["check".to_owned(), "-p".to_owned(), "core".to_owned()],
            }),
        },
    )
    .await
    .unwrap();

    let diagnostic_requests = store.list_build_requests().await.unwrap();
    assert!(
        frames.iter().any(|frame| matches!(
            frame,
            ServerFrame::Admission {
                effective_jobs: 6,
                ..
            }
        )),
        "frames={frames:?} requests={diagnostic_requests:?}"
    );
    assert!(frames.iter().any(|frame| matches!(frame, ServerFrame::Stdout { bytes, .. } if String::from_utf8_lossy(bytes.as_slice()).contains("stub-out"))));
    assert!(frames.iter().any(|frame| matches!(frame, ServerFrame::Stderr { bytes, .. } if String::from_utf8_lossy(bytes.as_slice()).contains("stub-err"))));
    assert!(frames.iter().any(|frame| matches!(
        frame,
        ServerFrame::Terminal {
            state: BuildRequestState::Failed,
            exit_code: Some(23),
            ..
        }
    )));

    let requests = store.list_build_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].state, BuildRequestState::Failed);
    assert_eq!(requests[0].effective_jobs, 6);
    assert_eq!(requests[0].exit_code, Some(23));

    shutdown_tx.send(true).unwrap();
    timeout(TokioDuration::from_secs(5), server_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[expect(
    clippy::too_many_lines,
    reason = "the transport cancellation test owns a complete isolated server fixture"
)]
async fn build_protocol_running_client_disconnect_cancels_silent_owned_process() {
    let _serial = acquire_service_test().await;
    let temp = tempfile::tempdir().unwrap();
    let state_root = temp.path().join("state");
    let worktree = temp.path().join("worktree");
    let target = worktree.join("target-private");
    std::fs::create_dir_all(&state_root).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    let (program, prefix) = sleeping_stub(temp.path());
    let store = Arc::new(
        ControlStore::open_writer(state_root.join("control.sqlite3"))
            .await
            .unwrap(),
    );
    let supervisor = store
        .acquire_supervisor("disconnect-server", Utc::now(), Duration::minutes(5))
        .await
        .unwrap();
    let snapshot = BuildPolicySnapshot {
        schema_version: 1,
        policy_sha256: "d".repeat(64),
        status: BuildPolicyStatus::Advisory,
        source_commit: "disconnect".to_owned(),
        loaded_at: Utc::now(),
    };
    store.record_build_policy_snapshot(&snapshot).await.unwrap();
    let token = load_or_create_control_token(&state_root).unwrap();
    let server = BuildAdmissionServer::new(BuildServerOptions {
        state_root: state_root.clone(),
        repository: worktree.clone(),
        bronze_roots: Vec::new(),
        cargo_program: program,
        cargo_prefix_args: prefix,
        policy: snapshot.clone(),
        bounded_policy: "policy".to_owned(),
        control_token: token.clone(),
        config: BuildAdmissionConfig::from_values(16, &std::collections::BTreeMap::new()).unwrap(),
        resource_override: Some(ResourceSnapshot {
            available_memory_bytes: 16 * 1024 * 1024 * 1024,
            target_free_bytes: 30 * 1024 * 1024 * 1024,
            target_total_bytes: 100 * 1024 * 1024 * 1024,
        }),
        store: Arc::clone(&store),
        supervisor: supervisor.clone(),
    });
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_task = tokio::spawn(server.serve(shutdown_rx));
    let endpoint = ControlEndpoint::for_state_root(&state_root).unwrap();
    let stream = connect_local_control(&endpoint).await.unwrap();
    let (read, mut write) = tokio::io::split(stream);
    write_json_line(
        &mut write,
        &ClientEnvelope {
            protocol_version: PROTOCOL_VERSION,
            control_token: token,
            request: BuildControlRequest::Build(BuildRequestMessage {
                supervisor_fence: supervisor.fence,
                lane_id: None,
                lane_fence: None,
                owner_identity: "disconnect:test".to_owned(),
                policy_sha256: snapshot.policy_sha256,
                explicit_class: Some(ResourceClass::Focused),
                category: None,
                worktree,
                target_dir: target,
                cargo_args: vec!["check".to_owned(), "-p".to_owned(), "core".to_owned()],
            }),
        },
    )
    .await
    .unwrap();
    let mut read = BufReader::new(read);
    loop {
        let Some(frame): Option<ServerFrame> = read_json_line(&mut read).await.unwrap() else {
            panic!(
                "server closed before admission: {:?}",
                store.list_build_requests().await.unwrap()
            );
        };
        if matches!(frame, ServerFrame::Admission { .. }) {
            break;
        }
    }
    drop(read);
    drop(write);

    timeout(TokioDuration::from_secs(5), async {
        loop {
            let requests = store.list_build_requests().await.unwrap();
            if requests
                .first()
                .is_some_and(|request| request.state == BuildRequestState::Cancelled)
            {
                break;
            }
            tokio::time::sleep(TokioDuration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    shutdown_tx.send(true).unwrap();
    timeout(TokioDuration::from_secs(5), server_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

fn cargo_stub(root: &Path) -> (PathBuf, Vec<String>) {
    if cfg!(windows) {
        let script = root.join("cargo-stub.ps1");
        std::fs::write(
            &script,
            "[Console]::Out.WriteLine('stub-out:' + ($args -join ' ')); [Console]::Error.WriteLine('stub-err'); exit 23",
        )
        .unwrap();
        (
            PathBuf::from("powershell.exe"),
            vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-File".to_owned(),
                script.to_string_lossy().into_owned(),
            ],
        )
    } else {
        let script = root.join("cargo-stub.sh");
        std::fs::write(
            &script,
            "printf 'stub-out:%s\\n' \"$*\"; printf 'stub-err\\n' >&2; exit 23\n",
        )
        .unwrap();
        (
            PathBuf::from("sh"),
            vec![script.to_string_lossy().into_owned()],
        )
    }
}

fn sleeping_stub(root: &Path) -> (PathBuf, Vec<String>) {
    if cfg!(windows) {
        let script = root.join("cargo-sleep.ps1");
        std::fs::write(&script, "Start-Sleep -Seconds 60; exit 0").unwrap();
        (
            PathBuf::from("powershell.exe"),
            vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-File".to_owned(),
                script.to_string_lossy().into_owned(),
            ],
        )
    } else {
        (
            PathBuf::from("sh"),
            vec!["-c".to_owned(), "sleep 60".to_owned()],
        )
    }
}
