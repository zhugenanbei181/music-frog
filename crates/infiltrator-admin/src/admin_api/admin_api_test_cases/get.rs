//! Behavior cases for get.
//! test-intent: behavior

use super::*;
use crate::support::select_test_profile;
use axum::body::to_bytes;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

#[tokio::test]
async fn test_get_profiles_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");

    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_settings_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_capabilities_route() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 2048).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["schema_version"], 2);
    assert!(json["runtime"]["status"].as_bool().unwrap());
    assert!(json["runtime"]["routing_intent"].as_bool().unwrap());
    assert!(json["outbound"]["inventory"].as_bool().unwrap());
    assert!(json["outbound"]["selection"].as_bool().unwrap());
    assert!(json["outbound"]["probe"].as_bool().unwrap());
    assert!(json["outbound"]["batch_probe"].as_bool().unwrap());
    assert!(json.get("proxy").is_none());
}

#[tokio::test]
async fn test_get_runtime_status_route_when_stopped() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/runtime/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 2048).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["running"], false);
}

#[tokio::test]
async fn test_get_rebuild_status_reflects_reality() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/rebuild/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();

    assert_eq!(body["in_progress"], false);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_dns_config_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(&manager, "default", "dns:\n  enable: true")
        .await
        .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/dns")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_tun_config_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(&manager, "default", "tun:\n  enable: true")
        .await
        .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/tun")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_rules_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(&manager, "default", "rules:\n  - DIRECT")
        .await
        .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_proxy_providers_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(
        &manager,
        "default",
        "proxy-providers:\n  p1:\n    type: http\n    url: https://example.com/p1.yaml\n",
    )
    .await
    .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/proxy-providers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_sniffer_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());
    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(
        &manager,
        "default",
        "sniffer:\n  enable: true\n  sniff:\n    TLS:\n      ports: [443]\n",
    )
    .await
    .unwrap();
    select_test_profile(&manager, "default").await;

    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/sniffer")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    clear_home_dir_override();
}

#[tokio::test]
async fn test_get_latest_stable_core_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    let app = setup_app_with_static_version(temp_dir.path());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/core/latest-stable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["version"], "v1.20.0");
    assert_eq!(json["release_date"], "2026-01-01T00:00:00Z");
}
