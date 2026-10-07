//! Behavior cases for script.
//! test-intent: behavior

use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn test_script_endpoints_presets_and_execute() {
    let _guard = TEST_LOCK.lock().await;
    let app = setup_app();

    // 1. GET /admin/api/scripts/presets
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/scripts/presets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let presets = json["presets"].as_array().expect("presets array");
    assert_eq!(presets.len(), 4);

    // 2. POST /admin/api/scripts/validate (valid script)
    let val_payload = serde_json::json!({
        "script": "function main(config) { filter_nodes_by_regex(config, 'ad', true); return config; }"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/scripts/validate")
                .header("content-type", "application/json")
                .body(Body::from(val_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["valid"], true);
    assert_eq!(json["entry_point_found"], true);

    // 3. POST /admin/api/scripts/execute
    let exec_payload = serde_json::json!({
        "script": "function main(config) {\n  filter_nodes_by_regex(config, '官网|广告', true);\n  console.log('Filtered ad nodes');\n  return config;\n}",
        "yaml_content": "proxies:\n  - name: \"🇭🇰 香港 01\"\n    type: ss\n  - name: \"官网-广告节点\"\n    type: ss\n",
        "stage": "pre_merge",
        "timeout_ms": 500
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/scripts/execute")
                .header("content-type", "application/json")
                .body(Body::from(exec_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert!(
        json["transformed_yaml"]
            .as_str()
            .unwrap()
            .contains("🇭🇰 香港 01")
    );
    assert!(
        !json["transformed_yaml"]
            .as_str()
            .unwrap()
            .contains("官网-广告节点")
    );
    assert_eq!(json["console_logs"][0], "Filtered ad nodes");
}
