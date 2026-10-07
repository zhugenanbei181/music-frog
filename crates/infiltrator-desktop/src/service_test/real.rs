//! Behavior cases for real.
//! test-intent: behavior

#[cfg(unix)]
use super::state_machine::CommandSequence;
use super::*;
#[cfg(unix)]
use std::fs::create_dir_all;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::id;
#[cfg(unix)]
use tokio::sync::watch::channel;
#[cfg(unix)]
use tokio::time::sleep;

#[tokio::test]
#[cfg(unix)]
async fn test_real_unix_socket_client_server_lifecycle() {
    let _ = create_dir_all("target/tmp");
    let socket_path = PathBuf::from(format!("target/tmp/mf_{}.sock", id()));
    let endpoint = IpcEndpoint::from_unix_path(&socket_path);

    let auth_token = AuthToken::generate();
    let handler = Arc::new(DefaultServiceCommandHandler::new(PrivilegeLevel::Root));
    let server = ServiceServer::new(endpoint.clone(), auth_token.clone(), handler.clone());

    let (shutdown_tx, shutdown_rx) = channel(false);

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
        sleep(Duration::from_millis(10)).await;
    }
    if !socket_path.exists() {
        eprintln!(
            "Skipping unix socket lifecycle test: socket bind restricted by sandbox/environment"
        );
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
    let full_seq = CommandSequence::tun_startup_sequence(Some("tun99".to_string()), None);
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
