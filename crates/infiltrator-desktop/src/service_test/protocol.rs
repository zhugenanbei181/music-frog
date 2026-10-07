//! Behavior cases for protocol.
//! test-intent: behavior

use super::*;

#[test]
fn test_protocol_command_serialization() {
    let cmd_ping = ServiceCommand::Ping { nonce: 42 };
    let json_ping = serde_json::to_string(&cmd_ping).unwrap();
    assert!(json_ping.contains("\"action\":\"ping\""));
    let de_ping: ServiceCommand = serde_json::from_str(&json_ping).unwrap();
    assert_eq!(cmd_ping, de_ping);

    let cmd_start_tun = ServiceCommand::StartTun {
        tun_interface: Some("tun0".to_string()),
        config_path: Some("/etc/mihomo/config.yaml".to_string()),
    };
    let json_tun = serde_json::to_string(&cmd_start_tun).unwrap();
    assert!(json_tun.contains("\"action\":\"start_tun\""));
    let de_tun: ServiceCommand = serde_json::from_str(&json_tun).unwrap();
    assert_eq!(cmd_start_tun, de_tun);

    let cmd_proxy = ServiceCommand::SetSystemProxy {
        endpoint: "127.0.0.1:7890".to_string(),
        bypass: Some("localhost;127.0.0.1".to_string()),
    };
    let json_proxy = serde_json::to_string(&cmd_proxy).unwrap();
    assert!(json_proxy.contains("\"action\":\"set_system_proxy\""));
    let de_proxy: ServiceCommand = serde_json::from_str(&json_proxy).unwrap();
    assert_eq!(cmd_proxy, de_proxy);

    let cmd_clear = ServiceCommand::ClearSystemProxy;
    let json_clear = serde_json::to_string(&cmd_clear).unwrap();
    assert_eq!(json_clear, "{\"action\":\"clear_system_proxy\"}");

    let cmd_stop = ServiceCommand::StopTun;
    let json_stop = serde_json::to_string(&cmd_stop).unwrap();
    assert_eq!(json_stop, "{\"action\":\"stop_tun\"}");

    let cmd_status = ServiceCommand::QueryStatus;
    let json_status = serde_json::to_string(&cmd_status).unwrap();
    assert_eq!(json_status, "{\"action\":\"query_status\"}");
}

#[test]
fn test_protocol_request_response_serialization() {
    let token = AuthToken::new("secret-token");
    let req = ServiceRequest::authed(&token, ServiceCommand::Ping { nonce: 99 });
    let json_req = serde_json::to_string(&req).unwrap();
    let de_req: ServiceRequest = serde_json::from_str(&json_req).unwrap();
    assert_eq!(req, de_req);

    let resp_ok = ServiceResponse::pong("req-1", 99);
    let json_resp = serde_json::to_string(&resp_ok).unwrap();
    let de_resp: ServiceResponse = serde_json::from_str(&json_resp).unwrap();
    assert_eq!(resp_ok, de_resp);

    let resp_err = ServiceResponse::err("req-2", "Something broke");
    assert!(!resp_err.success);
    assert_eq!(resp_err.error.as_deref(), Some("Something broke"));
}
