//! Behavior cases for anytls.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_anytls_roundtrip() {
    let node = parse_single(ANYTLS_YAML);
    let ProxyNode::Anytls(anytls) = &node else {
        panic!("anytls node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "anytls");
    assert_eq!(anytls.password.as_deref(), Some("anytls-secret-token"));
    assert_eq!(anytls.padding_range.as_deref(), Some("100-1000"));
    assert_eq!(anytls.idle_timeout, Some(60));
    assert_eq!(anytls.client_fingerprint.as_deref(), Some("chrome"));
    assert_eq!(anytls.sni.as_deref(), Some("anytls.example.com"));

    assert_proxies_semantic_equivalence(ANYTLS_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}
