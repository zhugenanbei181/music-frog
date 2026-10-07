//! Behavior cases for mock.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn test_mock_service_harness_requests() {
    let harness = MockServiceHarness::with_privilege(PrivilegeLevel::Admin);

    // 1. Authorized Ping
    let resp = harness
        .execute_request(
            harness.auth_token.secret(),
            ServiceCommand::Ping { nonce: 1000 },
        )
        .await
        .unwrap();
    assert!(resp.success);
    assert_eq!(
        resp.payload,
        Some(ServiceResponsePayload::Pong { nonce: 1000 })
    );

    // 2. Unauthorized request (wrong token)
    let resp_unauth = harness
        .execute_request("wrong-secret-token", ServiceCommand::Ping { nonce: 1000 })
        .await
        .unwrap();
    assert!(!resp_unauth.success);
    assert!(resp_unauth.error.unwrap().contains("Unauthorized"));

    // 3. Status Query
    let resp_status = harness
        .execute_request(harness.auth_token.secret(), ServiceCommand::QueryStatus)
        .await
        .unwrap();
    assert!(resp_status.success);
    if let Some(ServiceResponsePayload::Status(info)) = resp_status.payload {
        assert_eq!(info.privilege_level, PrivilegeLevel::Admin);
        assert_eq!(info.state, ServiceState::Running);
    } else {
        panic!("Expected Status payload");
    }

    // 4. Start Tun
    let resp_start_tun = harness
        .execute_request(
            harness.auth_token.secret(),
            ServiceCommand::StartTun {
                tun_interface: None,
                config_path: None,
            },
        )
        .await
        .unwrap();
    assert!(resp_start_tun.success);
    assert_eq!(
        resp_start_tun.payload,
        Some(ServiceResponsePayload::TunStarted {
            interface_name: Some("tun0".to_string())
        })
    );

    // 5. Stop Tun
    let resp_stop_tun = harness
        .execute_request(harness.auth_token.secret(), ServiceCommand::StopTun)
        .await
        .unwrap();
    assert!(resp_stop_tun.success);
    assert_eq!(
        resp_stop_tun.payload,
        Some(ServiceResponsePayload::TunStopped)
    );

    // 6. Set & Clear System Proxy
    let resp_proxy = harness
        .execute_request(
            harness.auth_token.secret(),
            ServiceCommand::SetSystemProxy {
                endpoint: "127.0.0.1:8080".to_string(),
                bypass: None,
            },
        )
        .await
        .unwrap();
    assert!(resp_proxy.success);
    assert_eq!(
        resp_proxy.payload,
        Some(ServiceResponsePayload::SystemProxyApplied)
    );

    let resp_clear = harness
        .execute_request(
            harness.auth_token.secret(),
            ServiceCommand::ClearSystemProxy,
        )
        .await
        .unwrap();
    assert!(resp_clear.success);
    assert_eq!(
        resp_clear.payload,
        Some(ServiceResponsePayload::SystemProxyCleared)
    );
}
