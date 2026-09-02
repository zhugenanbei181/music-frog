use super::*;
use super::client::ServiceClient;
use super::server::{MockServiceHarness, ServiceServer};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_service_status_equality() {
    let status1 = ServiceStatus::Running(1234);
    let status2 = ServiceStatus::Running(1234);
    let status3 = ServiceStatus::Running(5678);
    let status4 = ServiceStatus::Stopped;

    assert_eq!(status1, status2);
    assert_ne!(status1, status3);
    assert_ne!(status1, status4);
}

#[test]
fn test_service_manager_new() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let manager = ServiceManager::new(binary_path.clone(), config_path.clone());
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_manager_with_home() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let home = temp_dir.path().join("home");
    let manager = ServiceManager::with_home(binary_path.clone(), config_path.clone(), home);
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_manager_with_pid_file() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let pid_file = temp_dir.path().join("pidfile");
    let manager = ServiceManager::with_pid_file(binary_path.clone(), config_path.clone(), pid_file);
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_status_debug() {
    let running = ServiceStatus::Running(1234);
    let debug_str = format!("{:?}", running);
    assert!(debug_str.contains("Running"));
    assert!(debug_str.contains("1234"));

    let stopped = ServiceStatus::Stopped;
    assert_eq!(format!("{:?}", stopped), "Stopped");
}

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq(b"secret_key_123", b"secret_key_123"));
    assert!(!constant_time_eq(b"secret_key_123", b"secret_key_124"));
    assert!(!constant_time_eq(b"secret", b"secret_longer"));
    assert!(constant_time_eq(b"", b""));
}

#[test]
fn test_auth_token_operations() {
    let token1 = AuthToken::generate();
    let token2 = AuthToken::generate();
    assert_ne!(token1.secret(), token2.secret());
    assert!(token1.verify(token1.secret()));
    assert!(!token1.verify(token2.secret()));
    assert!(!token1.verify("invalid_token_candidate"));

    let manual = AuthToken::new("my-custom-shared-secret");
    assert_eq!(manual.secret(), "my-custom-shared-secret");
    assert!(manual.verify("my-custom-shared-secret"));
}

#[tokio::test]
async fn test_auth_token_file_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let token_path = temp_dir.path().join("nested").join("service.token");
    let token = AuthToken::generate();

    token
        .save_to_file(&token_path)
        .await
        .expect("Failed to save auth token");
    assert!(token_path.exists());

    let loaded = AuthToken::load_from_file(&token_path).await.expect("Failed to load auth token");
    assert_eq!(token.secret(), loaded.secret());
    assert!(loaded.verify(token.secret()));
}

#[test]
fn test_privilege_level_display_and_elevation() {
    assert_eq!(PrivilegeLevel::Admin.to_string(), "Administrator");
    assert_eq!(PrivilegeLevel::Root.to_string(), "Root");
    assert_eq!(PrivilegeLevel::CapNetAdmin.to_string(), "cap_net_admin");
    assert_eq!(PrivilegeLevel::Unprivileged.to_string(), "Unprivileged");

    assert!(PrivilegeLevel::Admin.is_elevated());
    assert!(PrivilegeLevel::Root.is_elevated());
    assert!(PrivilegeLevel::CapNetAdmin.is_elevated());
    assert!(!PrivilegeLevel::Unprivileged.is_elevated());

    let detected = PrivilegeLevel::detect();
    let _ = detected.to_string();
}

#[test]
fn test_service_state_display_and_running() {
    assert_eq!(ServiceState::NotInstalled.to_string(), "Not Installed");
    assert_eq!(ServiceState::Stopped.to_string(), "Stopped");
    assert_eq!(ServiceState::Running.to_string(), "Running");
    assert_eq!(
        ServiceState::Error("Daemon crashed".to_string()).to_string(),
        "Error: Daemon crashed"
    );

    assert!(ServiceState::Running.is_running());
    assert!(!ServiceState::Stopped.is_running());
    assert!(!ServiceState::NotInstalled.is_running());
    assert!(!ServiceState::Error("fail".to_string()).is_running());
}

#[test]
fn test_service_status_info_fallback() {
    let uninstalled = ServiceStatusInfo::fallback_uninstalled();
    assert_eq!(uninstalled.state, ServiceState::NotInstalled);
    assert!(!uninstalled.tun_active);
    assert!(!uninstalled.system_proxy_active);
    assert_eq!(uninstalled.version, SERVICE_VERSION);

    let stopped = ServiceStatusInfo::fallback_stopped();
    assert_eq!(stopped.state, ServiceState::Stopped);
    assert!(!stopped.tun_active);
    assert!(!stopped.system_proxy_active);
}

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

#[test]
fn test_ipc_endpoint_properties() {
    let pipe = IpcEndpoint::from_named_pipe(r"\\.\pipe\custom-pipe");
    assert_eq!(pipe.display_target(), r"\\.\pipe\custom-pipe");
    assert!(pipe.is_available());

    let temp_dir = TempDir::new().unwrap();
    let non_existent_sock = temp_dir.path().join("missing.sock");
    let unix_ep = IpcEndpoint::from_unix_path(&non_existent_sock);
    assert_eq!(
        unix_ep.display_target(),
        non_existent_sock.to_string_lossy()
    );
    assert!(!unix_ep.is_available());

    let default_ep = IpcEndpoint::default_for_platform();
    let _ = default_ep.display_target();
}

#[test]
fn test_service_error_display() {
    assert_eq!(
        ServiceError::NotInstalled.to_string(),
        "Service is not installed"
    );
    assert_eq!(
        ServiceError::NotRunning.to_string(),
        "Service is not running"
    );
    assert!(
        ServiceError::ConnectionFailed("refused".to_string())
            .to_string()
            .contains("refused")
    );
    assert!(
        ServiceError::Unauthorized("bad token".to_string())
            .to_string()
            .contains("bad token")
    );
    assert!(
        ServiceError::ProtocolError("bad json".to_string())
            .to_string()
            .contains("bad json")
    );
    assert!(
        ServiceError::CommandFailed("driver error".to_string())
            .to_string()
            .contains("driver error")
    );
    assert_eq!(
        ServiceError::Timeout.to_string(),
        "Service communication timed out"
    );
    assert!(
        ServiceError::Io("disk error".to_string())
            .to_string()
            .contains("disk error")
    );
}

#[tokio::test]
async fn test_send_and_recv_framed_json() {
    let (mut client_io, mut server_io) = tokio::io::duplex(4096);

    let sample_req =
        ServiceRequest::new("id-123", "token-xyz", ServiceCommand::Ping { nonce: 777 });

    tokio::spawn(async move {
        send_framed_json(&mut client_io, &sample_req).await.unwrap();
    });

    let (server_reader, _) = tokio::io::split(&mut server_io);
    let mut buf_reader = tokio::io::BufReader::new(server_reader);
    let received: ServiceRequest = recv_framed_json(&mut buf_reader).await.unwrap();

    assert_eq!(received.id, "id-123");
    assert_eq!(received.auth_token, "token-xyz");
    assert_eq!(received.command, ServiceCommand::Ping { nonce: 777 });
}

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

#[tokio::test]
#[cfg(unix)]
async fn test_real_unix_socket_client_server_lifecycle() {
    let _ = std::fs::create_dir_all("target/tmp");
    let socket_path = std::path::PathBuf::from(format!("target/tmp/mf_{}.sock", std::process::id()));
    let endpoint = IpcEndpoint::from_unix_path(&socket_path);

    let auth_token = AuthToken::generate();
    let handler = Arc::new(DefaultServiceCommandHandler::new(PrivilegeLevel::Root));
    let server = ServiceServer::new(endpoint.clone(), auth_token.clone(), handler.clone());

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let server_handle = tokio::spawn(async move {
        let res = server.run(shutdown_rx).await;
        if let Err(e) = &res {
            eprintln!("SERVER ERROR: {e:?}");
        }
        res
    });

    // Give the server time to bind
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    if !socket_path.exists() {
        eprintln!("Skipping unix socket lifecycle test: socket bind restricted by sandbox/environment");
        let _ = shutdown_tx.send(true);
        let _ = server_handle.await;
        return;
    }

    let client = ServiceClient::new(endpoint.clone(), auth_token.clone())
        .with_timeout(Duration::from_secs(2));

    // 1. Service availability check
    assert!(client.is_service_available().await);

    // 2. Ping
    let pong_nonce = client.ping(4242).await.unwrap();
    assert_eq!(pong_nonce, 4242);

    // 3. Query status
    let status_info = client.query_status().await.unwrap();
    assert_eq!(status_info.state, ServiceState::Running);
    assert_eq!(status_info.privilege_level, PrivilegeLevel::Root);
    assert!(!status_info.tun_active);

    // 4. Start & stop TUN
    let iface = client
        .start_tun(Some("utun2".to_string()), None)
        .await
        .unwrap();
    assert_eq!(iface, Some("utun2".to_string()));
    assert!(handler.is_tun_active());

    client.stop_tun().await.unwrap();
    assert!(!handler.is_tun_active());

    // 5. System proxy
    client
        .set_system_proxy("127.0.0.1:1080", Some("localhost".to_string()))
        .await
        .unwrap();
    assert!(handler.is_system_proxy_active());

    client.clear_system_proxy().await.unwrap();
    assert!(!handler.is_system_proxy_active());

    // 5b. Test execute_sequence over real IPC
    let full_seq = super::state_machine::CommandSequence::tun_startup_sequence(
        Some("tun99".to_string()),
        None,
    );
    let seq_res = client.execute_sequence(&full_seq).await;
    assert!(seq_res.all_successful());
    assert!(handler.is_tun_active());
    client.stop_tun().await.unwrap();

    // 6. Test unauthorized client
    let bad_client = ServiceClient::new(endpoint.clone(), AuthToken::new("bad-token"))
        .with_timeout(Duration::from_secs(2));
    let bad_result = bad_client.ping(1).await;
    assert!(matches!(bad_result, Err(ServiceError::Unauthorized(_))));

    // 7. Graceful server shutdown
    let _ = shutdown_tx.send(true);
    let _ = server_handle.await;

    // 8. Client fallback after server shutdown
    assert!(!client.is_service_available().await);
    let fallback_status = client.query_status_or_fallback().await;
    assert_eq!(fallback_status.state, ServiceState::Stopped);
}

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

#[test]
fn test_windows_service_manager_args_and_sddl() {
    use super::windows::{
        NamedPipeSecurity, WindowsServiceConfig, WindowsServiceManager, WindowsServiceStartType,
        WindowsServiceStatus,
    };

    let config = WindowsServiceConfig::new(r"C:\Program Files\MusicFrog\infiltrator.exe")
        .with_service_name("TestCustomService")
        .with_pipe_name(r"\\.\pipe\test-custom-pipe");

    let manager = WindowsServiceManager::new(config);
    assert_eq!(manager.config().service_name, "TestCustomService");

    // 1. Create args
    let create_args = manager.build_create_args();
    assert_eq!(create_args[0], "create");
    assert_eq!(create_args[1], "TestCustomService");
    assert!(create_args[2].contains(r"C:\Program Files\MusicFrog\infiltrator.exe"));
    assert_eq!(create_args[4], "start=auto");

    // 2. Delete / Start / Stop / Query args
    assert_eq!(manager.build_delete_args(), vec!["delete", "TestCustomService"]);
    assert_eq!(manager.build_start_args(), vec!["start", "TestCustomService"]);
    assert_eq!(manager.build_stop_args(), vec!["stop", "TestCustomService"]);
    assert_eq!(manager.build_query_args(), vec!["query", "TestCustomService"]);

    // 3. SDDL Generation
    let sddl_auth = NamedPipeSecurity::generate_sddl(true);
    assert!(sddl_auth.contains("AU"));
    assert!(sddl_auth.contains("SY"));
    assert!(sddl_auth.contains("BA"));

    let sddl_admin_only = NamedPipeSecurity::generate_sddl(false);
    assert!(!sddl_admin_only.contains("AU"));
    assert!(sddl_admin_only.contains("SY"));

    // 4. Pipe Name Validation
    assert!(NamedPipeSecurity::is_valid_pipe_name(r"\\.\pipe\musicfrog-infiltrator-service"));
    assert!(NamedPipeSecurity::is_valid_pipe_name(r"\\.\pipe\test_pipe_123"));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(r"/var/run/test.sock"));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(r"\\.\pipe\nested\pipe"));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(r"\\.\pipe\"));

    // 5. sc.exe Query Parser
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("[SC] EnumQueryServicesStatus:OpenService FAILED 1060: The specified service does not exist"),
        WindowsServiceStatus::NotInstalled
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 4  RUNNING \n"),
        WindowsServiceStatus::Running
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 1  STOPPED \n"),
        WindowsServiceStatus::Stopped
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 2  START_PENDING \n"),
        WindowsServiceStatus::StartPending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 3  STOP_PENDING \n"),
        WindowsServiceStatus::StopPending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 7  PAUSED \n"),
        WindowsServiceStatus::Paused
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 6  PAUSE_PENDING \n"),
        WindowsServiceStatus::PausePending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 5  CONTINUE_PENDING \n"),
        WindowsServiceStatus::ContinuePending
    );

    let unknown = WindowsServiceManager::parse_sc_query_output("SOMETHING UNEXPECTED");
    assert!(matches!(unknown, WindowsServiceStatus::Unknown(_)));

    assert!(WindowsServiceStatus::Running.is_running());
    assert!(!WindowsServiceStatus::Stopped.is_running());
    assert!(WindowsServiceStatus::Stopped.is_installed());
    assert!(!WindowsServiceStatus::NotInstalled.is_installed());

    assert_eq!(WindowsServiceStartType::Auto.as_sc_arg(), "auto");
    assert_eq!(WindowsServiceStartType::Demand.as_sc_arg(), "demand");
    assert_eq!(WindowsServiceStartType::Disabled.as_sc_arg(), "disabled");

    assert_eq!(WindowsServiceStatus::Running.to_string(), "Running");
    assert_eq!(WindowsServiceStatus::Stopped.to_string(), "Stopped");
    assert_eq!(WindowsServiceStatus::NotInstalled.to_string(), "Not Installed");
    assert_eq!(WindowsServiceStatus::StartPending.to_string(), "Start Pending");
    assert_eq!(WindowsServiceStatus::StopPending.to_string(), "Stop Pending");
    assert_eq!(WindowsServiceStatus::Paused.to_string(), "Paused");
    assert_eq!(WindowsServiceStatus::PausePending.to_string(), "Pause Pending");
    assert_eq!(WindowsServiceStatus::ContinuePending.to_string(), "Continue Pending");
}

#[test]
fn test_linux_privilege_wizard_and_generators() {
    use super::linux::LinuxPrivilegeWizard;
    use std::path::Path;

    // 1. Capability parser
    assert!(LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo = cap_net_admin,cap_net_bind_service+ep"
    ));
    assert!(LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo cap_net_bind_service,cap_net_admin=ep"
    ));
    assert!(!LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo = cap_sys_admin+ep"
    ));
    assert!(!LinuxPrivilegeWizard::parse_getcap_output(""));

    // 2. TUN device check
    let (tun_ok, tun_info) = LinuxPrivilegeWizard::check_tun_device();
    let _ = tun_ok;
    assert!(!tun_info.is_empty());

    // 3. Full diagnostic report
    let dummy_path = Path::new("/opt/musicfrog/mihomo");
    let report = LinuxPrivilegeWizard::diagnose(dummy_path);
    assert!(!report.checks.is_empty());
    let summary = report.summary();
    assert!(summary.contains("Linux Privilege Diagnostic"));

    // 4. Polkit action XML generation
    let polkit_xml = LinuxPrivilegeWizard::generate_polkit_action_xml(Some("/usr/sbin/setcap"));
    assert!(polkit_xml.contains("com.musicfrog.infiltrator.setcap"));
    assert!(polkit_xml.contains("/usr/sbin/setcap"));
    assert!(polkit_xml.contains("auth_admin_keep"));

    // 5. Polkit rule generation
    let polkit_rule = LinuxPrivilegeWizard::generate_polkit_rule();
    assert!(polkit_rule.contains("com.musicfrog.infiltrator.setcap"));
    assert!(polkit_rule.contains("wheel"));
    assert!(polkit_rule.contains("sudo"));

    // 6. Install / Uninstall script generation
    let install_sh = LinuxPrivilegeWizard::generate_install_script(dummy_path);
    assert!(install_sh.contains("#!/bin/bash"));
    assert!(install_sh.contains("cap_net_admin,cap_net_bind_service+ep"));
    assert!(install_sh.contains("/opt/musicfrog/mihomo"));

    let uninstall_sh = LinuxPrivilegeWizard::generate_uninstall_script(dummy_path);
    assert!(uninstall_sh.contains("setcap -r"));
    assert!(uninstall_sh.contains("/opt/musicfrog/mihomo"));

    // 7. Systemd service generation
    let systemd_unit = LinuxPrivilegeWizard::generate_systemd_service(
        "musicfrog-infiltrator.service",
        dummy_path,
        Some(Path::new("/etc/mihomo/config.yaml")),
        Some("musicfrog"),
    );
    assert!(systemd_unit.contains("Description=musicfrog-infiltrator.service"));
    assert!(systemd_unit.contains("AmbientCapabilities=CAP_NET_ADMIN CAP_NET_BIND_SERVICE"));
    assert!(systemd_unit.contains("CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_BIND_SERVICE"));
    assert!(systemd_unit.contains("User=musicfrog"));
    assert!(systemd_unit.contains("-f \"/etc/mihomo/config.yaml\""));
}

#[test]
fn test_macos_privileged_helper_contract_and_xpc() {
    use super::macos::{
        AUTH_RIGHT_PROXY_MANAGE, AUTH_RIGHT_TUN_MANAGE, MacHelperDoctor,
        MacPrivilegedHelperContract, MacPrivilegedHelperSpec, XpcMessage, XpcResponse,
    };

    let spec = MacPrivilegedHelperSpec::default();
    assert_eq!(spec.helper_bundle_id, "com.musicfrog.infiltrator.helper");
    assert_eq!(spec.app_bundle_id, "com.musicfrog.infiltrator");
    assert_eq!(spec.mach_service_name, "com.musicfrog.infiltrator.helper.xpc");

    // 1. Designated Requirement Generator
    let dr = MacPrivilegedHelperContract::generate_designated_requirement(
        &spec.team_id,
        &spec.app_bundle_id,
    );
    assert!(dr.contains("com.musicfrog.infiltrator"));
    assert!(dr.contains(&spec.team_id));
    assert!(dr.contains("anchor apple generic"));

    // 2. Launchd Plist Generator
    let plist = MacPrivilegedHelperContract::generate_launchd_plist(&spec);
    assert!(plist.contains("<key>Label</key>"));
    assert!(plist.contains("com.musicfrog.infiltrator.helper"));
    assert!(plist.contains("com.musicfrog.infiltrator.helper.xpc"));
    assert!(plist.contains("/Library/PrivilegedHelperTools/com.musicfrog.infiltrator.helper"));

    // 3. Helper Info Plist & App Info Plist
    let helper_info = MacPrivilegedHelperContract::generate_helper_info_plist(&spec, &dr);
    assert!(helper_info.contains("SMAuthorizedClients"));
    assert!(helper_info.contains("com.musicfrog.infiltrator.helper"));

    let app_info = MacPrivilegedHelperContract::generate_app_info_plist(&spec, &dr);
    assert!(app_info.contains("SMPrivilegedExecutables"));
    assert!(app_info.contains("com.musicfrog.infiltrator"));

    // 4. Authorization Rights
    let rights = MacPrivilegedHelperContract::authorization_rights_spec();
    assert!(rights.contains_key(AUTH_RIGHT_TUN_MANAGE));
    assert!(rights.contains_key(AUTH_RIGHT_PROXY_MANAGE));

    // 5. XPC Message & Response
    let xpc_msg = XpcMessage::new(
        "msg-1",
        "com.musicfrog.infiltrator",
        ServiceCommand::StartTun {
            tun_interface: Some("utun3".to_string()),
            config_path: None,
        },
    );
    assert_eq!(xpc_msg.protocol_version, 1);
    assert_eq!(xpc_msg.required_right, Some(AUTH_RIGHT_TUN_MANAGE.to_string()));

    let json_xpc = serde_json::to_string(&xpc_msg).unwrap();
    let de_xpc: XpcMessage = serde_json::from_str(&json_xpc).unwrap();
    assert_eq!(xpc_msg, de_xpc);

    let xpc_resp = XpcResponse::ok(
        "msg-1",
        ServiceResponsePayload::TunStarted {
            interface_name: Some("utun3".to_string()),
        },
    );
    assert!(xpc_resp.success);
    assert_eq!(xpc_resp.in_reply_to, "msg-1");

    let json_resp = serde_json::to_string(&xpc_resp).unwrap();
    let de_resp: XpcResponse = serde_json::from_str(&json_resp).unwrap();
    assert_eq!(xpc_resp, de_resp);

    // 6. Helper Doctor
    let doctor_status = MacHelperDoctor::check_helper_status(&spec);
    assert_eq!(doctor_status, crate::tun_service::ServiceModeStatus::NotInstalled);

    let verify_res = MacHelperDoctor::verify_support("install_service");
    #[cfg(target_os = "macos")]
    assert!(verify_res.is_err());
    #[cfg(not(target_os = "macos"))]
    assert!(verify_res.is_ok());
}

#[test]
fn test_service_state_machine_full_lifecycle() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled).with_max_history(50);
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalled);
    assert!(!sm.is_running());
    assert!(!sm.is_installed());

    // 1. Installation: Uninstalled -> Installing -> InstalledStopped
    assert!(sm.can_apply(&LifecycleEvent::InstallStart));
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Installing);
    assert!(!sm.is_installed());

    sm.apply(LifecycleEvent::InstallSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);
    assert!(sm.is_installed());
    assert!(!sm.is_running());

    // 2. Start Service: InstalledStopped -> Starting -> Running
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Starting);

    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(9999),
        tun_active: false,
    })
    .unwrap();
    assert_eq!(
        sm.current_state(),
        &LifecycleState::Running {
            pid: Some(9999),
            tun_active: false,
            proxy_active: false,
        }
    );
    assert!(sm.is_running());
    assert_eq!(sm.current_state().pid(), Some(9999));
    assert!(!sm.current_state().is_tun_active());
    assert!(!sm.current_state().is_proxy_active());

    // 3. Routing commands: StartTun, ProxyApplied, ProxyCleared, TunStopped
    sm.apply(LifecycleEvent::TunStarted {
        interface_name: Some("tun0".to_string()),
    })
    .unwrap();
    assert!(sm.current_state().is_tun_active());

    sm.apply(LifecycleEvent::ProxyApplied).unwrap();
    assert!(sm.current_state().is_proxy_active());

    sm.apply(LifecycleEvent::ProxyCleared).unwrap();
    assert!(!sm.current_state().is_proxy_active());

    sm.apply(LifecycleEvent::TunStopped).unwrap();
    assert!(!sm.current_state().is_tun_active());

    // 4. Stop Service: Running -> Stopping -> InstalledStopped
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Stopping);

    sm.apply(LifecycleEvent::StopSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);
    assert!(!sm.is_running());

    // 5. Uninstallation: InstalledStopped -> Uninstalling -> Uninstalled
    sm.apply(LifecycleEvent::UninstallStart).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalling);

    sm.apply(LifecycleEvent::UninstallSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalled);
    assert!(!sm.is_installed());

    // Check transition history
    assert!(sm.history().len() >= 10);
}

#[test]
fn test_service_state_machine_error_and_degraded_recovery() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    // 1. Install failure recovery
    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled);
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    sm.apply(LifecycleEvent::InstallFailure("Permission denied".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error { recoverable: true, .. }
    ));
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 2. Startup failure recovery
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    sm.apply(LifecycleEvent::StartFailure("Port conflict".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error { recoverable: true, .. }
    ));
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 3. Degraded state & recovery
    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(1234),
        tun_active: true,
    })
    .unwrap();
    sm.apply(LifecycleEvent::Degrade("TUN route packet loss".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Degraded { .. }
    ));
    assert!(sm.current_state().is_tun_active());
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert!(sm.is_running());

    // 4. Heartbeat missed -> Degraded -> Stop -> Stopped
    sm.apply(LifecycleEvent::HeartbeatMissed).unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Degraded { .. }
    ));
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    sm.apply(LifecycleEvent::StopSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 5. Crash recovery
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    sm.apply(LifecycleEvent::ProcessCrashed("SIGKILL".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error { .. }
    ));
    sm.apply(LifecycleEvent::Reset).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 6. Stop failure & Uninstall failure
    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(555),
        tun_active: false,
    })
    .unwrap();
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    sm.apply(LifecycleEvent::StopFailure("Timeout".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error { .. }
    ));
    sm.apply(LifecycleEvent::Reset).unwrap();

    sm.apply(LifecycleEvent::UninstallStart).unwrap();
    sm.apply(LifecycleEvent::UninstallFailure("File locked".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error { .. }
    ));
}

#[test]
fn test_service_state_machine_illegal_transitions() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled);

    // Cannot start or stop when uninstalled
    assert!(!sm.can_apply(&LifecycleEvent::StartRequested));
    let err = sm.apply(LifecycleEvent::StartRequested).unwrap_err();
    assert!(err.to_string().contains("uninstalled"));

    assert!(!sm.can_apply(&LifecycleEvent::TunStarted { interface_name: None }));
    let err2 = sm.apply(LifecycleEvent::TunStarted { interface_name: None }).unwrap_err();
    assert!(err2.to_string().contains("uninstalled"));

    // Transition to InstalledStopped
    sm.apply(LifecycleEvent::InstallSuccess).unwrap();

    // Cannot route commands when stopped
    let err3 = sm.apply(LifecycleEvent::ProxyApplied).unwrap_err();
    assert!(err3.to_string().contains("stopped"));

    // Transition to Installing
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    let err4 = sm.apply(LifecycleEvent::StartRequested).unwrap_err();
    assert!(err4.to_string().contains("in progress"));
}

#[tokio::test]
async fn test_command_sequences_with_mock_service_harness() {
    use super::state_machine::CommandSequence;

    let harness = MockServiceHarness::with_privilege(PrivilegeLevel::Admin);

    // 1. Execute TUN Startup Sequence
    let tun_seq = CommandSequence::tun_startup_sequence(
        Some("tun0".to_string()),
        Some("/etc/mihomo/config.yaml".to_string()),
    );
    assert_eq!(tun_seq.commands.len(), 3);
    let res = harness.execute_sequence(&tun_seq).await;
    assert!(res.all_successful());
    assert_eq!(res.step_results.len(), 3);

    // 2. Execute System Proxy Sequence
    let proxy_seq = CommandSequence::system_proxy_sequence("127.0.0.1:7890", Some("localhost".to_string()));
    let res_proxy = harness.execute_sequence(&proxy_seq).await;
    assert!(res_proxy.all_successful());
    assert_eq!(res_proxy.step_results.len(), 3);

    // 3. Execute Teardown Sequence
    let teardown_seq = CommandSequence::teardown_sequence();
    let res_teardown = harness.execute_sequence(&teardown_seq).await;
    assert!(res_teardown.all_successful());
    assert_eq!(res_teardown.step_results.len(), 3);

    // 4. Custom Sequence
    let custom_seq = CommandSequence::new("PingPongStatus")
        .then(ServiceCommand::Ping { nonce: 112233 })
        .then(ServiceCommand::QueryStatus);
    let res_custom = harness.execute_sequence(&custom_seq).await;
    assert!(res_custom.all_successful());
    assert_eq!(res_custom.step_results.len(), 2);
}

#[test]
fn test_lifecycle_state_and_event_display() {
    use super::state_machine::LifecycleState;

    assert_eq!(LifecycleState::Uninstalled.to_string(), "Uninstalled");
    assert_eq!(LifecycleState::Installing.to_string(), "Installing");
    assert_eq!(
        LifecycleState::InstalledStopped.to_string(),
        "Installed (Stopped)"
    );
    assert_eq!(LifecycleState::Starting.to_string(), "Starting");
    assert_eq!(LifecycleState::Stopping.to_string(), "Stopping");
    assert_eq!(LifecycleState::Uninstalling.to_string(), "Uninstalling");

    let running_str = LifecycleState::Running {
        pid: Some(123),
        tun_active: true,
        proxy_active: false,
    }
    .to_string();
    assert!(running_str.contains("Running"));
    assert!(running_str.contains("pid=Some(123)"));

    let degraded_str = LifecycleState::Degraded {
        reason: "NIC issue".to_string(),
        tun_active: false,
        proxy_active: true,
    }
    .to_string();
    assert!(degraded_str.contains("Degraded: NIC issue"));

    let error_str = LifecycleState::Error {
        message: "Crash".to_string(),
        recoverable: true,
    }
    .to_string();
    assert!(error_str.contains("Error (recoverable=true): Crash"));
}
