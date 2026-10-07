//! Behavior cases for wireguard.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_wireguard_full_roundtrip() {
    let node = parse_single(WIREGUARD_YAML);
    let ProxyNode::WireGuard(wg) = &node else {
        panic!("wireguard node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "wireguard");
    assert_eq!(
        wg.private_key.as_deref(),
        Some("eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=")
    );
    assert_eq!(
        wg.public_key.as_deref(),
        Some("Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=")
    );
    assert_eq!(
        wg.pre_shared_key.as_deref(),
        Some("31aIhAPwktDGpH4JDhA8GNvjFXEf/a6+UaQRyOAiyfM=")
    );
    // List form must be modeled as Reserved::Array and preserved.
    assert_eq!(wg.reserved, Some(Reserved::Array(vec![1, 2, 3])));
    assert_eq!(wg.mtu, Some(1420));
    assert_eq!(wg.ip.as_deref(), Some("172.16.0.2"));
    assert_eq!(
        wg.ipv6.as_deref(),
        Some("fd01:5ca1:ab1e:80fa:ab85:6eea:213f:f4a5")
    );
    assert_eq!(wg.remote_dns_resolve, Some(true));
    assert_eq!(
        wg.dns,
        Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
    );

    let awg = wg.amnezia_opts.as_ref().expect("amnezia-opts");
    assert_eq!(awg.jc, Some(5));
    assert_eq!(awg.jmin, Some(40));
    assert_eq!(awg.jmax, Some(70));
    assert_eq!(awg.s1, Some(15));
    assert_eq!(awg.s2, Some(20));
    assert_eq!(awg.h1, Some(123456));

    assert_eq!(wg.extra.get("fake-field"), Some(&Value::Number(123.into())));

    assert_proxies_semantic_equivalence(WIREGUARD_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}

#[test]
fn test_wireguard_reserved_base64_roundtrip() {
    let node = parse_single(WIREGUARD_RESERVED_BASE64_YAML);
    let ProxyNode::WireGuard(wg) = &node else {
        panic!("wireguard node degraded to Other: {node:?}");
    };
    // Base64 string form must stay a string (保形), not be decoded.
    assert_eq!(wg.reserved, Some(Reserved::Base64("AQID".to_string())));

    assert_proxies_semantic_equivalence(WIREGUARD_RESERVED_BASE64_YAML);
    let yaml = nodes_to_profile_yaml(from_ref(&node)).expect("serialize");
    assert!(
        yaml.contains("reserved: AQID"),
        "base64 reserved shape must survive serialization: {yaml}"
    );
    assert!(yaml.contains("fake-field: keep-me"));
    assert_roundtrip_fixed_point(from_ref(&node));
}
