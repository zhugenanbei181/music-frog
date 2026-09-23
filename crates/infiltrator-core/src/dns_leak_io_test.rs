//! DUAL-14-08: deterministic, offline-safe echo probe tests.
//!
//! Every test drives the real adapter against a loopback echo authority or a
//! loopback DoH endpoint. No test reaches the public internet, and no test
//! lets the platform resolver turn into an observation: the system path is
//! exercised against the reserved `.invalid` TLD, which can never resolve.

use super::*;
use crate::dns_wire;
use std::net::SocketAddr;

/// Independent decode of the query the prober sent, used by the loopback
/// authority to answer the exact id/question it received.
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

/// A loopback echo authority answering (or deliberately ignoring) every query.
async fn spawn_udp_authority(
    responder: impl Fn(&DnsQuestion, SocketAddr) -> Option<Vec<u8>> + Send + Sync + 'static,
) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind loopback authority");
    let address = socket.local_addr().expect("loopback address");
    tokio::spawn(async move {
        let mut buffer = [0u8; 512];
        while let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
            let Some(question) = decode_query(&buffer[..length]) else {
                continue;
            };
            if let Some(answer) = responder(&question, peer) {
                let _ = socket.send_to(&answer, peer).await;
            }
        }
    });
    address
}

fn probe(resolver: &str, question: &str) -> DnsLeakEchoProbe {
    DnsLeakEchoProbe {
        resolver: resolver.to_owned(),
        authority: "echo.example.org".to_owned(),
        question: question.to_owned(),
    }
}

fn request(probes: Vec<DnsLeakEchoProbe>, timeout_ms: u32) -> DnsLeakEchoRequest {
    DnsLeakEchoRequest::new(probes).with_timeout_ms(timeout_ms)
}

#[tokio::test]
async fn a_real_udp_echo_authority_reports_the_resolver_it_observed() {
    // The authority answers with the source address it actually observed,
    // which is the whole point of the echo fact.
    let address = spawn_udp_authority(|question, peer| {
        assert_eq!(question.qtype, dns_wire::TYPE_A);
        let octets = match peer.ip() {
            std::net::IpAddr::V4(v4) => v4.octets(),
            std::net::IpAddr::V6(_) => [127, 0, 0, 1],
        };
        Some(dns_wire::encode_answer(question, octets, 60))
    })
    .await;

    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&address.to_string(), "l42.echo.example.org");
    let observation = prober.observe_probe(&probe, &request(vec![], 1_500)).await;

    assert_eq!(observation.transport, DnsLeakProbeTransport::Udp);
    assert_eq!(observation.question, "l42.echo.example.org");
    assert_eq!(
        observation.outcome,
        DnsLeakObservationOutcome::Observed {
            identity: "127.0.0.1".to_owned()
        }
    );
}

#[tokio::test]
async fn an_answer_without_an_a_record_is_not_an_identity() {
    let address = spawn_udp_authority(|question, _| {
        let qname = dns_wire::encode_qname(&question.qname).expect("qname");
        let mut answer = dns_wire::encode_answer(question, [127, 0, 0, 1], 60);
        answer.truncate(dns_wire::HEADER_LEN + qname.len() + 4);
        answer[6..8].copy_from_slice(&0u16.to_be_bytes());
        Some(answer)
    })
    .await;

    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&address.to_string(), "l43.echo.example.org");
    let observation = prober.observe_probe(&probe, &request(vec![], 1_500)).await;

    match &observation.outcome {
        DnsLeakObservationOutcome::InvalidResponse { reason } => {
            assert!(reason.contains("without an A record"), "{reason}");
        }
        other => panic!("an empty answer must not become an identity: {other:?}"),
    }
}

#[tokio::test]
async fn a_nonzero_rcode_is_reported_instead_of_read_as_an_identity() {
    let address = spawn_udp_authority(|question, _| {
        let mut answer = dns_wire::encode_answer(question, [127, 0, 0, 1], 60);
        let flags = u16::from_be_bytes([answer[2], answer[3]]) | 0x0003;
        answer[2..4].copy_from_slice(&flags.to_be_bytes());
        Some(answer)
    })
    .await;

    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&address.to_string(), "l44.echo.example.org");
    let observation = prober.observe_probe(&probe, &request(vec![], 1_500)).await;

    match &observation.outcome {
        DnsLeakObservationOutcome::InvalidResponse { reason } => {
            assert!(reason.contains("rcode 3"), "{reason}");
        }
        other => panic!("NXDOMAIN must not become an identity: {other:?}"),
    }
}

#[tokio::test]
async fn a_wrong_response_id_is_rejected_before_it_is_observed() {
    let address = spawn_udp_authority(|question, _| {
        let mut answer = dns_wire::encode_answer(question, [127, 0, 0, 1], 60);
        let wrong = question.id.wrapping_add(1);
        answer[0..2].copy_from_slice(&wrong.to_be_bytes());
        Some(answer)
    })
    .await;

    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&address.to_string(), "l45.echo.example.org");
    let observation = prober.observe_probe(&probe, &request(vec![], 1_500)).await;

    match &observation.outcome {
        DnsLeakObservationOutcome::InvalidResponse { reason } => {
            assert!(reason.contains("does not match"), "{reason}");
        }
        other => panic!("a wrong id must not be observed: {other:?}"),
    }
}

#[tokio::test]
async fn a_silent_echo_authority_times_out() {
    let address = spawn_udp_authority(|_, _| None).await;

    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&address.to_string(), "l46.echo.example.org");
    let started = std::time::Instant::now();
    let observation = prober.observe_probe(&probe, &request(vec![], 150)).await;

    assert_eq!(observation.outcome, DnsLeakObservationOutcome::TimedOut);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn a_doh_echo_endpoint_answers_the_wire_format_query() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/dns-query")
        .match_header("content-type", "application/dns-message")
        .match_header("accept", "application/dns-message")
        .with_body_from_request(|request| {
            let question = decode_query(request.body().expect("wire query")).expect("question");
            assert!(question.qname.ends_with(".echo.example.org"));
            dns_wire::encode_answer(&question, [203, 0, 113, 7], 30)
        })
        .create_async()
        .await;

    let url = format!("{}/dns-query", server.url());
    let prober = HttpDnsLeakEchoProbe::new();
    let probe = probe(&url, "l47.echo.example.org");
    let observation = prober.observe_probe(&probe, &request(vec![], 2_000)).await;

    assert_eq!(observation.transport, DnsLeakProbeTransport::Doh);
    assert_eq!(
        observation.outcome,
        DnsLeakObservationOutcome::Observed {
            identity: "203.0.113.7".to_owned()
        }
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn undrivable_resolvers_are_reported_instead_of_probed() {
    let prober = HttpDnsLeakEchoProbe::new();
    for resolver in [
        "tls://1.0.0.1:853",
        "h3://dns.google/dns-query",
        "quic://dns.adguard.com",
        "sdns://stamp",
        "dhcp://en0",
        "ftp://dns.example",
        "   ",
    ] {
        let probe = probe(resolver, "l48.echo.example.org");
        let observation = prober.observe_probe(&probe, &request(vec![], 200)).await;
        assert!(
            matches!(
                observation.transport,
                DnsLeakProbeTransport::Undrivable { .. }
            ),
            "{resolver} must not claim a drivable transport"
        );
        assert!(
            matches!(
                observation.outcome,
                DnsLeakObservationOutcome::NotProbed { .. }
            ),
            "{resolver} must not be probed"
        );
    }
}

#[tokio::test]
async fn the_platform_resolver_path_reports_its_real_outcome() {
    let prober = HttpDnsLeakEchoProbe::new();
    // `.invalid` is reserved (RFC 2606) and can never resolve, so this can
    // only be a failure — never a fabricated identity.
    let probe = probe("system", "l49.echo.invalid");
    let observation = prober.observe_probe(&probe, &request(vec![], 1_500)).await;

    assert_eq!(observation.transport, DnsLeakProbeTransport::System);
    match &observation.outcome {
        DnsLeakObservationOutcome::Failed { message } => {
            assert!(!message.is_empty());
        }
        DnsLeakObservationOutcome::TimedOut => {}
        other => panic!("a reserved name must not resolve to an identity: {other:?}"),
    }
}

#[tokio::test]
async fn the_echo_port_probes_every_source_and_refuses_an_empty_list() {
    let address = spawn_udp_authority(|question, _| {
        Some(dns_wire::encode_answer(question, [198, 51, 100, 9], 30))
    })
    .await;

    let prober = HttpDnsLeakEchoProbe::new();
    let first = probe(&address.to_string(), "l50.echo.example.org");
    let report = DnsLeakEchoPort::observe(
        &prober,
        request(
            vec![first, probe("tls://1.0.0.1:853", "l51.echo.example.org")],
            1_500,
        ),
    )
    .await
    .expect("observe");

    assert_eq!(report.observations.len(), 2);
    assert_eq!(
        report.identity_of("l50.echo.example.org"),
        Some("198.51.100.9")
    );
    assert_eq!(report.identity_of("l51.echo.example.org"), None);
    assert!(report.observations[1].outcome.identity().is_none());

    assert!(
        DnsLeakEchoPort::observe(&prober, request(Vec::new(), 500))
            .await
            .is_err()
    );
}

#[test]
fn plan_decoding_covers_every_echo_resolver_shape() {
    assert!(matches!(plan_for("223.5.5.5"), ProbePlan::Udp { .. }));
    assert!(matches!(
        plan_for("udp://9.9.9.9:5300"),
        ProbePlan::Udp { .. }
    ));
    assert!(matches!(
        plan_for("https://dns.google/dns-query"),
        ProbePlan::Doh { .. }
    ));
    assert!(matches!(
        plan_for("doh://dns.pub/dns-query"),
        ProbePlan::Doh { .. }
    ));
    assert!(matches!(plan_for("system"), ProbePlan::System));
    assert!(matches!(
        plan_for("tls://1.0.0.1:853"),
        ProbePlan::Undrivable { .. }
    ));
    assert!(matches!(plan_for(""), ProbePlan::Undrivable { .. }));
}
