use super::*;
use std::collections::HashMap;

#[test]
fn test_pcap_header_default() {
    let header = PcapHeader::default();
    assert_eq!(header.magic, PcapHeader::MAGIC_MICROS);
    assert_eq!(header.version_major, 2);
    assert_eq!(header.version_minor, 4);
    assert_eq!(header.thiszone, 0);
    assert_eq!(header.sigfigs, 0);
    assert_eq!(header.snaplen, 65535);
    assert_eq!(header.link_type, PcapHeader::LINKTYPE_ETHERNET);
    assert!(!header.is_nanosecond_precision());
}

#[test]
fn test_pcap_header_nanoseconds() {
    let header = PcapHeader::new(1500, PcapHeader::LINKTYPE_RAW).with_nanoseconds();
    assert_eq!(header.magic, PcapHeader::MAGIC_NANOS);
    assert_eq!(header.snaplen, 1500);
    assert_eq!(header.link_type, PcapHeader::LINKTYPE_RAW);
    assert!(header.is_nanosecond_precision());
}

#[test]
fn test_pcap_header_to_from_bytes_roundtrip() {
    let original = PcapHeader::new(9000, PcapHeader::LINKTYPE_IPV4);
    let bytes = original.to_bytes();
    assert_eq!(bytes.len(), 24);

    let parsed = PcapHeader::from_bytes(&bytes).expect("parse pcap header");
    assert_eq!(original, parsed);
}

#[test]
fn test_pcap_header_from_bytes_invalid() {
    let short_buf = [0u8; 20];
    assert!(PcapHeader::from_bytes(&short_buf).is_err());

    let mut bad_magic = PcapHeader::default().to_bytes();
    bad_magic[0..4].copy_from_slice(&0xdeadbeef_u32.to_le_bytes());
    assert!(PcapHeader::from_bytes(&bad_magic).is_err());
}

#[test]
fn test_pcap_record_header_to_from_bytes_roundtrip() {
    let original = PcapRecordHeader::new(1725148800, 123456, 1500, 1514);
    let bytes = original.to_bytes();
    assert_eq!(bytes.len(), 16);

    let parsed = PcapRecordHeader::from_bytes(&bytes).expect("parse record header");
    assert_eq!(original, parsed);
}

#[test]
fn test_pcap_record_header_from_bytes_short() {
    let short_buf = [0u8; 12];
    assert!(PcapRecordHeader::from_bytes(&short_buf).is_err());
}

#[test]
fn test_pcap_exporter_write_helpers() {
    let hdr_bytes = write_header();
    assert_eq!(hdr_bytes.len(), 24);
    let parsed_hdr = PcapHeader::from_bytes(&hdr_bytes).unwrap();
    assert_eq!(parsed_hdr.magic, PcapHeader::MAGIC_MICROS);

    let payload = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let pkt_bytes = write_packet(1700000000, 500, payload);
    assert_eq!(pkt_bytes.len(), 16 + payload.len());

    let rec_hdr = PcapRecordHeader::from_bytes(&pkt_bytes[..16]).unwrap();
    assert_eq!(rec_hdr.ts_sec, 1700000000);
    assert_eq!(rec_hdr.ts_subsec, 500);
    assert_eq!(rec_hdr.caplen as usize, payload.len());
    assert_eq!(&pkt_bytes[16..], payload);
}

#[test]
fn test_pcap_exporter_append_and_parse_roundtrip() {
    let mut exporter = PcapExporter::default();

    let pkt1 = b"Ethernet Frame 1: DNS Query";
    let pkt2 = b"Ethernet Frame 2: DNS Response with Answers";
    let pkt3 = b"Ethernet Frame 3: HTTP Request";

    let written1 = exporter.append_packet(1001, 10, pkt1);
    assert_eq!(written1, 16 + pkt1.len());
    exporter.append_packet(1001, 25, pkt2);
    exporter.append_packet(1002, 5, pkt3);

    assert_eq!(exporter.packet_count(), 3);
    assert_eq!(
        exporter.total_bytes_captured(),
        pkt1.len() + pkt2.len() + pkt3.len()
    );

    let pcap_data = exporter.as_bytes();
    let (header, packets) = PcapExporter::parse_packets(pcap_data).expect("parse pcap packets");

    assert_eq!(header.magic, PcapHeader::MAGIC_MICROS);
    assert_eq!(packets.len(), 3);

    assert_eq!(packets[0].header.ts_sec, 1001);
    assert_eq!(packets[0].header.ts_subsec, 10);
    assert_eq!(packets[0].header.caplen as usize, pkt1.len());
    assert_eq!(packets[0].header.orig_len as usize, pkt1.len());
    assert_eq!(packets[0].data, pkt1);

    assert_eq!(packets[1].header.ts_sec, 1001);
    assert_eq!(packets[1].header.ts_subsec, 25);
    assert_eq!(packets[1].data, pkt2);

    assert_eq!(packets[2].header.ts_sec, 1002);
    assert_eq!(packets[2].header.ts_subsec, 5);
    assert_eq!(packets[2].data, pkt3);
}

#[test]
fn test_pcap_exporter_nanoseconds_roundtrip() {
    let header = PcapHeader::new(65535, PcapHeader::LINKTYPE_ETHERNET).with_nanoseconds();
    let mut exporter = PcapExporter::new(header);

    let pkt = b"Nanos timestamp frame";
    exporter.append_packet(1725000000, 999_888_777, pkt);

    let raw = exporter.into_bytes();
    let (parsed_hdr, packets) = PcapExporter::parse_packets(&raw).expect("parse nanos pcap");
    assert!(parsed_hdr.is_nanosecond_precision());
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].header.ts_sec, 1725000000);
    assert_eq!(packets[0].header.ts_subsec, 999_888_777);
    assert_eq!(packets[0].data, pkt);
}

#[test]
fn test_pcap_exporter_snaplen_clipping() {
    let header = PcapHeader::new(20, PcapHeader::LINKTYPE_ETHERNET);
    let mut exporter = PcapExporter::new(header);

    let large_packet = vec![0xaa; 100];
    exporter.append_packet(2000, 100, &large_packet);

    assert_eq!(exporter.packet_count(), 1);
    assert_eq!(exporter.total_bytes_captured(), 20);

    let raw_bytes = exporter.into_bytes();
    let (_, packets) = PcapExporter::parse_packets(&raw_bytes).unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].header.caplen, 20);
    assert_eq!(packets[0].header.orig_len, 100);
    assert_eq!(packets[0].data.len(), 20);
    assert_eq!(packets[0].data, vec![0xaa; 20]);
}

#[test]
fn test_pcap_exporter_clear_and_parse_errors() {
    let mut exporter = PcapExporter::default();
    exporter.append_packet(1, 2, b"test payload");
    assert_eq!(exporter.packet_count(), 1);

    exporter.clear();
    assert_eq!(exporter.packet_count(), 0);
    assert_eq!(exporter.total_bytes_captured(), 0);
    assert_eq!(exporter.as_bytes().len(), 24);

    let truncated_data = vec![0u8; 30]; // Valid header length but incomplete 16-byte record
    assert!(PcapExporter::parse_packets(&truncated_data).is_err());

    let too_short = vec![0u8; 10];
    assert!(PcapExporter::parse_packets(&too_short).is_err());
}

#[test]
fn test_rewrite_rule_direct() {
    let rule = RewriteRule::direct(
        r"^http://api\.example\.com/(v1|v2)/(.*)$",
        "https://internal-gw.local/services/$1/$2",
    )
    .unwrap();

    let outcome = rule
        .apply("http://api.example.com/v1/users/profile")
        .expect("should match");
    assert_eq!(
        outcome,
        RewriteOutcome::DirectRewrite("https://internal-gw.local/services/v1/users/profile".into())
    );
    assert_eq!(
        outcome.destination(),
        "https://internal-gw.local/services/v1/users/profile"
    );
    assert!(!outcome.is_redirect());
    assert_eq!(outcome.status_code(), None);

    assert_eq!(rule.apply("http://other.com/v1/users"), None);
}

#[test]
fn test_rewrite_rule_redirect_302_and_307() {
    let rule_302 = RewriteRule::redirect_302(
        r"^http://legacy\.site\.com/(.*)$",
        "https://new.site.com/$1",
    )
    .unwrap();
    let out_302 = rule_302.apply("http://legacy.site.com/docs/intro").unwrap();
    assert_eq!(
        out_302,
        RewriteOutcome::Redirect302("https://new.site.com/docs/intro".into())
    );
    assert!(out_302.is_redirect());
    assert_eq!(out_302.status_code(), Some(302));

    let rule_307 = RewriteRule::redirect_307(
        r"^http://auth\.site\.com/login\?token=(.*)$",
        "https://auth.site.com/sso/login?token=$1",
    )
    .unwrap();
    let out_307 = rule_307
        .apply("http://auth.site.com/login?token=abc123xyz")
        .unwrap();
    assert_eq!(
        out_307,
        RewriteOutcome::Redirect307("https://auth.site.com/sso/login?token=abc123xyz".into())
    );
    assert!(out_307.is_redirect());
    assert_eq!(out_307.status_code(), Some(307));
}

#[test]
fn test_url_rewrite_engine_rules_and_priority() {
    let mut engine = UrlRewriteEngine::new();
    assert!(engine.is_empty());

    engine
        .add_redirect_307(
            r"^https://login\.corp\.com/(.*)$",
            "https://sso.corp.com/$1",
        )
        .unwrap();
    engine
        .add_redirect_302(r"^http://(.*)$", "https://$1")
        .unwrap();
    engine
        .add_direct(
            r"^https://cdn\.corp\.com/assets/(.*)$",
            "https://s3.corp.com/cached-assets/$1",
        )
        .unwrap();

    assert_eq!(engine.len(), 3);
    assert!(!engine.is_empty());
    assert_eq!(engine.rules().len(), 3);

    // Matches first rule (307)
    let res1 = engine.apply_rewrite("https://login.corp.com/oauth/authorize");
    assert_eq!(
        res1,
        Some(RewriteOutcome::Redirect307(
            "https://sso.corp.com/oauth/authorize".into()
        ))
    );

    // Matches second rule (302)
    let res2 = engine.apply_rewrite("http://insecure.corp.com/dashboard");
    assert_eq!(
        res2,
        Some(RewriteOutcome::Redirect302(
            "https://insecure.corp.com/dashboard".into()
        ))
    );

    // Matches third rule (direct)
    let res3 = engine.apply_rewrite("https://cdn.corp.com/assets/logo.png");
    assert_eq!(
        res3,
        Some(RewriteOutcome::DirectRewrite(
            "https://s3.corp.com/cached-assets/logo.png".into()
        ))
    );

    // No match
    let res4 = engine.apply_rewrite("https://other.domain.org/test");
    assert_eq!(res4, None);

    engine.clear();
    assert!(engine.is_empty());
}

#[test]
fn test_url_rewrite_engine_with_rules_constructor() {
    let r1 = RewriteRule::redirect_302(r"^http://a\.com$", "https://a.com").unwrap();
    let r2 = RewriteRule::direct(r"^http://b\.com$", "https://b.com").unwrap();
    let engine = UrlRewriteEngine::with_rules(vec![r1, r2]);

    assert_eq!(engine.len(), 2);
    assert_eq!(
        engine.apply_rewrite("http://a.com"),
        Some(RewriteOutcome::Redirect302("https://a.com".into()))
    );
}

#[test]
fn test_header_modifier_set_user_agent_and_referer() {
    let mut modifier = HeaderModifier::new();
    modifier.set_user_agent("CustomBot/2.0 (Proxy)");
    modifier.set_referer("https://origin.trusted.com");

    let mut headers = HashMap::new();
    headers.insert("user-agent".to_string(), "Mozilla/5.0".to_string());
    headers.insert("Accept".to_string(), "*/*".to_string());

    modifier.modify_request_headers(&mut headers, None);

    assert_eq!(
        headers.get("User-Agent"),
        Some(&"CustomBot/2.0 (Proxy)".to_string())
    );
    assert_eq!(headers.get("user-agent"), None); // Case-insensitive replace cleaned old lowercase key
    assert_eq!(
        headers.get("Referer"),
        Some(&"https://origin.trusted.com".to_string())
    );
    assert_eq!(headers.get("Accept"), Some(&"*/*".to_string()));
}

#[test]
fn test_header_modifier_inject_cors_and_custom() {
    let mut modifier = HeaderModifier::new();
    modifier.inject_cors_origin("*");
    modifier.set_custom_header(HeaderTarget::Response, "X-Proxy-Latency-Ms", "42");
    modifier.remove_header(HeaderTarget::Response, "Server");

    let mut headers = HashMap::new();
    headers.insert("server".to_string(), "nginx/1.18.0".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    modifier.modify_response_headers(&mut headers, None);

    assert_eq!(
        headers.get("Access-Control-Allow-Origin"),
        Some(&"*".to_string())
    );
    assert_eq!(headers.get("X-Proxy-Latency-Ms"), Some(&"42".to_string()));
    assert_eq!(headers.get("server"), None);
    assert_eq!(headers.get("Server"), None);
}

#[test]
fn test_header_modifier_conditional_url_filter() {
    let mut modifier = HeaderModifier::new();
    let rule = HeaderRule::with_url_filter(
        HeaderTarget::Request,
        HeaderAction::Set {
            name: "X-Authorized-Agent".to_string(),
            value: "True".to_string(),
        },
        r"^https://secure\.internal\.net/.*$",
    )
    .unwrap();
    modifier.add_rule(rule);

    let mut headers_matching = HashMap::new();
    modifier.modify_request_headers(
        &mut headers_matching,
        Some("https://secure.internal.net/api/v1"),
    );
    assert_eq!(
        headers_matching.get("X-Authorized-Agent"),
        Some(&"True".to_string())
    );

    let mut headers_non_matching = HashMap::new();
    modifier.modify_request_headers(
        &mut headers_non_matching,
        Some("https://public.example.com/api"),
    );
    assert_eq!(headers_non_matching.get("X-Authorized-Agent"), None);
}

#[test]
fn test_header_modifier_inject_if_not_present_and_replace_if_present() {
    let mut modifier = HeaderModifier::new();
    modifier.add_rule(HeaderRule::inject(
        HeaderTarget::Request,
        "X-Request-Id",
        "uuid-12345",
    ));
    modifier.add_rule(HeaderRule::new(
        HeaderTarget::Request,
        HeaderAction::ReplaceIfPresent {
            name: "X-Trace-Id".to_string(),
            value: "trace-99999".to_string(),
        },
    ));

    let mut headers = HashMap::new();
    headers.insert("X-Request-Id".to_string(), "existing-req-id".to_string());

    modifier.modify_request_headers(&mut headers, None);

    // X-Request-Id already exists, so inject does not overwrite
    assert_eq!(
        headers.get("X-Request-Id"),
        Some(&"existing-req-id".to_string())
    );
    // X-Trace-Id was not present, so replace_if_present did nothing
    assert_eq!(headers.get("X-Trace-Id"), None);

    headers.insert("x-trace-id".to_string(), "old-trace".to_string());
    modifier.modify_request_headers(&mut headers, None);
    assert_eq!(headers.get("X-Trace-Id"), Some(&"trace-99999".to_string()));
    assert_eq!(headers.get("x-trace-id"), None);
}

#[test]
fn test_header_modifier_target_separation() {
    let mut modifier = HeaderModifier::new();
    modifier.add_rule(HeaderRule::set(
        HeaderTarget::Request,
        "X-Client-Type",
        "Web",
    ));
    modifier.add_rule(HeaderRule::set(
        HeaderTarget::Response,
        "X-Server-Type",
        "Edge",
    ));
    modifier.add_rule(HeaderRule::set(
        HeaderTarget::Both,
        "X-Shared-Tag",
        "Global",
    ));

    let mut req_headers = HashMap::new();
    modifier.modify_request_headers(&mut req_headers, None);
    assert_eq!(req_headers.get("X-Client-Type"), Some(&"Web".to_string()));
    assert_eq!(req_headers.get("X-Server-Type"), None);
    assert_eq!(req_headers.get("X-Shared-Tag"), Some(&"Global".to_string()));

    let mut resp_headers = HashMap::new();
    modifier.modify_response_headers(&mut resp_headers, None);
    assert_eq!(resp_headers.get("X-Client-Type"), None);
    assert_eq!(resp_headers.get("X-Server-Type"), Some(&"Edge".to_string()));
    assert_eq!(
        resp_headers.get("X-Shared-Tag"),
        Some(&"Global".to_string())
    );
}

#[test]
fn test_mock_response_json_and_text() {
    let json_val = serde_json::json!({
        "status": "success",
        "data": { "user_id": 42 }
    });
    let mock_json = MockResponse::ok_json(&json_val).unwrap();
    assert_eq!(mock_json.status_code, 200);
    assert_eq!(
        mock_json.headers.get("Content-Type"),
        Some(&"application/json; charset=utf-8".to_string())
    );

    let parsed: serde_json::Value = serde_json::from_slice(&mock_json.body).unwrap();
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["data"]["user_id"], 42);

    let mock_text = MockResponse::ok_text("pong");
    assert_eq!(mock_text.status_code, 200);
    assert_eq!(mock_text.body, b"pong");

    let mock_404 = MockResponse::not_found();
    assert_eq!(mock_404.status_code, 404);
}

#[test]
fn test_mock_response_status_reasons() {
    let check = |code: u16, expected: &str| {
        let resp = MockResponse::new(code, HashMap::new(), Vec::new());
        assert_eq!(resp.status_reason(), expected);
    };
    check(200, "OK");
    check(201, "Created");
    check(204, "No Content");
    check(301, "Moved Permanently");
    check(302, "Found");
    check(304, "Not Modified");
    check(307, "Temporary Redirect");
    check(400, "Bad Request");
    check(401, "Unauthorized");
    check(403, "Forbidden");
    check(404, "Not Found");
    check(405, "Method Not Allowed");
    check(500, "Internal Server Error");
    check(502, "Bad Gateway");
    check(503, "Service Unavailable");
    check(504, "Gateway Timeout");
    check(999, "Status");
}

#[test]
fn test_mock_response_to_http_11_wire_bytes() {
    let mut headers = HashMap::new();
    headers.insert("Server".to_string(), "MockEngine/1.0".to_string());
    let resp = MockResponse::new(200, headers, b"Hello World".to_vec());

    let wire = resp.to_http_11_bytes();
    let wire_str = String::from_utf8(wire).expect("valid utf8");

    assert!(wire_str.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(wire_str.contains("Content-Length: 11\r\n"));
    assert!(wire_str.contains("Server: MockEngine/1.0\r\n"));
    assert!(wire_str.ends_with("\r\n\r\nHello World"));
}

#[test]
fn test_mock_response_engine_match_and_respond() {
    let mut engine = MockResponseEngine::new();
    assert!(engine.is_empty());

    engine
        .add_json_mock(
            r"^https://api\.mock\.com/v1/health$",
            200,
            &serde_json::json!({"status": "UP"}),
        )
        .unwrap();

    engine
        .add_text_mock(r"^https://api\.mock\.com/v1/ping$", 200, "pong")
        .unwrap();

    let rule_custom = MockResponseRule::new(
        r"^https://api\.mock\.com/v1/error$",
        500,
        HashMap::from([("X-Error-Code".to_string(), "E_INTERNAL".to_string())]),
        b"Internal Failure".to_vec(),
    )
    .unwrap();
    engine.add_rule(rule_custom);

    assert_eq!(engine.len(), 3);
    assert!(!engine.is_empty());

    // Match health
    let resp_health = engine
        .match_and_respond("https://api.mock.com/v1/health")
        .expect("match health");
    assert_eq!(resp_health.status_code, 200);
    assert!(String::from_utf8_lossy(&resp_health.body).contains("UP"));

    // Match ping
    let resp_ping = engine
        .match_and_respond("https://api.mock.com/v1/ping")
        .expect("match ping");
    assert_eq!(resp_ping.status_code, 200);
    assert_eq!(resp_ping.body, b"pong");

    // Match custom error
    let resp_err = engine
        .match_and_respond("https://api.mock.com/v1/error")
        .expect("match error");
    assert_eq!(resp_err.status_code, 500);
    assert_eq!(
        resp_err.headers.get("X-Error-Code"),
        Some(&"E_INTERNAL".to_string())
    );

    // No match
    assert!(engine
        .match_and_respond("https://api.mock.com/v1/unknown")
        .is_none());

    engine.clear();
    assert!(engine.is_empty());
}
