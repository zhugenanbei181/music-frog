//! Behaviour of the isolated service-mode port fixture.

use infiltrator_contract::service_mode::{ServiceModePlatform, ServiceModeState};
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_test_support::service_mode_fixtures::ServiceModeCapturePort;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;

#[tokio::test]
async fn the_privilege_state_is_published_verbatim() {
    let port =
        ServiceModeCapturePort::new(ServiceModePlatform::LinuxPolkit, ServiceModeState::Ready);
    let snapshot = port.snapshot().await.expect("service mode snapshot");

    assert_eq!(snapshot.platform, ServiceModePlatform::LinuxPolkit);
    assert_eq!(snapshot.state, ServiceModeState::Ready);
}

#[tokio::test]
async fn prepare_installs_the_service_and_reports_the_new_state() {
    let port = ServiceModeCapturePort::new(
        ServiceModePlatform::LinuxPolkit,
        ServiceModeState::NotInstalled,
    );

    let prepared = port.prepare().await.expect("prepared service mode");

    assert_eq!(prepared.state, ServiceModeState::InstalledStopped);
    assert_eq!(
        port.current_state(),
        ServiceModeState::InstalledStopped,
        "the published state must follow the preparation"
    );
}

#[tokio::test]
async fn a_host_without_the_privilege_refuses_to_prepare() {
    let port = ServiceModeCapturePort::new(
        ServiceModePlatform::LinuxPolkit,
        ServiceModeState::MissingPrivilege,
    );

    let error = port.prepare().await.expect_err("the preparation must fail");

    assert!(matches!(error, PortError::PermissionDenied(_)));
    assert_eq!(port.current_state(), ServiceModeState::MissingPrivilege);
}

#[tokio::test]
async fn a_host_without_service_mode_support_is_typed_unsupported() {
    let port = ServiceModeCapturePort::new(
        ServiceModePlatform::Unsupported,
        ServiceModeState::Unsupported,
    );

    let error = port.prepare().await.expect_err("the preparation must fail");

    match error {
        PortError::Unsupported { capability, .. } => {
            assert_eq!(capability, Capability::CoreLifecycle);
        }
        other => panic!("expected typed unsupported, got {other:?}"),
    }
}
