//! Behavior cases for import.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

#[tokio::test]
async fn test_import_profile_integration() {
    let _guard = TEST_LOCK.lock().await;

    let mut server = mockito::Server::new_async().await;
    let mock_yaml = "port: 7890\nmode: rule";
    let _m = server
        .mock("GET", "/sub")
        .with_status(200)
        .with_body(mock_yaml)
        .create_async()
        .await;

    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let payload = ImportProfilePayload {
        name: "test-import".to_string(),
        url: format!("{}/sub", server.url()),
        activate: Some(true),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/profiles/import")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);

    if status != StatusCode::OK {
        panic!(
            "Import profile failed with status {}. Body: {}",
            status, body_str
        );
    }

    // Verify file was saved
    let config_path = temp_dir.path().join("configs").join("test-import.yaml");
    assert!(config_path.exists());

    clear_home_dir_override();
}
