//! Behavior cases for admin.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn test_admin_api_token_auth_isolation() {
    let _guard = TEST_LOCK.lock().await;
    let token = "test_super_secret_admin_token_456".to_string();
    let app = setup_app_with_auth(Some(token.clone()));

    // 1. Request with no token -> 401 Unauthorized
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 2. Request with invalid Bearer token -> 401 Unauthorized
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .header("Authorization", "Bearer wrong_token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 3. Request with valid Bearer token -> 200 OK
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 4. Request with valid x-admin-token header -> 200 OK
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .header("x-admin-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 5. Request with query param token -> 200 OK
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/api/capabilities?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 6. Request with unconfigured auth_token -> allows access
    let open_app = setup_app_with_auth(None);
    let response = open_app
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
