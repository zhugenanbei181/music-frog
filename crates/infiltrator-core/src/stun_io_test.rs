//! DUAL-14-09: offline tests for the real UDP STUN egress adapter.
//!
//! Every test stands up a local UDP responder on loopback and exercises the
//! real socket path; none of them needs network access. The one live test is
//! `#[ignore]`d and documented.

use super::*;
use infiltrator_contract::stun_probe::{StunMappedAddress, StunProbeStatus, StunProbeTransport};
use tokio::net::UdpSocket;

fn transaction_of(query: &[u8]) -> stun_wire::TransactionId {
    let mut transaction = [0u8; stun_wire::TRANSACTION_ID_LEN];
    transaction.copy_from_slice(&query[8..stun_wire::HEADER_LEN]);
    transaction
}

fn error_response(
    transaction: &stun_wire::TransactionId,
    class: u8,
    number: u8,
    reason: &str,
) -> Vec<u8> {
    let mut value = Vec::new();
    value.extend_from_slice(&[0, 0, class, number]);
    value.extend_from_slice(reason.as_bytes());
    let mut response = Vec::new();
    response.extend_from_slice(&stun_wire::BINDING_ERROR_RESPONSE.to_be_bytes());
    response.extend_from_slice(&((value.len() + 4) as u16).to_be_bytes());
    response.extend_from_slice(&stun_wire::MAGIC_COOKIE.to_be_bytes());
    response.extend_from_slice(transaction);
    response.extend_from_slice(&stun_wire::ATTR_ERROR_CODE.to_be_bytes());
    response.extend_from_slice(&(value.len() as u16).to_be_bytes());
    response.extend_from_slice(&value);
    response
}

/// Spawn a one-shot loopback UDP responder that answers with every datagram
/// `respond` returns, in order.
async fn responder(respond: impl FnOnce(&[u8]) -> Vec<Vec<u8>> + Send + 'static) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind responder");
    let address = socket.local_addr().expect("responder address");
    tokio::spawn(async move {
        let mut buffer = vec![0u8; 1024];
        if let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
            for reply in respond(&buffer[..length]) {
                let _ = socket.send_to(&reply, peer).await;
            }
        }
    });
    address
}

fn request(server: SocketAddr, timeout_ms: u32) -> StunProbeRequest {
    StunProbeRequest::new(&format!("127.0.0.1:{}", server.port())).with_timeout_ms(timeout_ms)
}

#[tokio::test]
async fn a_real_udp_loopback_responder_reports_the_mapping_it_crafted() {
    let expected = StunMappedAddress::new("203.0.113.9", 51234);
    let crafted = expected.clone();
    let server = responder(move |query| {
        let transaction = transaction_of(query);
        vec![stun_wire::encode_binding_success(&transaction, &crafted)]
    })
    .await;

    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(&probe, request(server, 1_000))
        .await
        .expect("observe");

    assert_eq!(observation.transport, StunProbeTransport::Udp);
    assert!(observation.status.is_observed());
    assert_eq!(observation.mapping, Some(expected));
    assert!(observation.server.starts_with("127.0.0.1:"));
}

#[tokio::test]
async fn an_ipv6_mapping_from_a_loopback_responder_is_reported_verbatim() {
    let expected = StunMappedAddress::new("2001:db8::9", 3478);
    let crafted = expected.clone();
    let server = responder(move |query| {
        let transaction = transaction_of(query);
        vec![stun_wire::encode_binding_success(&transaction, &crafted)]
    })
    .await;

    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(&probe, request(server, 1_000))
        .await
        .expect("observe");
    assert_eq!(observation.mapping, Some(expected));
}

#[tokio::test]
async fn a_stray_datagram_is_ignored_until_the_real_answer_arrives() {
    let expected = StunMappedAddress::new("198.51.100.7", 60000);
    let crafted = expected.clone();
    let server = responder(move |query| {
        let transaction = transaction_of(query);
        let mut wrong = transaction;
        wrong[0] ^= 0xff;
        vec![
            stun_wire::encode_binding_success(&wrong, &StunMappedAddress::new("10.0.0.1", 1)),
            stun_wire::encode_binding_success(&transaction, &crafted),
        ]
    })
    .await;

    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(&probe, request(server, 1_000))
        .await
        .expect("observe");
    assert_eq!(
        observation.mapping,
        Some(expected),
        "the matching answer wins, the stray does not become a mapping"
    );
}

#[tokio::test]
async fn a_silent_stun_server_times_out_without_a_mapping() {
    // The socket stays bound for the whole probe but never answers.
    let silent = UdpSocket::bind("127.0.0.1:0").await.expect("bind silent");
    let address = silent.local_addr().expect("silent address");

    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(&probe, request(address, 150))
        .await
        .expect("observe");

    assert_eq!(observation.status, StunProbeStatus::TimedOut);
    assert_eq!(observation.mapping, None);
}

#[tokio::test]
async fn a_server_error_response_is_a_typed_failure_not_a_mapping() {
    let server = responder(|query| {
        let transaction = transaction_of(query);
        vec![error_response(&transaction, 4, 1, "Unauthorized")]
    })
    .await;

    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(&probe, request(server, 1_000))
        .await
        .expect("observe");

    assert!(!observation.status.is_observed());
    assert_eq!(observation.mapping, None);
    let message = observation.status.failure().expect("failure message");
    assert!(message.contains("401"), "{message}");
    assert!(message.contains("Unauthorized"), "{message}");
}

#[tokio::test]
async fn an_unusable_server_address_fails_without_a_mapping() {
    let probe = UdpStunProbe::new();
    // A port that cannot be a `u16` is not a usable endpoint; no datagram is
    // sent and no mapping is invented.
    let observation = StunProbePort::observe(&probe, StunProbeRequest::new("127.0.0.1:99999"))
        .await
        .expect("observe");
    assert!(matches!(observation.status, StunProbeStatus::Failed { .. }));
    assert_eq!(observation.mapping, None);

    let empty = StunProbePort::observe(&probe, StunProbeRequest::new("   "))
        .await
        .expect("observe");
    assert!(matches!(empty.status, StunProbeStatus::Failed { .. }));
    assert_eq!(empty.mapping, None);
}

/// Requires live network access to a public STUN server; not part of the
/// offline suite. Run with `cargo nextest run -p infiltrator-core
/// --features network-tests --run-ignored ignored-only live_public_stun`.
#[cfg(feature = "network-tests")]
#[tokio::test]
#[ignore = "requires live network access to a public STUN server"]
async fn live_public_stun_server_observes_this_hosts_udp_mapping() {
    let probe = UdpStunProbe::new();
    let observation = StunProbePort::observe(
        &probe,
        StunProbeRequest::new(infiltrator_contract::stun_probe::DEFAULT_STUN_SERVER)
            .with_timeout_ms(4_000),
    )
    .await
    .expect("observe");

    match observation.status {
        StunProbeStatus::Observed => {
            let mapping = observation.mapping.expect("an observed mapping");
            assert!(!mapping.ip.is_empty(), "the observed IP must not be empty");
        }
        other => panic!("live STUN probe did not observe a mapping: {other:?}"),
    }
}
