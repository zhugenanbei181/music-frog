//! test-intent: behavior
//! Captured from the locked kernel, using only a loopback controller and synthetic nodes.
use crate::client::MihomoClient;
use crate::runtime_proxy::ControllerProxy;
use crate::types::ProxiesResponse;
use infiltrator_domain::proxy::Proxy;
use mockito::Server;
use serde_json::{Value, from_str, to_value};

const CONTROLLER_FIXTURE: &str = include_str!("../../tests/fixtures/runtime-proxies-v1.19.18.json");

#[test]
fn real_controller_runtime_nodes_decode_without_configuration_secrets_or_server_fields() {
    let response: ProxiesResponse = from_str(CONTROLLER_FIXTURE)
        .expect("actual controller observations must decode independently of configuration fields");
    let node = &response.proxies["Inspection node"];
    assert_eq!(node.proxy_type(), "Shadowsocks");
    assert!(!node.is_group());
    assert!(node.alive());
    assert!(node.history().is_empty());
    let group = &response.proxies["Inspection group"];
    assert!(group.is_group());
    assert_eq!(group.now(), Some("Inspection node"));
    assert_eq!(
        group.all(),
        Some(["Inspection node".into(), "DIRECT".into()].as_slice())
    );
    let serialized = to_value(node).unwrap();
    assert_eq!(serialized["type"], "Shadowsocks");
    for key in ["server", "port", "cipher", "password", "uuid"] {
        assert!(
            serialized.get(key).is_none(),
            "unreported field {key} must remain absent"
        );
    }
}

#[tokio::test]
async fn actual_controller_list_and_single_node_endpoints_use_the_runtime_decoder() {
    let mut server = Server::new_async().await;
    let payload: Value = from_str(CONTROLLER_FIXTURE).unwrap();
    let node = &payload["proxies"]["Inspection node"];
    let list = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(CONTROLLER_FIXTURE)
        .create_async()
        .await;
    let single = server
        .mock("GET", "/proxies/Inspection%20node")
        .with_status(200)
        .with_body(node.to_string())
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let proxies = client.get_proxies().await.unwrap();
    let detail = client.get_proxy("Inspection node").await.unwrap();
    assert_eq!(proxies["Inspection node"], detail);
    assert_eq!(detail.name(), "Inspection node");
    assert_eq!(detail.health_observation(), Some(true));
    list.assert_async().await;
    single.assert_async().await;
}

#[test]
fn unreported_flags_remain_unknown_and_runtime_decode_does_not_relax_configuration() {
    let payload = r#"{"type":"Shadowsocks","name":"leaf","history":[{"time":"old","delay":42},{"time":"new","delay":0}]}"#;
    let ControllerProxy(proxy) = from_str(payload).unwrap();
    assert_eq!(proxy.health_observation(), None);
    assert_eq!(proxy.udp_observation(), None);
    assert_eq!(proxy.delay(), Some(0));
    assert_eq!(proxy.history().len(), 2);
    assert!(from_str::<Proxy>(payload).is_err());
    let serialized = to_value(proxy).unwrap();
    for key in ["server", "port", "cipher", "alive", "udp"] {
        assert!(serialized.get(key).is_none());
    }
}

#[test]
fn extra_reported_metadata_cannot_change_the_latest_latency_or_override_explicit_zero() {
    let sparse = r#"{"type":"Shadowsocks","name":"leaf","udp":true,"alive":false,"history":[{"time":"last","delay":85}]}"#;
    let ControllerProxy(sparse) = from_str(sparse).unwrap();
    let rich = r#"{"type":"Shadowsocks","name":"leaf","udp":true,"alive":false,"server":"node.example.test","port":443,"cipher":"aes-128-gcm","history":[{"time":"last","delay":85}]}"#;
    let ControllerProxy(rich) = from_str(rich).unwrap();
    assert_eq!(sparse.delay(), Some(85));
    assert_eq!(rich.delay(), Some(85));
    assert_eq!(rich.health_observation(), Some(false));
    let mut explicit_timeout = to_value(&rich).unwrap();
    explicit_timeout["delay"] = Value::from(0);
    let ControllerProxy(timeout) = serde_json::from_value(explicit_timeout).unwrap();
    assert_eq!(timeout.delay(), Some(0));
    assert_eq!(timeout.health_observation(), Some(false));
}

#[test]
fn malformed_reported_fields_and_mismatched_identity_are_rejected_without_fallback() {
    for payload in [
        r#"{"type":"Shadowsocks","name":"leaf","server":123}"#,
        r#"{"type":"Shadowsocks","name":"leaf","port":70000}"#,
        r#"{"type":"Shadowsocks","name":"leaf","alive":"true"}"#,
        r#"{"type":"Shadowsocks","name":"leaf","history":"missing"}"#,
        r#"{"type":"","name":"leaf"}"#,
    ] {
        assert!(
            from_str::<ControllerProxy>(payload).is_err(),
            "malformed source must be rejected: {payload}"
        );
    }
    assert!(
        from_str::<ProxiesResponse>(
            r#"{"proxies":{"first":{"type":"Shadowsocks","name":"other"}}}"#
        )
        .is_err()
    );
}
