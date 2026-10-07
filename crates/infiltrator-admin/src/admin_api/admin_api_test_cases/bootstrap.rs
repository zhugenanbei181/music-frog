//! Behavior cases for bootstrap.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

#[tokio::test]
async fn test_bootstrap_route_is_idempotent() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    let steps = json["steps"].as_array().expect("steps array");
    assert!(!steps.is_empty(), "bootstrap must report its steps");
    assert!(steps.iter().any(|step| step["id"] == "configs_dir"));
    assert!(steps.iter().any(|step| step["id"] == "default_config"));
    assert!(temp_dir.path().join("configs").is_dir());

    // Second run on the initialized home must skip every step.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    for step in json["steps"].as_array().expect("steps array") {
        assert_eq!(step["executed"], false, "step must be skipped: {step}");
    }

    clear_home_dir_override();
}
