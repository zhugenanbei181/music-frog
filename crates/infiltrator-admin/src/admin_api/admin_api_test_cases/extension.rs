//! Behavior cases for extension.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn test_extension_endpoints_package_and_manifest() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    // 1. POST /admin/api/extensions/package/export
    let ext_pkg = serde_json::json!({
        "package": {
            "name": "Auto Country Router",
            "version": "1.0.0",
            "author": "Infiltrator",
            "description": "Auto groups nodes by country",
            "stage": "pre_merge",
            "script_code": "function main(config) { auto_country_groups(config); return config; }",
            "mixin_yaml": null,
            "tags": ["country", "auto"]
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/extensions/package/export")
                .header("content-type", "application/json")
                .body(Body::from(ext_pkg.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let export_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let exported_json_str = export_json["json"].as_str().unwrap();
    let checksum = export_json["checksum"].as_str().unwrap();
    assert!(!checksum.is_empty());

    // 2. POST /admin/api/extensions/package/import
    let import_payload = serde_json::json!({
        "json": exported_json_str,
        "expected_checksum": checksum
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/extensions/package/import")
                .header("content-type", "application/json")
                .body(Body::from(import_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let imported_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(imported_json["package"]["name"], "Auto Country Router");

    // 3. POST /admin/api/extensions/manifest/validate
    let manifest_payload = serde_json::json!({
        "manifest": {
            "id": "ext-test",
            "name": "Extension Test",
            "version": "1.0.0",
            "author": "Dev",
            "description": "Valid test manifest",
            "permissions": ["network_access", "modify_rules"],
            "settings_schema": [
                {
                    "key": "enable_auto",
                    "label": "Enable Auto",
                    "field_type": "boolean",
                    "default_value": true
                }
            ]
        }
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/extensions/manifest/validate")
                .header("content-type", "application/json")
                .body(Body::from(manifest_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let res: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(res["valid"], true);
}
