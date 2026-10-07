//! Behavior cases for download.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
use std::fs::read;
use std::process;

#[tokio::test]
async fn test_download_core_installed_version_route() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    let version = "v1.20.0";
    plant_runnable_fake_binary(temp_dir.path(), version);
    let planted = temp_dir.path().join("versions/v1.20.0/mihomo");
    assert!(planted.exists(), "planted binary missing before request");
    let raw = read(&planted).unwrap();
    println!("planted bytes: {:?}", String::from_utf8_lossy(&raw));
    let direct = process::Command::new(&planted).arg("-v").output().unwrap();
    println!(
        "direct exec: status={:?} out={:?}",
        direct.status,
        String::from_utf8_lossy(&direct.stdout)
    );
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let payload = serde_json::json!({ "version": version });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/core/download")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["version"], version);
    assert_eq!(json["downloaded"], false);
    assert_eq!(json["already_installed"], true);
    clear_home_dir_override();
}
