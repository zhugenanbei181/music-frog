use super::*;

#[test]
fn test_parse_structured_connection_log() {
    let line = "INFO[0003] [TCP] 192.168.1.23:52118 --> www.google.com:443 match DomainSuffix(google.com) using 节点选择[香港 IEPL-01]";
    let parsed = parse_structured_log(line);
    assert_eq!(parsed.level, LogLevel::Info);
    assert_eq!(parsed.timestamp.as_deref(), Some("0003"));
    assert_eq!(parsed.protocol.as_deref(), Some("TCP"));
    assert_eq!(parsed.source.as_deref(), Some("192.168.1.23:52118"));
    assert_eq!(parsed.destination.as_deref(), Some("www.google.com:443"));
    assert_eq!(parsed.rule.as_deref(), Some("DomainSuffix(google.com)"));
    assert_eq!(parsed.outbound_group.as_deref(), Some("节点选择"));
    assert_eq!(parsed.outbound_node.as_deref(), Some("香港 IEPL-01"));
    assert_eq!(parsed.outbound_flag.as_deref(), Some("🇭🇰"));
    assert!(parsed.is_connection);
}

#[test]
fn test_parse_structured_udp_log() {
    let line = "INFO[0004] [UDP] 192.168.1.23:52119 --> 8.8.8.8:53 match Ip CIDR(8.8.8.8/32) using 全球直连[DIRECT]";
    let parsed = parse_structured_log(line);
    assert_eq!(parsed.level, LogLevel::Info);
    assert_eq!(parsed.timestamp.as_deref(), Some("0004"));
    assert_eq!(parsed.protocol.as_deref(), Some("UDP"));
    assert_eq!(parsed.destination.as_deref(), Some("8.8.8.8:53"));
    assert_eq!(parsed.rule.as_deref(), Some("Ip CIDR(8.8.8.8/32)"));
    assert_eq!(parsed.outbound_node.as_deref(), Some("DIRECT"));
    assert_eq!(parsed.outbound_flag.as_deref(), Some("⚡"));
    assert!(parsed.is_connection);
}

#[test]
fn test_parse_structured_system_error_log() {
    let line = "ERROR[0009] DNS 解析失败：resolver default: lookup doh.privatedns.example: server misbehaving";
    let parsed = parse_structured_log(line);
    assert_eq!(parsed.level, LogLevel::Error);
    assert_eq!(parsed.timestamp.as_deref(), Some("0009"));
    assert_eq!(parsed.protocol, None);
    assert!(!parsed.is_connection);
    assert!(parsed.message.contains("DNS 解析失败"));
}

#[test]
fn test_parse_json_formatted_log() {
    let line = r#"{"type":"warning","payload":"[TCP] connect error: timeout"}"#;
    let parsed = parse_structured_log(line);
    assert_eq!(parsed.level, LogLevel::Warn);
    assert_eq!(parsed.protocol.as_deref(), Some("TCP"));
    assert!(!parsed.is_connection);
}
