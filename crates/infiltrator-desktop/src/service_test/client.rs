//! Behavior cases for client.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn test_client_graceful_degradation_non_existent_service() {
    let temp_dir = TempDir::new().unwrap();
    let missing_socket = temp_dir.path().join("non_existent_infiltrator.sock");
    let endpoint = IpcEndpoint::from_unix_path(missing_socket);
    let client = ServiceClient::new(endpoint, AuthToken::generate());

    assert!(!client.is_service_available().await);
    assert_eq!(client.check_state().await, ServiceState::Stopped);

    let fallback_info = client.query_status_or_fallback().await;
    assert_eq!(fallback_info.state, ServiceState::Stopped);
    assert!(!fallback_info.tun_active);
    assert!(!fallback_info.system_proxy_active);
}
