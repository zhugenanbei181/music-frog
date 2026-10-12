//! Behavior cases for flush.
//! test-intent: behavior

use super::*;
use crate::support::{app_config_manager, select_test_profile};
use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
use infiltrator_ports::profile_store::ProfileStore;

#[tokio::test]
async fn test_flush_fake_ip_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = app_config_manager().await.unwrap();
    ProfileStore::save_inactive_profile(&manager, "default", "port: 7890")
        .await
        .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/fake-ip/flush")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}
