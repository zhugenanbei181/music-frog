use tokio_tungstenite::tungstenite::Message;

use super::*;
use mockito::Server;

#[tokio::test]
async fn test_client_new() {
    let client = MihomoClient::new("http://127.0.0.1:9090", None);
    assert!(client.is_ok());
}

#[tokio::test]
async fn test_get_version() {
    let mut server = Server::new_async().await;
    let body = json!({
        "version": "v1.18.0",
        "premium": false
    });

    let mock = server
        .mock("GET", "/version")
        .with_status(200)
        .with_body(serde_json::to_string(&body).unwrap())
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), None).unwrap();
    let version = client.get_version().await.unwrap();

    mock.assert_async().await;
    assert_eq!(version.version, "v1.18.0");
    assert!(!version.premium);
}

#[tokio::test]
async fn test_get_proxies() {
    let mut server = Server::new_async().await;
    let body = json!({
        "proxies": {
            "GLOBAL": {
                "type": "Selector",
                "name": "GLOBAL",
                "now": "Proxy-A",
                "all": ["Proxy-A", "Proxy-B"],
                "history": []
            },
            "Proxy-A": {
                "type": "Shadowsocks",
                "name": "Proxy-A",
                "udp": true,
                "history": [],
                "alive": true,
                "server": "1.1.1.1",
                "port": 443,
                "cipher": "aes-256-gcm"
            }
        }
    });

    let mock = server
        .mock("GET", "/proxies")
        .with_status(200)
        .with_body(serde_json::to_string(&body).unwrap())
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), None).unwrap();
    let proxies = client.get_proxies().await.unwrap();

    mock.assert_async().await;
    assert!(proxies.contains_key("GLOBAL"));
    assert!(proxies.contains_key("Proxy-A"));
}

#[tokio::test]
async fn test_switch_proxy() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PUT", "/proxies/GLOBAL")
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), None).unwrap();
    let result = client.switch_proxy("GLOBAL", "Proxy-B").await;

    mock.assert_async().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_stream_traffic() {
    let addr = "127.0.0.1:19090";
    let server = tokio::net::TcpListener::bind(addr).await.unwrap();

    tokio::spawn(async move {
        use futures_util::SinkExt;
        if let Ok((stream, _)) = server.accept().await
            && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
        {
            let traffic = json!({
                "up": 1024,
                "down": 2048
            });
            let _ = ws_stream
                .send(Message::Text(
                    serde_json::to_string(&traffic).unwrap().into(),
                ))
                .await;
        }
    });

    let client = MihomoClient::new(&format!("http://{}", addr), None).unwrap();
    let mut rx = client.stream_traffic().await.unwrap();

    let data = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap();
    assert!(data.is_some());
    let traffic = data.unwrap();
    assert_eq!(traffic.up, 1024);
    assert_eq!(traffic.down, 2048);
}

#[tokio::test]
async fn test_stream_connections() {
    let addr = "127.0.0.1:19091";
    let server = tokio::net::TcpListener::bind(addr).await.unwrap();

    tokio::spawn(async move {
        use futures_util::SinkExt;
        if let Ok((stream, _)) = server.accept().await
            && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
        {
            let snapshot = json!({
                "downloadTotal": 1000,
                "uploadTotal": 2000,
                "connections": []
            });
            let _ = ws_stream
                .send(Message::Text(
                    serde_json::to_string(&snapshot).unwrap().into(),
                ))
                .await;
        }
    });

    let client = MihomoClient::new(&format!("http://{}", addr), None).unwrap();
    let mut rx = client.stream_connections().await.unwrap();

    let data = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap();
    assert!(data.is_some());
}

#[tokio::test]
async fn test_stream_events_expose_lifecycle_and_data() {
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    tokio::spawn(async move {
        use futures_util::SinkExt;
        if let Ok((stream, _)) = server.accept().await
            && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
        {
            let traffic = TrafficData { up: 11, down: 22 };
            let _ = ws_stream
                .send(Message::Text(
                    serde_json::to_string(&traffic).unwrap().into(),
                ))
                .await;
            let _ = ws_stream.close(None).await;
        }
    });

    let client = MihomoClient::new(&format!("http://{addr}"), None).unwrap();
    let mut rx = client.stream_traffic_events().await.unwrap();
    let mut saw_connecting = false;
    let mut saw_connected = false;
    let mut saw_item = false;
    for _ in 0..6 {
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        let Some(event) = event else { break };
        match event {
            StreamEvent::Connecting => saw_connecting = true,
            StreamEvent::Connected => saw_connected = true,
            StreamEvent::Item(data) => {
                assert_eq!(data.down, 22);
                saw_item = true;
                break;
            }
            StreamEvent::Reconnecting(_) | StreamEvent::Failed(_) => {}
        }
    }
    assert!(saw_connecting);
    assert!(saw_connected);
    assert!(saw_item);
}

#[tokio::test]
async fn test_restart_core() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/restart")
        .match_header("authorization", "Bearer test-secret")
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let result = client.restart_core().await;

    mock.assert_async().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reload_config_uses_force_query_path_and_auth() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PUT", "/configs")
        .match_query("force=true")
        .match_header("authorization", "Bearer test-secret")
        .match_body(mockito::Matcher::JsonString(
            json!({ "path": "/tmp/profile.yaml" }).to_string(),
        ))
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    client
        .reload_config(Some("/tmp/profile.yaml"))
        .await
        .expect("mihomo hot reload request");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_reload_config_surfaces_controller_rejection() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PUT", "/configs")
        .match_query("force=true")
        .with_status(400)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), None).unwrap();
    assert!(client.reload_config(None).await.is_err());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_restart_core_accepts_json_response() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/restart")
        .with_status(200)
        .with_body(json!({ "message": "restarting" }).to_string())
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), None).unwrap();
    let result = client.restart_core().await;

    mock.assert_async().await;
    assert!(result.is_ok(), "a JSON success body must also be Ok(())");
}

#[tokio::test]
async fn test_provider_healthcheck() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/providers/proxies/My%20Provider/healthcheck")
        .match_header("authorization", "Bearer test-secret")
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let result = client.provider_healthcheck("My Provider").await;

    mock.assert_async().await;
    assert!(result.is_ok());
}

#[test]
fn test_provider_name_is_percent_encoded_as_path_segment() {
    let client = MihomoClient::new("http://127.0.0.1:9090", None).unwrap();
    let url = client
        .build_url_with_segments(&["providers", "proxies", "My Provider #1?", "healthcheck"])
        .unwrap();
    assert_eq!(
        url.as_str(),
        "http://127.0.0.1:9090/providers/proxies/My%20Provider%20%231%3F/healthcheck",
        "space/#/? must be encoded inside the provider segment, not split off"
    );
}

#[tokio::test]
async fn test_flush_fakeip_cache() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/cache/fakeip/flush")
        .match_header("authorization", "Bearer test-secret")
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let result = client.flush_fakeip_cache().await;

    mock.assert_async().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_upgrade_geo() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/upgrade/geo")
        .match_header("authorization", "Bearer test-secret")
        .with_status(204)
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let result = client.upgrade_geo().await;

    mock.assert_async().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_fakeip_cache() {
    let mut server = Server::new_async().await;
    let body = json!({
        "music.example.org": "198.18.0.5",
        "cdn.example.net": "198.18.0.7"
    });
    let mock = server
        .mock("GET", "/cache/fakeip")
        .match_header("authorization", "Bearer test-secret")
        .with_status(200)
        .with_body(serde_json::to_string(&body).unwrap())
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let cache = client.fakeip_cache().await.unwrap();

    mock.assert_async().await;
    // Keys are dynamic domain names; Value keeps the original shape.
    assert_eq!(
        cache.get("music.example.org").and_then(|v| v.as_str()),
        Some("198.18.0.5")
    );
    assert!(cache.is_object());
}

#[tokio::test]
async fn test_group_delay() {
    let mut server = Server::new_async().await;
    let body = json!({
        "Proxy-A": 123,
        "Proxy-B": "An error occurred in the delay test"
    });
    let mock = server
        .mock("GET", "/group/My%20Group/delay")
        .match_query("url=https%3A%2F%2Fexample.com%2Fclash&timeout=5000")
        .match_header("authorization", "Bearer test-secret")
        .with_status(200)
        .with_body(serde_json::to_string(&body).unwrap())
        .create_async()
        .await;

    let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
    let delays = client
        .group_delay("My Group", "https://example.com/clash", 5000)
        .await
        .unwrap();

    mock.assert_async().await;
    // Numeric delay for a reachable node...
    assert_eq!(delays.get("Proxy-A").and_then(|v| v.as_u64()), Some(123));
    // ...and an error string for one that failed the test.
    assert_eq!(
        delays.get("Proxy-B").and_then(|v| v.as_str()),
        Some("An error occurred in the delay test")
    );
}

#[test]
fn test_group_name_is_percent_encoded_in_delay_url() {
    let client = MihomoClient::new("http://127.0.0.1:9090", None).unwrap();
    let mut url = client
        .build_url_with_segments(&["group", "My Group #1", "delay"])
        .unwrap();
    url.query_pairs_mut()
        .append_pair("url", "https://example.com/ping")
        .append_pair("timeout", "3000");
    assert_eq!(
        url.as_str(),
        "http://127.0.0.1:9090/group/My%20Group%20%231/delay?url=https%3A%2F%2Fexample.com%2Fping&timeout=3000",
        "group segment must be encoded while query pairs use form encoding"
    );
}
