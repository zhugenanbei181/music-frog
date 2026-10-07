//! Behavior cases for tuic.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_tuic_full_roundtrip() {
    let node = parse_single(TUIC_YAML);
    let ProxyNode::Tuic(tuic) = &node else {
        panic!("tuic node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "tuic");
    assert_eq!(
        tuic.uuid.as_deref(),
        Some("5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22")
    );
    assert_eq!(tuic.password.as_deref(), Some("tuic-pass"));
    assert_eq!(tuic.congestion_controller.as_deref(), Some("bbr"));
    assert_eq!(tuic.udp_relay_mode.as_deref(), Some("native"));
    assert_eq!(tuic.alpn, Some(vec!["h3".to_string()]));
    assert_eq!(tuic.reduce_rtt, Some(true));
    assert_eq!(tuic.heartbeat_interval, Some(10000));
    assert_eq!(tuic.request_timeout, Some(8000));
    assert_eq!(
        tuic.extra.get("fake-field"),
        Some(&Value::Number(123.into()))
    );

    assert_proxies_semantic_equivalence(TUIC_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}
