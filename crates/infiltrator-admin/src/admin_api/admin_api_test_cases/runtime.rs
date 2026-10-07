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
async fn test_runtime_proxy_delays_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{
                    "proxies": {
                        "GLOBAL": {"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},
                        "Proxy-A": {"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[{"time":"2026-02-06T00:00:00Z","delay":120}],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},
                        "Proxy-B": {"type":"Shadowsocks","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}
                    }
                }"#,
        )
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/proxies")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(json["nodes"][0]["name"], "Proxy-A");
    assert_eq!(json["nodes"][0]["delay_ms"], 120);
    assert_eq!(
        json["default_test_url"],
        "http://www.gstatic.com/generate_204"
    );
    assert_eq!(json["default_timeout_ms"], 5000);
}

#[tokio::test]
async fn test_runtime_proxy_delay_test_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex("^/proxies/proxy1/delay(\\?.*)?$".to_string()),
        )
        .with_status(200)
        .with_body(r#"{"delay":123}"#)
        .create_async()
        .await;
    let app = setup_app_with_runtime(Some(server.url()));
    let payload = serde_json::json!({ "proxy": "proxy1" });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/runtime/delay/test")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["proxy"], "proxy1");
    assert_eq!(json["delay_ms"], 123);
    assert_eq!(json["test_url"], "http://www.gstatic.com/generate_204");
    assert_eq!(json["timeout_ms"], 5000);
}

#[tokio::test]
async fn test_runtime_proxy_delay_test_all_route() {
    let _guard = TEST_LOCK.lock().await;
    let mut server = mockito::Server::new_async().await;
    let _m_proxies = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(
            r#"{
                    "proxies": {
                        "GLOBAL": {"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},
                        "Proxy-A": {"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},
                        "Proxy-B": {"type":"Shadowsocks","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}
                    }
                }"#,
        )
        .create_async()
        .await;
    let _m_ok = server
        .mock(
            "GET",
            mockito::Matcher::Regex("^/proxies/Proxy-A/delay(\\?.*)?$".to_string()),
        )
        .with_status(200)
        .with_body(r#"{"delay":88}"#)
        .create_async()
        .await;
    let _m_fail = server
        .mock(
            "GET",
            mockito::Matcher::Regex("^/proxies/Proxy-B/delay(\\?.*)?$".to_string()),
        )
        .with_status(500)
        .with_body(r#"{"error":"failed"}"#)
        .create_async()
        .await;

    let app = setup_app_with_runtime(Some(server.url()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/runtime/delay/test-all")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success_count"], 1);
    assert_eq!(json["failed_count"], 1);
    assert_eq!(json["results"].as_array().unwrap().len(), 2);
}
