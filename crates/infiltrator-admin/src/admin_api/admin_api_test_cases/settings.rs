//! Behavior cases for settings.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn test_settings_configs_dir_roundtrip() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let payload = serde_json::json!({ "configs_dir": "/custom/configs" });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["configs_dir"], "/custom/configs");

    // A blank override means "unset" and must round-trip to null.
    let payload = serde_json::json!({ "configs_dir": "   " });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // An absent key must leave the stored value untouched.
    let payload = serde_json::json!({ "language": "en-US" });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["configs_dir"], serde_json::Value::Null);
    assert_eq!(json["language"], "en-US");
}

/// 0.20 OS 系统通知开关：POST 持久化 + GET 回填，且缺省键不动存量值。
#[tokio::test]
async fn test_settings_notifications_enabled_roundtrip() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    // 缺省开启。
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["notifications_enabled"], true);

    // 显式关闭并回读。
    let payload = serde_json::json!({ "notifications_enabled": false });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["notifications_enabled"], false);

    // 不携带该键的保存不得改动已持久化的关闭状态。
    let payload = serde_json::json!({ "language": "en-US" });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["notifications_enabled"], false);
    assert_eq!(json["language"], "en-US");
}

/// WebDAV 密码永不落盘/回传：POST 带非空 password → 进内存 keyring、
/// settings 快照无明文；GET 不回传 password 键；POST 空密码不动既有条目。
#[tokio::test]
async fn test_settings_webdav_password_roundtrip() {
    let _guard = TEST_LOCK.lock().await;
    let (app, secrets) = setup_app_with_runtime_and_secrets(None);
    let key = secrets_key();

    // POST：显式非空 password。
    let payload = serde_json::json!({
        "webdav": {
            "enabled": true,
            "url": "https://dav.example.com",
            "username": "user",
            "password": "s3cret"
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    assert_eq!(
        secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .map(String::as_str),
        Some("s3cret"),
        "non-empty password must land in the credential store"
    );

    // GET：webdav 在场，但不得携带 password 键。
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["webdav"]["url"], "https://dav.example.com");
    assert!(
        json["webdav"].get("password").is_none(),
        "GET must not echo the password back: {}",
        json["webdav"]
    );

    // POST 不带 password：既有 keyring 条目保持不动。
    let payload = serde_json::json!({
        "webdav": {
            "enabled": true,
            "url": "https://dav.example.com",
            "username": "user"
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .map(String::as_str),
        Some("s3cret"),
        "empty/absent password must leave the stored entry untouched"
    );

    // 内存 settings 快照同样无明文（宿主落盘时被 skip_serializing 跳过）。
    let body = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body.status(), StatusCode::OK);
    let raw = to_bytes(body.into_body(), 4096).await.unwrap();
    assert!(
        !String::from_utf8_lossy(&raw).contains("s3cret"),
        "settings snapshot must not leak plaintext"
    );
}
