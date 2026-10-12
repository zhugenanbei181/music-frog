//! Behavior cases for delete.
//! test-intent: behavior

use super::*;
use crate::support::select_test_profile;
use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::manager::ConfigManager;

#[tokio::test]
async fn test_delete_active_profile_rejected() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(&manager, "active", "port: 7890")
        .await
        .unwrap();
    select_test_profile(&manager, "active").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/api/profiles/active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    clear_home_dir_override();
}
