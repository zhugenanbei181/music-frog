//! test-intent: behavior
//! Historical v1.19.18 controller response with synthetic nodes. This is a
//! decoder fixture, not evidence for the currently locked v1.19.32 binary.
use crate::api::client::MihomoClient;
use crate::api::proxy::ProxiesResponse;
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
    assert_eq!(node.health_observation(), Some(true));
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
async fn actual_controller_proxy_inventory_uses_the_runtime_decoder() {
    let mut server = Server::new_async().await;
    let list = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(CONTROLLER_FIXTURE)
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let proxies = client.get_proxies().await.unwrap();
    let observed = &proxies["Inspection node"];
    assert_eq!(observed.name(), "Inspection node");
    assert_eq!(observed.health_observation(), Some(true));
    list.assert_async().await;
}

#[test]
fn unreported_flags_remain_unknown() {
    let payload = r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","history":[{"time":"old","delay":42},{"time":"new","delay":0}]}}}"#;
    let proxies = from_str::<ProxiesResponse>(payload).unwrap().proxies;
    let leaf = &proxies["leaf"];
    assert_eq!(leaf.health_observation(), None);
    assert_eq!(leaf.udp_observation(), None);
    assert_eq!(leaf.delay(), Some(0));
    assert_eq!(leaf.history().len(), 2);
    let serialized = to_value(leaf).unwrap();
    for key in ["server", "port", "cipher", "alive", "udp"] {
        assert!(serialized.get(key).is_none());
    }
}

#[test]
fn extra_reported_metadata_cannot_change_the_latest_latency_or_override_explicit_zero() {
    let payload = r#"{"proxies":{
        "leaf":{
            "type":"Shadowsocks","name":"leaf","udp":true,"alive":false,
            "server":"node.example.test","port":443,"cipher":"aes-128-gcm",
            "delay":0,"history":[{"time":"last","delay":85}]
        }
    }}"#;
    let proxies = from_str::<ProxiesResponse>(payload).unwrap().proxies;
    let leaf = &proxies["leaf"];
    assert_eq!(leaf.delay(), Some(0), "explicit zero wins over history");
    assert_eq!(leaf.health_observation(), Some(false));

    let sparse: ProxiesResponse = from_str(
        r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","history":[{"time":"last","delay":85}]}}}"#,
    )
    .unwrap();
    assert_eq!(sparse.proxies["leaf"].delay(), Some(85));

    let mut explicit_timeout = to_value(leaf).unwrap();
    explicit_timeout["delay"] = Value::from(0);
    let timeout: ProxiesResponse = serde_json::from_value(serde_json::json!({
        "proxies": {"leaf": explicit_timeout}
    }))
    .unwrap();
    assert_eq!(timeout.proxies["leaf"].delay(), Some(0));
    assert_eq!(timeout.proxies["leaf"].health_observation(), Some(false));
}

#[test]
fn malformed_reported_fields_and_mismatched_identity_are_rejected_without_fallback() {
    for payload in [
        r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","server":123}}}"#,
        r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","port":70000}}}"#,
        r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","alive":"true"}}}"#,
        r#"{"proxies":{"leaf":{"type":"Shadowsocks","name":"leaf","history":"missing"}}}"#,
        r#"{"proxies":{"leaf":{"type":"","name":"leaf"}}}"#,
    ] {
        assert!(
            from_str::<ProxiesResponse>(payload).is_err(),
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
