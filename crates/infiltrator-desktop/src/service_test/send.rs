//! Behavior cases for send.
//! test-intent: behavior

use super::*;
use tokio::io::{BufReader, duplex, split};

#[tokio::test]
async fn test_send_and_recv_framed_json() {
    let (mut client_io, mut server_io) = duplex(4096);

    let sample_req =
        ServiceRequest::new("id-123", "token-xyz", ServiceCommand::Ping { nonce: 777 });

    tokio::spawn(async move {
        send_framed_json(&mut client_io, &sample_req).await.unwrap();
    });

    let (server_reader, _) = split(&mut server_io);
    let mut buf_reader = BufReader::new(server_reader);
    let received: ServiceRequest = recv_framed_json(&mut buf_reader).await.unwrap();

    assert_eq!(received.id, "id-123");
    assert_eq!(received.auth_token, "token-xyz");
    assert_eq!(received.command, ServiceCommand::Ping { nonce: 777 });
}
