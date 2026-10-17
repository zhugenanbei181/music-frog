use super::*;
use infiltrator_contract::android_control::{
    VpnOwnerCoreReadback, VpnOwnerResourceFact, VpnOwnerResourceKind, VpnOwnerResourceState,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::vpn::VpnGeneration;

fn owner_snapshot(
    state: VpnOwnerLifecycle,
    readiness: VpnOwnerReadiness,
    resources: Vec<VpnOwnerResourceFact>,
) -> VpnOwnerSnapshot {
    VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 7,
        vpn_generation: VpnGeneration::new(4),
        foreground: matches!(
            &state,
            VpnOwnerLifecycle::Foreground
                | VpnOwnerLifecycle::TunnelEstablished
                | VpnOwnerLifecycle::Running
                | VpnOwnerLifecycle::Stopping
        ),
        vpn_state: state,
        readiness,
        resources,
        mtu: Some(1500),
        route_count: 1,
        dns_servers: vec!["1.1.1.1".to_owned()],
        ipv6: false,
        core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
            identity: None,
            lifecycle: CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            revision: 0,
            failure: None,
        }),
    }
}

#[test]
fn data_plane_start_requires_the_owner_tunnel_and_readiness_facts() {
    assert_eq!(
        vpn_tunnel_start_gate(&owner_snapshot(
            VpnOwnerLifecycle::TunnelEstablished,
            VpnOwnerReadiness::AwaitingDataPlane,
            Vec::new(),
        )),
        VpnTunnelStartGate::Established,
    );
    assert_eq!(
        vpn_tunnel_start_gate(&owner_snapshot(
            VpnOwnerLifecycle::Foreground,
            VpnOwnerReadiness::AwaitingTunnel,
            Vec::new(),
        )),
        VpnTunnelStartGate::AwaitingTunnel(VpnOwnerLifecycle::Foreground),
    );
    assert_eq!(
        vpn_tunnel_start_gate(&owner_snapshot(
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            Vec::new(),
        )),
        VpnTunnelStartGate::LifecycleConflict(VpnOwnerLifecycle::Running),
    );
}

#[test]
fn owner_projection_preserves_revision_generation_and_resource_metadata() {
    let snapshot = vpn_session_projection(&owner_snapshot(
        VpnOwnerLifecycle::Running,
        VpnOwnerReadiness::Ready,
        vec![VpnOwnerResourceFact {
            kind: VpnOwnerResourceKind::Worker,
            state: VpnOwnerResourceState::Held { generation: 4 },
        }],
    ));
    assert_eq!(snapshot.state, VpnSessionState::Running);
    assert!(snapshot.is_running());
    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.mtu, Some(1500));
    assert_eq!(snapshot.route_count, 1);
    assert_eq!(snapshot.dns_servers, vec!["1.1.1.1".to_owned()]);
}

#[test]
fn cleanup_pending_resource_cannot_project_as_stopped() {
    let snapshot = vpn_session_projection(&owner_snapshot(
        VpnOwnerLifecycle::Stopped,
        VpnOwnerReadiness::CleanupPending,
        vec![VpnOwnerResourceFact {
            kind: VpnOwnerResourceKind::TunnelDescriptor,
            state: VpnOwnerResourceState::CleanupPending {
                generation: 4,
                failure_message: "descriptor close failed".to_owned(),
            },
        }],
    ));
    let VpnSessionState::Failed { failure } = snapshot.state else {
        panic!("pending owner resources must remain visible as failed cleanup");
    };
    assert_eq!(failure.message, "descriptor close failed");
}

#[test]
fn owner_unavailable_stop_is_bounded_and_does_not_forge_a_snapshot() {
    let port = AndroidVpnServicePort::new();
    let error = port.owner_unavailable_stop("VpnService owner is unavailable");
    let PortError::Rejected(failure) = error else {
        panic!("owner unavailability must remain a typed stop failure");
    };
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.message.contains("VpnService owner is unavailable"));
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn non_android_host_returns_typed_vpn_unsupported() {
    let port = AndroidVpnServicePort::new();
    let failure = port
        .request_start()
        .await
        .expect_err("desktop test host must not emulate Android VpnService");
    assert!(matches!(
        failure,
        PortError::Unsupported {
            capability: Capability::VpnService,
            ..
        }
    ));
}
