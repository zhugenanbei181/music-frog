//! Behavior cases for doctor.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

#[tokio::test]
async fn test_doctor_checks_list_route() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/doctor/checks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    let checks = json.as_array().expect("checks must be a JSON array");
    assert!(!checks.is_empty(), "expected the full check metadata list");
    assert!(
        checks
            .iter()
            .any(|check| check["id"] == "config.settings_parse"),
        "known check id missing from metadata list"
    );
    for check in checks {
        assert!(check["id"].is_string());
        assert!(check["category"].is_string());
        assert!(check["summary"].is_string());
        assert!(check["fixable"].is_boolean());
        assert!(check["default_enabled"].is_boolean());
    }
}

#[tokio::test]
async fn test_doctor_check_detail_route() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/doctor/checks/config.settings_parse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(json["id"], "config.settings_parse");
    assert_eq!(json["category"], "config");
    assert!(json["why"].is_string());
    assert!(json["fail_means"].is_string());
    assert!(json["hint"].is_string());
}

#[tokio::test]
async fn test_doctor_check_detail_unknown_id_returns_404() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/doctor/checks/nope.nothing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 2048).await.unwrap()).unwrap();
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_doctor_run_route_shape_and_filter() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/doctor")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert!(json["started_at"].is_u64());
    assert!(json["finished_at"].is_u64());
    assert!(json["exit_code"].is_i64());
    assert!(json["exit_code"].as_i64().unwrap() >= 0);
    let checks = json["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty(), "unfiltered run must execute checks");
    for check in checks {
        assert!(check["id"].is_string());
        assert!(check["status"].is_string());
    }

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/doctor?only=config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    let checks = json["checks"].as_array().expect("filtered checks array");
    assert!(!checks.is_empty(), "config filter must select checks");
    assert!(
        checks.iter().all(|check| check["category"] == "config"),
        "filter must narrow the report to the config category"
    );

    clear_home_dir_override();
}

#[tokio::test]
async fn test_doctor_fix_route_is_idempotent() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/doctor/fix")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let first: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert!(first["actions"].is_array());

    // Every conservative repair is a no-op once its artifact exists.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/doctor/fix")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let second: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert!(
        second["actions"]
            .as_array()
            .expect("actions array")
            .is_empty(),
        "second fix run must change nothing, got: {second}"
    );

    clear_home_dir_override();
}
