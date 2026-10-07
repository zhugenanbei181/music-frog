//! Behavior cases for malformed.
//! test-intent: behavior

use super::*;

#[test]
fn test_malformed_values_degrade_to_other_losslessly() {
    let text = r#"
proxies:
  - name: wg-bad-reserved
    type: wireguard
    server: 10.0.0.1
    port: 51820
    private-key: k
    public-key: k2
    reserved: [300, 2, 3]
  - name: hy2-bad-port
    type: hysteria2
    server: 10.0.0.2
    port: not-a-port
    password: p
"#;
    let nodes = parse_profile_yaml(text).expect("parse");
    assert_eq!(nodes.len(), 2);

    // reserved [300, ...] does not fit u8 -> the typed variant fails and
    // the node degrades to Other instead of dropping data.
    let ProxyNode::Other(wg) = &nodes[0] else {
        panic!("expected lossless Other fallback, got {:?}", nodes[0]);
    };
    assert_eq!(wg.type_name, "wireguard");
    assert_eq!(
        wg.fields.get("reserved"),
        Some(&serde_yaml_ng::from_str::<Value>("[300, 2, 3]").expect("reserved value"))
    );
    assert!(wg.fields.contains_key("private-key"));

    let ProxyNode::Other(hy2) = &nodes[1] else {
        panic!("expected lossless Other fallback, got {:?}", nodes[1]);
    };
    assert_eq!(hy2.type_name, "hysteria2");
    assert_eq!(
        hy2.fields.get("port"),
        Some(&Value::String("not-a-port".to_string()))
    );

    assert_proxies_semantic_equivalence(text);
    assert_roundtrip_fixed_point(&nodes);
}
