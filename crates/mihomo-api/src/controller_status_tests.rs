//! test-intent: behavior
use crate::client::MihomoClient;
use crate::overview::ControllerOverviewReader;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use mockito::Server;
use serde_json::json;

#[tokio::test]
async fn controller_permission_denial_is_typed_even_when_the_body_contains_valid_proxy_data() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/proxies")
        .with_status(401)
        .with_body(include_str!(
            "../tests/fixtures/runtime-proxies-v1.19.18.json"
        ))
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let error = RuntimeGateway::get_proxies(&client).await.unwrap_err();
    let failure = Failure::from(error);
    assert_eq!(failure.code, ErrorCode::Authentication);
    assert!(!failure.retryable);
    mock.assert_async().await;
}

#[tokio::test]
async fn failed_controller_mutations_never_return_success_and_can_retry_after_recovery() {
    let mut server = Server::new_async().await;
    let denial = server
        .mock("DELETE", "/connections")
        .with_status(403)
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let error = RuntimeGateway::close_all_connections(&client)
        .await
        .unwrap_err();
    assert!(matches!(error, PortError::PermissionDenied(_)));
    denial.assert_async().await;
    denial.remove_async().await;
    let success = server
        .mock("DELETE", "/connections")
        .with_status(204)
        .create_async()
        .await;
    RuntimeGateway::close_all_connections(&client)
        .await
        .unwrap();
    success.assert_async().await;
    let patch = server
        .mock("PATCH", "/configs")
        .with_status(503)
        .create_async()
        .await;
    assert!(matches!(
        RuntimeGateway::patch_config(&client, json!({"mode":"rule"}))
            .await
            .unwrap_err(),
        PortError::Network(_)
    ));
    patch.assert_async().await;
    let select = server
        .mock("PUT", "/proxies/Group")
        .with_status(400)
        .create_async()
        .await;
    let failure = Failure::from(
        RuntimeGateway::switch_proxy(&client, "Group", "leaf")
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert!(!failure.retryable);
    select.assert_async().await;
}

#[tokio::test]
async fn mode_change_verifies_actual_readback_and_preserves_denied_and_retryable_failure_categories()
 {
    let mut server = Server::new_async().await;
    let reader = ControllerOverviewReader::new(MihomoClient::new(&server.url(), None).unwrap());
    let denied = server
        .mock("PATCH", "/configs")
        .with_status(401)
        .create_async()
        .await;
    let not_read = server
        .mock("GET", "/configs")
        .expect(0)
        .create_async()
        .await;
    let failure = Failure::from(reader.set_mode(ProxyMode::Global).await.unwrap_err());
    assert_eq!(failure.code, ErrorCode::Authentication);
    assert!(!failure.retryable);
    denied.assert_async().await;
    not_read.assert_async().await;
    denied.remove_async().await;
    not_read.remove_async().await;
    let accepted = server
        .mock("PATCH", "/configs")
        .with_status(204)
        .expect(3)
        .create_async()
        .await;
    let retained = server
        .mock("GET", "/configs")
        .with_status(200)
        .with_body(
            json!({"mode":"rule","port":0,"socks-port":0,"redir-port":0,"tproxy-port":0,"mixed-port":7890,"allow-lan":false,"log-level":"info"})
                .to_string(),
        )
        .create_async()
        .await;
    let failure = Failure::from(reader.set_mode(ProxyMode::Global).await.unwrap_err());
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(failure.retryable);
    retained.assert_async().await;
    retained.remove_async().await;
    let read_failed = server
        .mock("GET", "/configs")
        .with_status(503)
        .create_async()
        .await;
    let failure = Failure::from(reader.set_mode(ProxyMode::Global).await.unwrap_err());
    assert_eq!(failure.code, ErrorCode::Network);
    assert!(failure.retryable);
    read_failed.assert_async().await;
    read_failed.remove_async().await;
    let applied = server.mock("GET", "/configs").with_status(200)
        .with_body(json!({"mode":"global","port":0,"socks-port":0,"redir-port":0,"tproxy-port":0,"mixed-port":7890,"allow-lan":false,"log-level":"info"}).to_string())
        .create_async().await;
    assert_eq!(
        reader.set_mode(ProxyMode::Global).await.unwrap(),
        ProxyMode::Global
    );
    applied.assert_async().await;
    accepted.assert_async().await;
}
