//! Behavior cases for validate.
//! test-intent: behavior

use super::*;

#[test]
fn test_validate_flags_missing_fields() {
    let text = r#"
proxies:
  - name: vless-broken
    type: vless
    server: 10.4.4.4
    port: 443
  - name: hy2-broken
    type: hysteria2
    server: 10.2.2.2
    port: 443
    obfs: salamander
  - name: tuic-broken
    type: tuic
    server: 10.1.1.1
    port: 443
    congestion-controller: reno
    udp-relay-mode: tcp
  - name: wg-broken
    type: wireguard
    server: 10.3.3.3
    port: 51820
    private-key: k
    public-key: k2
    ip: not-an-ip
  - name: mystery
    type: quantum-tunnel
    port: 7000
"#;
    let nodes = parse_profile_yaml(text).expect("parse");
    assert_eq!(nodes.len(), 5);

    let issues = validate(&nodes[0]);
    assert!(
        issues.iter().any(|m| m.contains("uuid")),
        "vless without uuid must be reported: {issues:?}"
    );

    let issues = validate(&nodes[1]);
    assert!(
        issues.iter().any(|m| m.contains("password"))
            && issues.iter().any(|m| m.contains("obfs-password")),
        "hysteria2 salamander without obfs-password must be reported: {issues:?}"
    );

    let issues = validate(&nodes[2]);
    assert!(
        issues.iter().any(|m| m.contains("uuid"))
            && issues.iter().any(|m| m.contains("password"))
            && issues.iter().any(|m| m.contains("congestion-controller"))
            && issues.iter().any(|m| m.contains("udp-relay-mode")),
        "tuic problems must be reported: {issues:?}"
    );

    let issues = validate(&nodes[3]);
    assert!(
        issues.iter().any(|m| m.contains("not a valid IP")),
        "wireguard address problems must be reported: {issues:?}"
    );

    let issues = validate(&nodes[4]);
    assert!(
        issues.iter().any(|m| m.contains("server")),
        "untyped node without server must be reported: {issues:?}"
    );
}

/// DUAL-08-09: the flat aggregation precheck reports the same
/// required-field problems for the protocols that degrade to `OtherNode`.
#[test]
fn validate_item_reports_missing_credentials_and_port() {
    use crate::profile_converter::ProxyNodeItem;
    use crate::proxy_nodes::validate::validate_item;

    let mut ss = ProxyNodeItem::new("HK 01", "ss", "1.1.1.1", 443);
    ss.password = Some("pass".to_owned());
    let issues = validate_item(&ss);
    assert!(
        issues.iter().any(|m| m.contains("cipher")),
        "ss without cipher must be reported: {issues:?}"
    );

    let mut vmess = ProxyNodeItem::new("VM 01", "vmess", "2.2.2.2", 443);
    vmess.uuid = Some("uuid".to_owned());
    assert!(
        validate_item(&vmess).is_empty(),
        "a complete vmess node passes the precheck"
    );

    let mut broken = ProxyNodeItem::new("Broken", "trojan", "", 0);
    broken.password = None;
    let issues = validate_item(&broken);
    assert!(issues.iter().any(|m| m.contains("server")));
    assert!(issues.iter().any(|m| m.contains("port")));
    assert!(issues.iter().any(|m| m.contains("password")));

    // Unknown protocols never get guessed credentials.
    let custom = ProxyNodeItem::new("Custom", "custom-protocol", "3.3.3.3", 8443);
    assert!(validate_item(&custom).is_empty());
}
