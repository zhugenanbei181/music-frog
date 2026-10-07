//! Behavior cases for future.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_future_unknown_type_falls_back_to_other() {
    let text = r#"
proxies:
  - name: future-node
    type: quantum-tunnel-v9
    server: 203.0.113.99
    port: 9000
    secret-handshake: open-sesame
"#;
    let node = parse_single(text);
    let ProxyNode::Other(other) = &node else {
        panic!("future protocol must degrade to Other, got {node:?}");
    };
    assert_eq!(other.type_name, "quantum-tunnel-v9");
    assert_eq!(
        other.fields.get("secret-handshake"),
        Some(&Value::String("open-sesame".to_string()))
    );
    assert_proxies_semantic_equivalence(text);
    assert_roundtrip_fixed_point(from_ref(&node));
}
