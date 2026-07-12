#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

#[test]
fn build_protocol_cli_fails_closed_with_exit_75_when_server_is_absent() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_govfolio-loop"))
        .args(["cargo", "--", "check", "-p", "core"])
        .env("GOVFOLIO_LOOP_STATE_DIR", temp.path())
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(75));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .to_ascii_lowercase()
            .contains("admission server")
    );
}

#[test]
fn build_protocol_cli_help_lists_the_admission_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_govfolio-loop"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in ["serve-builds", "build-policy", "cargo", "recover-build"] {
        assert!(stdout.contains(command), "missing {command} in help");
    }
}

#[test]
fn build_protocol_cli_allows_unmanaged_version_passthrough_without_server() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_govfolio-loop"))
        .args(["cargo", "--", "--version"])
        .env("GOVFOLIO_LOOP_STATE_DIR", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("cargo "));
}
