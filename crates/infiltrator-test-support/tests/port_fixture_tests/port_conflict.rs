//! Behaviour of the isolated port-conflict port fixture.

use infiltrator_contract::port_conflict::{PortBinding, PortConflict};
use infiltrator_ports::port_conflict::PortConflictPort;
use infiltrator_test_support::port_conflict_fixtures::{
    CAPTURE_FREE_CONTROLLER_PORT, CAPTURE_TAKEN_MIXED_PORT, PortConflictCapturePort,
};
use lurker_core_ports::error::PortError;

#[tokio::test]
async fn the_snapshot_publishes_the_taken_binding_verbatim() {
    let port = PortConflictCapturePort::new();
    let snapshot = port.snapshot().await.expect("conflict snapshot");

    assert!(snapshot.has_conflicts());
    let mixed = snapshot
        .conflicts
        .iter()
        .find(|conflict| conflict.binding == PortBinding::MixedProxy)
        .expect("the mixed proxy binding");
    assert_eq!(mixed.port, CAPTURE_TAKEN_MIXED_PORT);
    assert!(!mixed.available);
    assert!(mixed.owner_pid.is_some());
    assert!(!mixed.can_release);

    let controller = snapshot
        .conflicts
        .iter()
        .find(|conflict| conflict.binding == PortBinding::Controller)
        .expect("the controller binding");
    assert_eq!(controller.port, CAPTURE_FREE_CONTROLLER_PORT);
    assert!(controller.available);
}

#[tokio::test]
async fn a_repair_refuses_to_release_a_foreign_owner() {
    let port = PortConflictCapturePort::new();

    let error = port.repair().await.expect_err("the repair must fail");

    assert!(matches!(error, PortError::PermissionDenied(_)));
    let snapshot = port.snapshot().await.expect("conflict snapshot");
    assert!(snapshot.has_conflicts());
}

#[tokio::test]
async fn a_releasable_conflict_is_released_and_the_revision_advances() {
    let port = PortConflictCapturePort::with_conflicts(vec![PortConflict {
        binding: PortBinding::MixedProxy,
        port: CAPTURE_TAKEN_MIXED_PORT,
        available: false,
        owner_pid: Some(4242),
        owner_name: Some("owned-core".to_owned()),
        can_release: true,
    }]);
    let before = port.snapshot().await.expect("conflict snapshot");

    let repaired = port.repair().await.expect("the repair must succeed");

    assert!(!repaired.has_conflicts());
    assert_eq!(repaired.revision, before.revision + 1);
    assert!(repaired.conflicts.iter().all(|conflict| conflict.available));
}
