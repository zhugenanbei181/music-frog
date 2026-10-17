//! test-intent: behavior
use crate::client::MihomoClient;
use crate::error::MihomoError;
use crate::network_error::network_error;
use crate::types::ConfigResponse;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::error::PortError;
use lurker_core_contract::error::{ErrorCode, Failure};
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
    let error = client.get_proxies().await.unwrap_err();
    let failure = Failure::from(network_error(error));
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
    let error = client.close_all_connections().await.unwrap_err();
    assert!(matches!(
        error,
        MihomoError::Http(ref response)
            if response.status().is_some_and(|status| status.as_u16() == 403)
    ));
    denial.assert_async().await;
    denial.remove_async().await;
    let success = server
        .mock("DELETE", "/connections")
        .with_status(204)
        .create_async()
        .await;
    client.close_all_connections().await.unwrap();
    success.assert_async().await;
    let select = server
        .mock("PUT", "/proxies/Group")
        .with_status(400)
        .create_async()
        .await;
    let failure = Failure::from(network_error(
        client.switch_proxy("Group", "leaf").await.unwrap_err(),
    ));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert!(!failure.retryable);
    select.assert_async().await;
}

#[tokio::test]
async fn core_log_level_port_uses_mihomo_wire_and_reads_back_a_typed_level() {
    let mut server = Server::new_async().await;
    let patch = server
        .mock("PATCH", "/configs")
        .match_body(mockito::Matcher::JsonString(
            json!({"log-level":"warn"}).to_string(),
        ))
        .with_status(204)
        .create_async()
        .await;
    let config = ConfigResponse {
        log_level: "warning".to_owned(),
        ..ConfigResponse::default()
    };
    let read = server
        .mock("GET", "/configs")
        .with_status(200)
        .with_body(serde_json::to_string(&config).unwrap())
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let observed = CoreLogLevelPort::set_core_log_level(&client, CoreLogLevel::Warn)
        .await
        .unwrap();
    assert_eq!(observed, CoreLogLevel::Warn);
    patch.assert_async().await;
    read.assert_async().await;
}

#[tokio::test]
async fn core_log_level_port_rejects_unknown_mihomo_readback() {
    let mut server = Server::new_async().await;
    let patch = server
        .mock("PATCH", "/configs")
        .match_body(mockito::Matcher::JsonString(
            json!({"log-level":"debug"}).to_string(),
        ))
        .with_status(204)
        .create_async()
        .await;
    let config = ConfigResponse {
        log_level: "trace".to_owned(),
        ..ConfigResponse::default()
    };
    let read = server
        .mock("GET", "/configs")
        .with_status(200)
        .with_body(serde_json::to_string(&config).unwrap())
        .create_async()
        .await;
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let failure = CoreLogLevelPort::set_core_log_level(&client, CoreLogLevel::Debug)
        .await
        .unwrap_err();
    assert!(matches!(failure, PortError::Failed(_)));
    patch.assert_async().await;
    read.assert_async().await;
}
