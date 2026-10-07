//! Behavior cases for default.
//! test-intent: behavior

use super::*;

#[test]
fn test_default_service_command_handler() {
    let handler = DefaultServiceCommandHandler::new(PrivilegeLevel::CapNetAdmin);
    assert!(!handler.is_tun_active());
    assert!(!handler.is_system_proxy_active());

    // Ping
    let pong = handler
        .handle_command(ServiceCommand::Ping { nonce: 123 })
        .unwrap();
    assert_eq!(pong, ServiceResponsePayload::Pong { nonce: 123 });

    // Status initial
    let status_payload = handler.handle_command(ServiceCommand::QueryStatus).unwrap();
    if let ServiceResponsePayload::Status(status) = status_payload {
        assert_eq!(status.state, ServiceState::Running);
        assert_eq!(status.privilege_level, PrivilegeLevel::CapNetAdmin);
        assert!(!status.tun_active);
        assert!(!status.system_proxy_active);
    } else {
        panic!("Expected Status payload");
    }

    // Start Tun
    let tun_resp = handler
        .handle_command(ServiceCommand::StartTun {
            tun_interface: Some("custom-tun".to_string()),
            config_path: None,
        })
        .unwrap();
    assert_eq!(
        tun_resp,
        ServiceResponsePayload::TunStarted {
            interface_name: Some("custom-tun".to_string())
        }
    );
    assert!(handler.is_tun_active());

    // Set System Proxy
    let proxy_resp = handler
        .handle_command(ServiceCommand::SetSystemProxy {
            endpoint: "127.0.0.1:7890".to_string(),
            bypass: None,
        })
        .unwrap();
    assert_eq!(proxy_resp, ServiceResponsePayload::SystemProxyApplied);
    assert!(handler.is_system_proxy_active());

    // Clear System Proxy
    let clear_resp = handler
        .handle_command(ServiceCommand::ClearSystemProxy)
        .unwrap();
    assert_eq!(clear_resp, ServiceResponsePayload::SystemProxyCleared);
    assert!(!handler.is_system_proxy_active());

    // Stop Tun
    let stop_resp = handler.handle_command(ServiceCommand::StopTun).unwrap();
    assert_eq!(stop_resp, ServiceResponsePayload::TunStopped);
    assert!(!handler.is_tun_active());
}
