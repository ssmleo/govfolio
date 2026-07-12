#![allow(clippy::unwrap_used, clippy::expect_used)]

use loop_supervisor::build_protocol::{
    BuildControlRequest, ClientEnvelope, ControlEndpoint, PROTOCOL_VERSION, ServerFrame,
    load_or_create_control_token, read_json_line, write_json_line,
};
use loop_supervisor::build_transport::{LocalControlListener, connect_local_control};
use tokio::io::BufReader;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn build_protocol_uses_the_real_host_local_transport() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = ControlEndpoint::for_state_root(temp.path()).unwrap();
    let token = load_or_create_control_token(temp.path()).unwrap();
    let mut listener = LocalControlListener::bind(&endpoint).unwrap();
    let expected_token = token.clone();
    let server = tokio::spawn(async move {
        let stream = listener.accept().await.unwrap();
        let (read, mut write) = tokio::io::split(stream);
        let mut read = BufReader::new(read);
        let envelope: ClientEnvelope = read_json_line(&mut read).await.unwrap().unwrap();
        assert_eq!(envelope.control_token, expected_token);
        write_json_line(
            &mut write,
            &ServerFrame::QueueHeartbeat {
                request_id: "transport".to_owned(),
                position: 1,
            },
        )
        .await
        .unwrap();
    });

    let stream = connect_local_control(&endpoint).await.unwrap();
    let (read, mut write) = tokio::io::split(stream);
    write_json_line(
        &mut write,
        &ClientEnvelope {
            protocol_version: PROTOCOL_VERSION,
            control_token: token,
            request: BuildControlRequest::Status,
        },
    )
    .await
    .unwrap();
    let mut read = BufReader::new(read);
    let response: ServerFrame = read_json_line(&mut read).await.unwrap().unwrap();
    assert_eq!(
        response,
        ServerFrame::QueueHeartbeat {
            request_id: "transport".to_owned(),
            position: 1,
        }
    );
    server.await.unwrap();
}

#[tokio::test]
async fn build_protocol_listener_survives_a_cancelled_accept_future() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = ControlEndpoint::for_state_root(temp.path()).unwrap();
    let mut listener = LocalControlListener::bind(&endpoint).unwrap();
    assert!(
        timeout(Duration::from_millis(10), listener.accept())
            .await
            .is_err()
    );
    let client = tokio::spawn({
        let endpoint = endpoint.clone();
        async move { connect_local_control(&endpoint).await.unwrap() }
    });
    let server_stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let _client_stream = client.await.unwrap();
    drop(server_stream);
}
