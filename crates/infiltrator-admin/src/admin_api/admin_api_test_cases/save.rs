//! Behavior cases for save.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
use std::time;
use tokio::time::sleep;

#[tokio::test]
async fn test_save_invalid_yaml_returns_400() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();
    let payload = SaveProfilePayload {
        name: "invalid-yaml".to_string(),
        content: "key: : : : value".to_string(), // Invalid YAML
        activate: Some(false),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/profiles/save")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_save_rules_route_schedules_rebuild() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    manager.save("default", "rules:\n  - DIRECT").await.unwrap();

    let app = setup_app();
    let payload = serde_json::json!({
        "rules": [
            {
                "rule": "DOMAIN,example.com,DIRECT",
                "enabled": true
            }
        ]
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/rules")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["rules"][0]["rule"], "DOMAIN,example.com,DIRECT");
    assert_eq!(json["rules"][0]["enabled"], true);

    sleep(time::Duration::from_millis(40)).await;
    let status_response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/rebuild/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_body = to_bytes(status_response.into_body(), 2048).await.unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["in_progress"], false);
    assert_eq!(status_json["last_error"], serde_json::Value::Null);
    assert_eq!(status_json["last_reason"], "rules-update");
    clear_home_dir_override();
}
