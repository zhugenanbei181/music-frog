//! Behavior cases for shadowsocks.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_shadowsocks_2022_roundtrip() {
    let node = parse_single(SHADOWSOCKS_2022_YAML);
    let ProxyNode::Shadowsocks(ss) = &node else {
        panic!("shadowsocks node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "ss");
    assert_eq!(ss.cipher.as_deref(), Some("2022-blake3-aes-128-gcm"));
    assert_eq!(ss.password.as_deref(), Some("eCtXsJZ27+4PbhDkHnB92w=="));
    assert_eq!(ss.udp_over_tcp, Some(true));
    assert_eq!(ss.uot_version, Some(2));
    assert_eq!(ss.plugin.as_deref(), Some("v2ray-plugin"));

    assert_proxies_semantic_equivalence(SHADOWSOCKS_2022_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}
