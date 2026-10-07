//! Behavior cases for switch.
//! test-intent: behavior

use super::*;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

#[tokio::test]
async fn test_switch_nonexistent_profile_returns_error() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let payload = SwitchProfilePayload {
        name: "i-do-not-exist".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/profiles/switch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_client_error() || response.status().is_server_error());
    clear_home_dir_override();
}
