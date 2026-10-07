//! Behavior cases for list.
//! test-intent: behavior

use super::*;
use mihomo_platform::paths::clear_home_dir_override;

#[tokio::test]
async fn test_list_core_versions_route() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/core/versions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}
