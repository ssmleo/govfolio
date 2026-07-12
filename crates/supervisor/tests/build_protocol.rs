#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use loop_supervisor::build_classifier::ResourceClass;
use loop_supervisor::build_protocol::{
    BuildControlRequest, BuildRequestMessage, ClientEnvelope, ControlEndpoint, PROTOCOL_VERSION,
    ServerFrame, load_or_create_control_token, read_json_line, validate_envelope, write_json_line,
};
use tokio::io::BufReader;

fn envelope(token: &str) -> ClientEnvelope {
    ClientEnvelope {
        protocol_version: PROTOCOL_VERSION,
        control_token: token.to_owned(),
        request: BuildControlRequest::Build(BuildRequestMessage {
            supervisor_fence: 7,
            lane_id: None,
            lane_fence: None,
            owner_identity: "interactive:test".to_owned(),
            policy_sha256: "a".repeat(64),
            explicit_class: Some(ResourceClass::Focused),
            category: Some("test".to_owned()),
            worktree: PathBuf::from("C:/repo/lane"),
            target_dir: PathBuf::from("C:/repo/lane/target"),
            cargo_args: vec!["check".to_owned(), "-p".to_owned(), "core".to_owned()],
        }),
    }
}

#[tokio::test]
async fn build_protocol_round_trips_bounded_json_lines() {
    let (client, server) = tokio::io::duplex(16 * 1024);
    let expected = ServerFrame::QueueHeartbeat {
        request_id: "request-1".to_owned(),
        position: 2,
    };
    let writer = tokio::spawn(async move {
        let mut client = client;
        write_json_line(&mut client, &expected).await.unwrap();
        expected
    });
    let mut server = BufReader::new(server);
    let actual: ServerFrame = read_json_line(&mut server).await.unwrap().unwrap();
    assert_eq!(actual, writer.await.unwrap());
}

#[test]
fn build_protocol_authenticates_token_version_identity_policy_and_fence() {
    let token = "f".repeat(64);
    let valid = envelope(&token);
    validate_envelope(&valid, &token, 7, &"a".repeat(64)).unwrap();

    let mut wrong = valid.clone();
    wrong.control_token = "0".repeat(64);
    assert!(validate_envelope(&wrong, &token, 7, &"a".repeat(64)).is_err());
    wrong = valid.clone();
    wrong.protocol_version += 1;
    assert!(validate_envelope(&wrong, &token, 7, &"a".repeat(64)).is_err());
    wrong = valid.clone();
    if let BuildControlRequest::Build(build) = &mut wrong.request {
        build.supervisor_fence = 6;
    }
    assert!(validate_envelope(&wrong, &token, 7, &"a".repeat(64)).is_err());
    wrong = valid;
    if let BuildControlRequest::Build(build) = &mut wrong.request {
        build.policy_sha256 = "b".repeat(64);
    }
    assert!(validate_envelope(&wrong, &token, 7, &"a".repeat(64)).is_err());

    let mut invalid_interactive = envelope(&token);
    if let BuildControlRequest::Build(build) = &mut invalid_interactive.request {
        build.owner_identity = "unscoped-owner".to_owned();
    }
    assert!(validate_envelope(&invalid_interactive, &token, 7, &"a".repeat(64),).is_err());
}

#[test]
fn build_protocol_control_token_is_stable_and_endpoint_is_state_root_scoped() {
    let temp = tempfile::tempdir().unwrap();
    let first = load_or_create_control_token(temp.path()).unwrap();
    let second = load_or_create_control_token(temp.path()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let endpoint = ControlEndpoint::for_state_root(temp.path()).unwrap();
    if cfg!(windows) {
        assert!(endpoint.display().starts_with(r"\\.\pipe\govfolio-loop-"));
    } else {
        assert_eq!(
            endpoint.display(),
            temp.path().join("control.sock").to_string_lossy()
        );
    }
}
