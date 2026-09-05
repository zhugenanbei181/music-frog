//! Focused controller-auth and live-config tests kept outside the large
//! client implementation so the production source stays within the line
//! budget.

use super::MihomoClient;
use mockito::Server;
use serde_json::json;

#[tokio::test]
async fn test_get_version_injects_controller_secret_as_bearer_auth() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/version")
        .match_header("authorization", "Bearer generated-by-host")
        .with_status(200)
        .with_body(json!({ "version": "v1.19.30", "premium": true }).to_string())
        .create_async()
        .await;

    let client = MihomoClient::new(
        &server.url(),
        Some("generated-by-host".to_owned()),
    )
    .unwrap();
    client.get_version().await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn test_patch_config_changes_log_level_and_surfaces_rejection() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PATCH", "/configs")
        .match_header("authorization", "Bearer generated-by-host")
        .match_body(mockito::Matcher::JsonString(
            json!({ "log-level": "debug" }).to_string(),
        ))
        .with_status(204)
        .create_async()
        .await;
    let client = MihomoClient::new(
        &server.url(),
        Some("generated-by-host".to_owned()),
    )
    .unwrap();
    client
        .patch_config(json!({ "log-level": "debug" }))
        .await
        .unwrap();
    mock.assert_async().await;

    let rejected = server
        .mock("PATCH", "/configs")
        .with_status(400)
        .create_async()
        .await;
    assert!(client.patch_config(json!({ "log-level": "trace" })).await.is_err());
    rejected.assert_async().await;
}
