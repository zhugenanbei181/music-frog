//! Behavior cases for full.
//! test-intent: behavior

use super::*;

#[test]
fn test_full_profile_roundtrip_all_protocols() {
    let nodes = parse_profile_yaml(PROFILE_YAML).expect("parse profile");
    assert_eq!(nodes.len(), 7);
    let type_names: Vec<&str> = nodes.iter().map(ProxyNode::type_name).collect();
    assert_eq!(
        type_names,
        [
            "vless",
            "hysteria2",
            "tuic",
            "wireguard",
            "ss",
            "anytls",
            "quantum-tunnel-v9"
        ]
    );
    // Exactly 6 nodes are strongly typed; the 7th falls back to Other.
    assert_eq!(nodes.iter().filter(|n| n.is_typed()).count(), 6);

    // typed -> YAML -> typed stability for the whole list.
    assert_roundtrip_fixed_point(&nodes);
    assert_proxies_semantic_equivalence(PROFILE_YAML);

    // Writing nodes back into the original profile must not touch the other sections.
    let updated = replace_proxies_in_profile(PROFILE_YAML, &nodes).expect("write back");
    let doc: Value = serde_yaml_ng::from_str(&updated).expect("updated doc");
    assert_eq!(doc.get("mixed-port").and_then(Value::as_i64), Some(7890));
    let dns = doc.get("dns").expect("dns section kept");
    assert_eq!(
        dns.get("fake-ip-range"),
        Some(&Value::String("198.18.0.1/16".to_string()))
    );
    let rules = doc
        .get("rules")
        .and_then(Value::as_sequence)
        .expect("rules section kept");
    assert_eq!(rules.len(), 2);
    assert_eq!(doc.get("proxies"), Some(&proxies_value(PROFILE_YAML)));

    // A minimal profile built from nodes parses back to the same nodes.
    let minimal = nodes_to_profile_yaml(&nodes).expect("serialize");
    assert!(parse_profile_yaml(&minimal).expect("reparse minimal") == nodes);
}
