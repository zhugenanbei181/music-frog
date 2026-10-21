//! Behaviour of the isolated privileged-network port fixture.

use infiltrator_contract::privileged_network::{
    PrivilegedNetworkOperation, PrivilegedNetworkRequest, PrivilegedNetworkState,
};
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_test_support::privileged_network_fixtures::PrivilegedNetworkCapturePort;
use lurker_core_ports::error::PortError;

fn request() -> PrivilegedNetworkRequest {
    PrivilegedNetworkRequest {
        operations: vec![
            PrivilegedNetworkOperation::TunService,
            PrivilegedNetworkOperation::SystemProxy,
        ],
    }
}

#[tokio::test]
async fn an_injection_publishes_an_active_snapshot() {
    let port = PrivilegedNetworkCapturePort::new();

    let snapshot = port.inject(request()).await.expect("injection");

    assert_eq!(snapshot.state, PrivilegedNetworkState::Active);
    assert!(snapshot.injected);
    assert_eq!(snapshot.operation_count, 2);
}

#[tokio::test]
async fn a_cleanup_publishes_a_clean_snapshot() {
    let port = PrivilegedNetworkCapturePort::new();
    port.inject(request()).await.expect("injection");

    let snapshot = port.cleanup().await.expect("cleanup");

    assert_eq!(snapshot.state, PrivilegedNetworkState::Cleaned);
    assert!(snapshot.cleanup_attempted);
    assert!(snapshot.rollback_attempted);
    assert!(!snapshot.injected);
    assert!(snapshot.is_clean());
}

#[tokio::test]
async fn a_refused_injection_is_a_typed_failure() {
    let port = PrivilegedNetworkCapturePort::new();
    port.fail_injection("the host refused the privileged operation");

    let error = port
        .inject(request())
        .await
        .expect_err("the injection must fail");

    assert!(matches!(error, PortError::PermissionDenied(_)));
    let snapshot = port.snapshot().await.expect("snapshot");
    assert!(!snapshot.injected);
}

#[tokio::test]
async fn a_refused_cleanup_is_a_typed_failure() {
    let port = PrivilegedNetworkCapturePort::new();
    port.inject(request()).await.expect("injection");
    port.fail_cleanup("the host refused the rollback");

    let error = port.cleanup().await.expect_err("the cleanup must fail");

    assert!(matches!(error, PortError::PermissionDenied(_)));
    let snapshot = port.snapshot().await.expect("snapshot");
    assert!(snapshot.injected);
}
