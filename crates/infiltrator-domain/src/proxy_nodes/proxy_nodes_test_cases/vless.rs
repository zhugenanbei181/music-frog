//! Behavior cases for vless.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_vless_full_roundtrip() {
    let node = parse_single(VLESS_YAML);
    let ProxyNode::Vless(vless) = &node else {
        panic!("vless node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "vless");
    assert_eq!(vless.common.name, "vless-reality-vision");
    assert_eq!(vless.common.server, "203.0.113.10");
    assert_eq!(vless.common.port, 443);
    assert_eq!(vless.common.udp, Some(true));
    assert_eq!(vless.common.tls, Some(true));
    assert_eq!(vless.common.skip_cert_verify, Some(false));
    assert_eq!(vless.flow.as_deref(), Some("xtls-rprx-vision"));
    assert_eq!(vless.client_fingerprint.as_deref(), Some("chrome"));
    assert_eq!(vless.network.as_deref(), Some("tcp"));
    assert_eq!(vless.packet_encoding.as_deref(), Some("packetaddr"));

    let reality = vless.reality_opts.as_ref().expect("reality-opts");
    assert_eq!(
        reality.public_key.as_deref(),
        Some("SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc")
    );
    assert_eq!(reality.short_id.as_deref(), Some("6ba85179e30d4fc2"));
    assert_eq!(reality.spider_x.as_deref(), Some("/spx"));

    let grpc = vless.grpc_opts.as_ref().expect("grpc-opts");
    assert_eq!(
        grpc.get("grpc-service-name"),
        Some(&Value::String("grpc-svc".to_string()))
    );
    let ws = vless.ws_opts.as_ref().expect("ws-opts");
    assert_eq!(ws.get("path"), Some(&Value::String("/ws".to_string())));

    let xhttp = vless.xhttp_opts.as_ref().expect("xhttp-opts");
    assert_eq!(xhttp.mode.as_deref(), Some("stream-up"));
    assert_eq!(xhttp.path.as_deref(), Some("/xhttp-path"));

    // Unknown keys must be captured by the flatten extra map.
    assert_eq!(
        vless.uuid.as_deref(),
        Some("b831381d-6324-4d53-ad4f-8cda48b30811")
    );
    assert_eq!(vless.servername.as_deref(), Some("www.microsoft.com"));
    assert!(
        matches!(vless.extra.get("fake-field"), Some(Value::Number(_))),
        "fake-field kept in extra"
    );

    assert_proxies_semantic_equivalence(VLESS_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));
    assert!(validate(&node).is_empty());
}
