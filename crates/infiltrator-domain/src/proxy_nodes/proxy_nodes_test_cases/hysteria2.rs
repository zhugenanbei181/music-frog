//! Behavior cases for hysteria2.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_hysteria2_full_roundtrip() {
    let node = parse_single(HYSTERIA2_YAML);
    let ProxyNode::Hysteria2(hy2) = &node else {
        panic!("hysteria2 node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "hysteria2");
    assert_eq!(hy2.common.port, 36712);
    assert_eq!(hy2.ports.as_deref(), Some("20000-30000,8443,443"));
    assert_eq!(hy2.password.as_deref(), Some("hy2-pass"));
    assert_eq!(hy2.obfs.as_deref(), Some("salamander"));
    assert_eq!(hy2.obfs_password.as_deref(), Some("obfs-pass"));
    assert_eq!(hy2.hop_interval, Some(30));
    assert_eq!(hy2.up, Some(Bandwidth::Text("100 Mbps".to_string())));
    assert_eq!(hy2.down, Some(Bandwidth::U64(200)));
    assert_eq!(hy2.alpn, Some(vec!["h3".to_string()]));
    assert_eq!(hy2.cwnd, Some(1024));
    assert_eq!(hy2.recv_window_conn, Some(65536));
    assert_eq!(
        hy2.extra.get("fake-field"),
        Some(&Value::String("hello".to_string()))
    );

    assert_proxies_semantic_equivalence(HYSTERIA2_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}
