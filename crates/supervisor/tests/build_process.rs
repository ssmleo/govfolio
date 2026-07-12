#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use loop_supervisor::model::CommandSpec;
use loop_supervisor::process::{
    ProcessRunner, RawProcessEvent, RawProcessOutputPaths, cancellation_pair,
    observed_process_activity, should_retry_build_failure,
};

#[test]
fn build_process_samples_owned_process_cpu_progress() {
    assert!(
        observed_process_activity(std::process::id())
            .unwrap()
            .is_some()
    );
}

#[test]
fn build_process_retries_only_whitelisted_transient_failures_once() {
    assert!(should_retry_build_failure(
        Some(101),
        b"spurious network error: failed to download registry index",
        false,
        0,
    ));
    assert!(should_retry_build_failure(
        Some(1),
        b"The process cannot access the file because it is being used by another process",
        false,
        0,
    ));
    assert!(!should_retry_build_failure(
        Some(101),
        b"error[E0308]: mismatched types",
        false,
        0,
    ));
    assert!(!should_retry_build_failure(
        Some(101),
        b"test result: FAILED. 1 failed",
        false,
        0,
    ));
    assert!(!should_retry_build_failure(
        Some(101),
        b"spurious network error",
        false,
        1,
    ));
    assert!(!should_retry_build_failure(
        Some(101),
        b"spurious network error",
        true,
        0,
    ));
}
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

#[tokio::test]
async fn build_process_streams_raw_stdout_stderr_and_preserves_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let output = RawProcessOutputPaths {
        stdout: temp.path().join("stdout.log"),
        stderr: temp.path().join("stderr.log"),
    };
    let command = shell_command(
        temp.path(),
        "[Console]::Out.Write('out'); [Console]::Error.Write('err'); exit 37",
        "printf out; printf err >&2; exit 37",
    );
    let (events_tx, mut events_rx) = mpsc::channel(16);
    let (pid_tx, pid_rx) = oneshot::channel();
    let (_cancel, cancellation) = cancellation_pair();
    let execution = ProcessRunner::default()
        .run_raw(&command, &output, cancellation, events_tx, pid_tx)
        .await
        .unwrap();
    let identity = pid_rx.await.unwrap();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while let Some(event) = events_rx.recv().await {
        match event {
            RawProcessEvent::Stdout(bytes) => stdout.extend(bytes),
            RawProcessEvent::Stderr(bytes) => stderr.extend(bytes),
            RawProcessEvent::Progress => {}
        }
    }
    assert!(identity.pid > 0);
    assert_eq!(execution.exit_code, Some(37));
    assert!(!execution.cancelled);
    assert_eq!(stdout, b"out");
    assert_eq!(stderr, b"err");
    assert_eq!(std::fs::read(output.stdout).unwrap(), b"out");
    assert_eq!(std::fs::read(output.stderr).unwrap(), b"err");
}

#[tokio::test]
async fn build_process_cancellation_reaps_the_owned_raw_process() {
    let temp = tempfile::tempdir().unwrap();
    let output = RawProcessOutputPaths {
        stdout: temp.path().join("stdout.log"),
        stderr: temp.path().join("stderr.log"),
    };
    let command = shell_command(
        temp.path(),
        "[Console]::Out.WriteLine('ready'); Start-Sleep -Seconds 60",
        "printf 'ready\\n'; sleep 60",
    );
    let (events_tx, mut events_rx) = mpsc::channel(16);
    let (pid_tx, _pid_rx) = oneshot::channel();
    let (cancel, cancellation) = cancellation_pair();
    let runner = ProcessRunner::default();
    let task = tokio::spawn(async move {
        runner
            .run_raw(&command, &output, cancellation, events_tx, pid_tx)
            .await
    });
    while let Some(event) = events_rx.recv().await {
        if matches!(event, RawProcessEvent::Stdout(ref bytes) if bytes.windows(5).any(|window| window == b"ready"))
        {
            break;
        }
    }
    cancel.cancel();
    let execution = timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(execution.cancelled);
}

fn shell_command(cwd: &Path, windows: &str, unix: &str) -> CommandSpec {
    if cfg!(windows) {
        CommandSpec {
            program: PathBuf::from("powershell.exe"),
            args: vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                windows.to_owned(),
            ],
            cwd: cwd.to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    } else {
        CommandSpec {
            program: PathBuf::from("sh"),
            args: vec!["-c".to_owned(), unix.to_owned()],
            cwd: cwd.to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    }
}
