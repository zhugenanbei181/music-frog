//! Behavior cases for trojan.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_trojan_and_vmess_roundtrip() {
    let trojan_node = parse_single(TROJAN_YAML);
    let ProxyNode::Trojan(trojan) = &trojan_node else {
        panic!("trojan node degraded to Other: {trojan_node:?}");
    };
    assert_eq!(trojan_node.type_name(), "trojan");
    assert_eq!(trojan.password.as_deref(), Some("trojan-secret-pass"));
    assert_eq!(trojan.network.as_deref(), Some("ws"));
    assert_proxies_semantic_equivalence(TROJAN_YAML);
    assert_roundtrip_fixed_point(from_ref(&trojan_node));
    assert!(validate(&trojan_node).is_empty());

    let vmess_node = parse_single(VMESS_YAML);
    let ProxyNode::Vmess(vmess) = &vmess_node else {
        panic!("vmess node degraded to Other: {vmess_node:?}");
    };
    assert_eq!(vmess_node.type_name(), "vmess");
    assert_eq!(
        vmess.uuid.as_deref(),
        Some("b831381d-6324-4d53-ad4f-8cda48b30811")
    );
    assert_eq!(vmess.alter_id, Some(0));
    assert_eq!(vmess.cipher.as_deref(), Some("auto"));
    assert_proxies_semantic_equivalence(VMESS_YAML);
    assert_roundtrip_fixed_point(from_ref(&vmess_node));
    assert!(validate(&vmess_node).is_empty());
}

#[test]
fn test_trojan_ss_opts_is_typed_and_validated() {
    let node = parse_single(TROJAN_SS_OPTS_YAML);
    let ProxyNode::Trojan(trojan) = &node else {
        panic!("trojan node degraded to Other: {node:?}");
    };
    let ss = trojan.ss_opts.as_ref().expect("ss-opts");
    assert_eq!(ss.enabled, Some(true));
    assert_eq!(ss.method.as_deref(), Some("aes-128-gcm"));
    assert_eq!(ss.password.as_deref(), Some("ss-pw"));
    assert!(validate(&node).is_empty(), "{:?}", validate(&node));

    assert_proxies_semantic_equivalence(TROJAN_SS_OPTS_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));

    // Options without `enabled: true` are ignored by the core and reported.
    let off = r#"
proxies:
  - name: trojan-ss-off
    type: trojan
    server: 203.0.113.94
    port: 443
    password: pw
    ss-opts:
      enabled: false
      method: aes-128-gcm
"#;
    let node = parse_single(off);
    let issues = validate(&node);
    assert!(
        issues.iter().any(|issue| issue.contains("ss-opts")),
        "{issues:?}"
    );
}
