//! DUAL-14-10: deterministic, offline-safe prober tests.
//!
//! Every test drives the real adapter against a loopback nameserver or a
//! loopback DoH endpoint. No test reaches the public internet.

use super::*;
use crate::dns_wire;
use infiltrator_contract::dns_latency::{DnsLatencyProbeRequest, DnsProbeTarget};
use std::net::SocketAddr;

/// Independent decode of the query the prober sent, used by the loopback
/// nameserver to answer the exact id/question it received.
fn decode_query(bytes: &[u8]) -> Option<DnsQuestion> {
    if bytes.len() < dns_wire::HEADER_LEN {
        return None;
    }
    let id = u16::from_be_bytes([bytes[0], bytes[1]]);
    let mut offset = dns_wire::HEADER_LEN;
    let mut labels = Vec::new();
    loop {
        let length = *bytes.get(offset)? as usize;
        offset += 1;
        if length == 0 {
            break;
        }
        let label = bytes.get(offset..offset + length)?;
        labels.push(String::from_utf8_lossy(label).to_string());
        offset += length;
    }
    let qtype = u16::from_be_bytes([*bytes.get(offset)?, *bytes.get(offset + 1)?]);
    Some(DnsQuestion {
        id,
        qname: labels.join("."),
        qtype,
    })
}

/// A loopback nameserver answering (or deliberately ignoring) every query.
async fn spawn_udp_nameserver(
    responder: impl Fn(&DnsQuestion) -> Option<Vec<u8>> + Send + Sync + 'static,
) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind loopback nameserver");
    let address = socket.local_addr().expect("loopback address");
    tokio::spawn(async move {
        let mut buffer = [0u8; 512];
        while let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
            let Some(question) = decode_query(&buffer[..length]) else {
                continue;
            };
            if let Some(answer) = responder(&question) {
                let _ = socket.send_to(&answer, peer).await;
            }
        }
    });
    address
}

fn target(address: &str) -> DnsProbeTarget {
    DnsProbeTarget::new(address, false)
}

fn request(addresses: &[&str], timeout_ms: u32) -> DnsLatencyProbeRequest {
    DnsLatencyProbeRequest::new(addresses.iter().map(|value| target(value)).collect())
        .with_timeout_ms(timeout_ms)
}

#[tokio::test]
async fn a_real_udp_nameserver_on_loopback_is_measured() {
    let address = spawn_udp_nameserver(|question| {
        assert_eq!(question.qtype, dns_wire::TYPE_A);
        Some(dns_wire::encode_answer(question, [203, 0, 113, 9], 60))
    })
    .await;

    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&address.to_string()], 1_500);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    assert_eq!(result.transport, DnsProbeTransport::Udp);
    assert!(
        result.outcome.is_measured(),
        "a real answer must be measured: {:?}",
        result.outcome
    );
    assert!(result.outcome.rtt_ms().expect("rtt") < 1_500);
    assert!(!result.is_fallback);
}

#[tokio::test]
async fn a_wrong_response_id_is_rejected_instead_of_timed() {
    let address = spawn_udp_nameserver(|question| {
        let mut answer = dns_wire::encode_answer(question, [203, 0, 113, 9], 60);
        let wrong = question.id.wrapping_add(1);
        answer[0..2].copy_from_slice(&wrong.to_be_bytes());
        Some(answer)
    })
    .await;

    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&address.to_string()], 1_500);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    match &result.outcome {
        DnsProbeOutcome::InvalidResponse { reason } => {
            assert!(reason.contains("does not match"), "{reason}");
        }
        other => panic!("a wrong id must not be measured: {other:?}"),
    }
    assert_eq!(result.outcome.rtt_ms(), None);
}

#[tokio::test]
async fn a_silent_nameserver_times_out_without_a_number() {
    let address = spawn_udp_nameserver(|_| None).await;

    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&address.to_string()], 150);
    let started = std::time::Instant::now();
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    assert_eq!(result.outcome, DnsProbeOutcome::TimedOut);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn an_unreachable_port_is_not_measured_but_is_still_reported() {
    // A port that was free a moment ago: the kernel answers the connected
    // datagram with ICMP unreachable, which is a real failure, not a number.
    let free = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
    let port = free.local_addr().expect("addr").port();
    drop(free);

    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&format!("127.0.0.1:{port}")], 400);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    assert_eq!(result.outcome.rtt_ms(), None);
    assert!(
        !matches!(result.outcome, DnsProbeOutcome::NotProbed { .. }),
        "the address was attempted, so it is never reported as not probed"
    );
}

#[tokio::test]
async fn a_doh_endpoint_answers_the_wire_format_query() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/dns-query")
        .match_header("content-type", "application/dns-message")
        .match_header("accept", "application/dns-message")
        .with_body_from_request(|request| {
            let question = decode_query(request.body().expect("wire query")).expect("question");
            dns_wire::encode_answer(&question, [198, 51, 100, 7], 30)
        })
        .create_async()
        .await;

    let url = format!("{}/dns-query", server.url());
    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&url], 2_000);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    assert_eq!(result.transport, DnsProbeTransport::Doh);
    assert!(
        result.outcome.is_measured(),
        "the DoH wire exchange must be measured: {:?}",
        result.outcome
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn a_doh_endpoint_with_a_wrong_id_is_rejected() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/dns-query")
        .with_body_from_request(|request| {
            let question = decode_query(request.body().expect("wire query")).expect("question");
            let mut answer = dns_wire::encode_answer(&question, [198, 51, 100, 7], 30);
            answer[0..2].copy_from_slice(&0u16.to_be_bytes());
            answer
        })
        .create_async()
        .await;

    let prober = HttpDnsLatencyProber::new();
    let url = format!("{}/dns-query", server.url());
    let probe = request(&[&url], 2_000);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    assert!(matches!(
        result.outcome,
        DnsProbeOutcome::InvalidResponse { .. }
    ));
}

#[tokio::test]
async fn a_doh_endpoint_error_status_is_a_failure_not_a_measurement() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/dns-query")
        .with_status(503)
        .create_async()
        .await;

    let prober = HttpDnsLatencyProber::new();
    let url = format!("{}/dns-query", server.url());
    let probe = request(&[&url], 2_000);
    let result = prober.probe_target(&probe.targets[0], &probe).await;

    match &result.outcome {
        DnsProbeOutcome::Failed { message } => assert!(message.contains("503"), "{message}"),
        other => panic!("an HTTP error is not a measurement: {other:?}"),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn undrivable_transports_are_reported_instead_of_probed() {
    let prober = HttpDnsLatencyProber::new();
    for address in [
        "tls://1.0.0.1:853",
        "h3://dns.google/dns-query",
        "quic://dns.adguard.com",
        "sdns://stamp",
        "dhcp://en0",
        "system",
        "ftp://dns.example",
        "   ",
    ] {
        let probe = request(&[address], 200);
        let result = prober.probe_target(&probe.targets[0], &probe).await;
        assert!(
            matches!(result.transport, DnsProbeTransport::Undrivable { .. }),
            "{address} must not claim a drivable transport"
        );
        assert!(
            matches!(result.outcome, DnsProbeOutcome::NotProbed { .. }),
            "{address} must not be probed"
        );
    }
}

#[tokio::test]
async fn the_port_probes_every_configured_nameserver_and_refuses_an_empty_list() {
    let address = spawn_udp_nameserver(|question| {
        Some(dns_wire::encode_answer(question, [203, 0, 113, 9], 60))
    })
    .await;
    let prober = HttpDnsLatencyProber::new();
    let probe = request(&[&address.to_string(), "tls://1.0.0.1:853"], 1_500);
    let report = DnsLatencyProbePort::probe(&prober, probe)
        .await
        .expect("probe");

    assert!(report.status.is_ready());
    assert_eq!(
        report.question,
        infiltrator_contract::dns_latency::DEFAULT_PROBE_QUESTION
    );
    assert_eq!(report.results.len(), 2);
    assert_eq!(report.measured_count(), 1);
    assert!(report.latency_of(&address.to_string()).is_some());
    assert_eq!(report.latency_of("tls://1.0.0.1:853"), None);
    assert!(!report.is_upstream_unreachable());

    let empty = request(&[], 500);
    assert!(DnsLatencyProbePort::probe(&prober, empty).await.is_err());
}

#[test]
fn plan_decoding_covers_both_surface_editor_shapes() {
    let cases: [(&str, ProbePlanKind); 9] = [
        ("223.5.5.5", ProbePlanKind::Udp(53)),
        ("8.8.8.8:5353", ProbePlanKind::Udp(5353)),
        ("udp://9.9.9.9:5300", ProbePlanKind::Udp(5300)),
        ("https://dns.google/dns-query", ProbePlanKind::Doh),
        ("https://dns.google/dns-query#Proxy", ProbePlanKind::Doh),
        ("http://127.0.0.1:8080/dns-query", ProbePlanKind::Doh),
        ("doh://dns.pub/dns-query", ProbePlanKind::Doh),
        ("[2001:db8::1]:5353", ProbePlanKind::Udp(5353)),
        ("2001:db8::1", ProbePlanKind::Udp(53)),
    ];
    for (address, expected) in cases {
        match (plan_for(address), expected) {
            (ProbePlan::Udp { port, .. }, ProbePlanKind::Udp(expected)) => assert_eq!(
                port, expected,
                "{address} decoded to port {port}, expected {expected}"
            ),
            (ProbePlan::Doh { .. }, ProbePlanKind::Doh) => {}
            (plan, _) => panic!("{address} decoded unexpectedly: {plan:?}"),
        }
    }
    assert!(matches!(
        plan_for("doh://dns.pub/dns-query"),
        ProbePlan::Doh { .. }
    ));
    if let ProbePlan::Doh { url } = plan_for("doh://dns.pub/dns-query") {
        assert_eq!(url, "https://dns.pub/dns-query");
    }
    if let ProbePlan::Doh { url } = plan_for("https://dns.google/dns-query#Proxy") {
        assert_eq!(url, "https://dns.google/dns-query");
    }
    assert!(matches!(
        plan_for("tls://1.0.0.1:853"),
        ProbePlan::Undrivable { .. }
    ));
    assert!(matches!(
        plan_for("[not-an-ip]:53"),
        ProbePlan::Undrivable { .. }
    ));
}

enum ProbePlanKind {
    Udp(u16),
    Doh,
}

#[test]
fn the_measured_round_trip_is_clamped_into_the_shared_range() {
    assert_eq!(clamp_rtt(Duration::from_millis(7)), 7);
    assert_eq!(
        clamp_rtt(Duration::from_secs(120)),
        infiltrator_contract::dns_latency::MAX_REPORTED_RTT_MS
    );
    // Two probes never share a transaction id.
    let first = next_query_id();
    let second = next_query_id();
    assert_ne!(first, second);
}
