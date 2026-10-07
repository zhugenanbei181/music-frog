//! Behavior cases for activate.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
use std::time;
use tokio::fs::read_to_string;
use tokio::time::sleep;

#[tokio::test]
async fn test_activate_core_version_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    let version = "v1.19.0";
    plant_runnable_fake_binary(temp_dir.path(), version);
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let payload = serde_json::json!({ "version": version });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/core/activate")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

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
    assert_eq!(status_json["last_reason"], "core-activate");

    let config_file = temp_dir.path().join("config.toml");
    let content = read_to_string(config_file).await.unwrap();
    assert!(content.contains(&format!("version = \"{}\"", version)));
    clear_home_dir_override();
}
