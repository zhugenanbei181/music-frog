//! Behavior cases for runtime.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn test_runtime_connections_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/connections")
        .with_status(200)
        .with_body(
            r#"{
                    "downloadTotal": 1000,
                    "uploadTotal": 2000,
                    "connections": [{
                        "id":"c1",
                        "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                        "uploadTotal": 100,
                        "downloadTotal": 200,
                        "start": "2024-01-01T00:00:00Z",
                        "rule": "Match",
                        "rulePayload": ""
                    }]
                }"#,
        )
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/connections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_runtime_memory_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/memory")
        .with_status(200)
        .with_body(r#"{"inuse":123,"oslimit":456}"#)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/memory")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["inuse"], 123);
    assert_eq!(json["oslimit"], 456);
}

#[tokio::test]
async fn test_runtime_close_single_connection_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("DELETE", "/connections/c1")
        .with_status(204)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/runtime/connections/c1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_runtime_traffic_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/connections")
        .with_status(200)
        .with_body(
            r#"{
                    "downloadTotal": 3000,
                    "uploadTotal": 4000,
                    "connections": [
                        {
                            "id":"c1",
                            "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                            "uploadTotal": 100,
                            "downloadTotal": 200,
                            "start": "2024-01-01T00:00:00Z",
                            "rule": "Match",
                            "rulePayload": ""
                        },
                        {
                            "id":"c2",
                            "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                            "uploadTotal": 100,
                            "downloadTotal": 200,
                            "start": "2024-01-01T00:00:00Z",
                            "rule": "Match",
                            "rulePayload": ""
                        }
                    ]
                }"#,
        )
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/traffic")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["up_total"], 4000);
    assert_eq!(json["down_total"], 3000);
    assert_eq!(json["connections"], 2);
}

#[tokio::test]
async fn test_runtime_logs_invalid_level_returns_400() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app_with_runtime(Some("http://127.0.0.1:65535".to_string()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/logs?level=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_admin_outbound_route_returns_typed_inventory_and_revision() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},"Proxy-B":{"type":"Trojan","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443}}}"#,
        )
        .expect(1)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/outbounds")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["inventory"]["revision"], 1);
    let entries = json["inventory"]["inventory"]["ready"]["entries"]
        .as_array()
        .unwrap();
    let proxy_a = entries
        .iter()
        .find(|entry| entry["id"]["value"] == "Proxy-A")
        .expect("typed Proxy-A entry");
    assert_eq!(
        proxy_a["id"],
        serde_json::json!({ "engine_id": "mihomo", "value": "Proxy-A" })
    );
    assert!(json["details"]["detail_revision"].as_u64().is_some());
    assert!(json.get("groups").is_none());
}

#[tokio::test]
async fn test_admin_typed_selection_uses_observed_id_revision_and_readback() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let initial = r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},"Proxy-B":{"type":"Trojan","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443}}}"#;
    let selected = r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-B","all":["Proxy-A","Proxy-B"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},"Proxy-B":{"type":"Trojan","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443}}}"#;
    let _m_before = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(initial)
        .expect(4)
        .create_async()
        .await;
    let _m_select = server
        .mock("PUT", "/proxies/GLOBAL")
        .match_body(mockito::Matcher::JsonString(
            r#"{"name":"Proxy-B"}"#.to_owned(),
        ))
        .with_status(204)
        .expect(1)
        .create_async()
        .await;
    let _m_after = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(selected)
        .expect(1)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/outbounds")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let observation: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    let expected_revision = observation["inventory"]["revision"].as_u64().unwrap();

    let payload = serde_json::json!({
        "collection_id": { "engine_id": "mihomo", "value": "GLOBAL" },
        "outbound_id": { "engine_id": "mihomo", "value": "Proxy-B" },
        "expected_revision": expected_revision
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/outbounds/select")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["revision"], 2);
    assert_eq!(
        json["inventory"]["ready"]["collections"][0]["selection"]["ready"]["selected"],
        serde_json::json!({ "engine_id": "mihomo", "value": "Proxy-B" })
    );
}

#[tokio::test]
async fn test_admin_typed_single_probe_returns_shared_probe_snapshot() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m_proxies = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}}}"#,
        )
        .expect(4)
        .create_async()
        .await;
    let _m_probe = server
        .mock("GET", "/proxies/Proxy-A/delay")
        .match_query("url=https%3A%2F%2Fexample.test%2Fhealth&timeout=4000")
        .with_status(200)
        .with_body(r#"{"delay":123}"#)
        .expect(1)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let payload = serde_json::json!({
        "outbound_id": { "engine_id": "mihomo", "value": "Proxy-A" },
        "expected_inventory_revision": 1,
        "test_url": "https://example.test/health",
        "timeout_ms": 4000
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/outbounds/probe")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["outbound_id"],
        serde_json::json!({ "engine_id": "mihomo", "value": "Proxy-A" })
    );
    assert_eq!(json["observed_inventory_revision"], 1);
    assert_eq!(json["state"]["measured"]["delay_ms"], 123);
}

#[tokio::test]
async fn test_admin_typed_batch_probe_preserves_requested_id_order_and_outcomes() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m_proxies = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},"Proxy-B":{"type":"Trojan","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443}}}"#,
        )
        .expect(6)
        .create_async()
        .await;
    let _m_a = server
        .mock("GET", "/proxies/Proxy-A/delay")
        .match_query("url=https%3A%2F%2Fexample.test%2Fhealth&timeout=4000")
        .with_status(200)
        .with_body(r#"{"delay":19}"#)
        .expect(1)
        .create_async()
        .await;
    let _m_b = server
        .mock("GET", "/proxies/Proxy-B/delay")
        .match_query("url=https%3A%2F%2Fexample.test%2Fhealth&timeout=4000")
        .with_status(200)
        .with_body(r#"{"delay":11}"#)
        .expect(1)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let payload = serde_json::json!({
        "target_scope": {
            "selected": [
                { "engine_id": "mihomo", "value": "Proxy-B" },
                { "engine_id": "mihomo", "value": "Proxy-A" }
            ]
        },
        "expected_inventory_revision": 1,
        "test_url": "https://example.test/health",
        "timeout_ms": 4000,
        "concurrency": 2
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/outbounds/probe-batch")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
    assert_eq!(json["items"][0]["outbound_id"]["value"], "Proxy-B");
    assert_eq!(json["items"][0]["outcome"]["measured"]["delay_ms"], 11);
    assert_eq!(json["items"][1]["outbound_id"]["value"], "Proxy-A");
    assert_eq!(json["items"][1]["outcome"]["measured"]["delay_ms"], 19);
}

#[tokio::test]
async fn test_admin_routing_intent_route_is_typed_and_scoped() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/routing-intent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["scope"]["identity"]["engine"], "mihomo");
    assert_eq!(json["snapshot"]["current"]["kind"], "follow_rules");
    assert_eq!(json["snapshot"]["status"], "Ready");

    let payload = serde_json::json!({
        "scope": test_outbound_scope(),
        "intent": { "kind": "bypass_proxy" }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/routing-intent")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["scope"]["generation"], 1);
    assert_eq!(json["snapshot"]["current"]["kind"], "bypass_proxy");
}

#[tokio::test]
async fn test_admin_typed_selection_rejects_stale_revision_before_controller_write() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m_inventory = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{"proxies":{"GLOBAL":{"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A"],"history":[]},"Proxy-A":{"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}}}"#,
        )
        .expect(2)
        .create_async()
        .await;
    let _m_no_write = server
        .mock("PUT", "/proxies/GLOBAL")
        .expect(0)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let payload = serde_json::json!({
        "collection_id": { "engine_id": "mihomo", "value": "GLOBAL" },
        "outbound_id": { "engine_id": "mihomo", "value": "Proxy-A" },
        "expected_revision": 0
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/outbounds/select")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_admin_typed_selection_rejects_name_only_payload() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/outbounds/select")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"proxy":"Proxy-A"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_name_based_outbound_routes_are_removed() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();
    for (method, uri) in [
        ("GET", "/admin/api/proxies"),
        ("POST", "/admin/api/proxy/select"),
        ("GET", "/admin/api/runtime/proxies"),
        ("POST", "/admin/api/runtime/delay/test"),
        ("POST", "/admin/api/runtime/delay/test-all"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {uri}");
    }
}
