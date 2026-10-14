use super::project_runtime_status;
use infiltrator_contract::android_control::{
    VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
    VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use infiltrator_contract::runtime_status::{
    ControllerApiObservation, CoreProcessObservation, ProxyEffectivenessObservation,
    ProxyReadinessBlocker, TunDataPlaneObservation, VpnAuthorizationObservation,
};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_contract::traffic_topology::{TrafficTopologySnapshot, TrafficTopologyStatus};
use infiltrator_contract::vpn::{VpnGeneration, VpnSurfaceObservation};

fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
    CoreSnapshot {
        identity: Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("active").expect("instance ID"),
            None,
        )),
        lifecycle,
        generation: 8,
        session_token: Some(SessionToken::new(18)),
        revision: 11,
        routing_intent: RoutingIntentSnapshot::demo_fixture(),
        core_version: Some("1.19.32".to_owned()),
        sampled_at_epoch_ms: Some(100),
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog: Default::default(),
    }
}

fn owner(ready: bool) -> VpnOwnerSnapshot {
    let generation = VpnGeneration::new(4);
    let resource_state = |kind| {
        if ready && kind != VpnOwnerResourceKind::PreparedConfiguration {
            VpnOwnerResourceState::Held {
                generation: generation.value(),
            }
        } else {
            VpnOwnerResourceState::Absent
        }
    };
    VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 12,
        vpn_generation: generation,
        vpn_state: if ready {
            VpnOwnerLifecycle::Running
        } else {
            VpnOwnerLifecycle::Stopped
        },
        readiness: if ready {
            VpnOwnerReadiness::Ready
        } else {
            VpnOwnerReadiness::Stopped
        },
        resources: [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: resource_state(kind),
        })
        .collect(),
        foreground: ready,
        mtu: Some(1500),
        route_count: 1,
        dns_servers: Vec::new(),
        ipv6: false,
        core: VpnOwnerCoreReadback::Available(core(CoreLifecycle::Running).lifecycle_snapshot()),
    }
}

fn topology(status: TrafficTopologyStatus, active_connections: u32) -> TrafficTopologySnapshot {
    TrafficTopologySnapshot {
        generation: 8,
        revision: 11,
        status,
        failure: None,
        nodes: Vec::new(),
        links: Vec::new(),
        active_connections,
        flow_bps: 0.0,
        sniffer_enabled: None,
        jitter_histogram: None,
    }
}

fn projection(
    lifecycle: CoreLifecycle,
    authorization: VpnAuthorizationObservation,
    owner_ready: bool,
    topology: TrafficTopologySnapshot,
    origin: SurfaceOrigin,
) -> infiltrator_contract::runtime_status::RuntimeStatusSnapshot {
    let core = core(lifecycle);
    project_runtime_status(
        HostKind::Android,
        origin,
        &core,
        Ok(&core),
        &authorization,
        &VpnSurfaceObservation::from_android_owner(owner(owner_ready)),
        &topology,
    )
}

#[test]
fn running_control_service_and_core_do_not_claim_effective_proxy_without_tun() {
    let status = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Granted,
        false,
        topology(TrafficTopologyStatus::Empty, 0),
        SurfaceOrigin::Live,
    );

    assert!(matches!(
        status.control_service,
        infiltrator_contract::runtime_status::ControlServiceObservation::Available { .. }
    ));
    assert_eq!(status.core_process, CoreProcessObservation::Running);
    assert_eq!(
        status.controller_api,
        ControllerApiObservation::Connected {
            core_generation: 8,
            core_revision: 11,
        }
    );
    assert_eq!(status.tun_data_plane, TunDataPlaneObservation::Stopped);
    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::TunDataPlaneNotReady,
        }
    );
}

#[test]
fn ready_core_api_vpn_and_tun_without_flows_reports_no_active_traffic() {
    let status = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Granted,
        true,
        topology(TrafficTopologyStatus::Empty, 0),
        SurfaceOrigin::Live,
    );

    assert_eq!(
        status.tun_data_plane,
        TunDataPlaneObservation::Ready {
            generation: VpnGeneration::new(4),
        }
    );
    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::NoActiveTraffic
    );
}

#[test]
fn active_controller_flows_without_typed_route_attribution_stay_unverified() {
    let status = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Granted,
        true,
        topology(TrafficTopologyStatus::Unknown, 3),
        SurfaceOrigin::Live,
    );

    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::ActiveTrafficUnattributed {
            active_connections: 3,
        }
    );
}

#[test]
fn routing_intent_and_demo_fixtures_never_claim_live_proxy_effectiveness() {
    let status = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Granted,
        true,
        topology(TrafficTopologyStatus::Empty, 0),
        SurfaceOrigin::Demo,
    );
    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::DemoFixture
    );
}

#[test]
fn vpn_permission_required_blocks_proxy_effectiveness_even_when_other_axes_run() {
    let status = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Required,
        true,
        topology(TrafficTopologyStatus::Ready, 1),
        SurfaceOrigin::Live,
    );
    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::VpnAuthorizationRequired,
        }
    );
}

#[test]
fn vpn_authorization_failure_and_unsupported_are_not_reported_as_owner_failure() {
    let failure = Failure::new(ErrorCode::Permission, "permission read failed", true);
    let unavailable = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Unavailable {
            failure: failure.clone(),
        },
        true,
        topology(TrafficTopologyStatus::Ready, 1),
        SurfaceOrigin::Live,
    );
    let unsupported = projection(
        CoreLifecycle::Running,
        VpnAuthorizationObservation::Unsupported {
            reason: "host has no permission adapter".to_owned(),
        },
        true,
        topology(TrafficTopologyStatus::Ready, 1),
        SurfaceOrigin::Live,
    );

    assert_eq!(
        unavailable.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::VpnAuthorizationUnavailable,
        }
    );
    assert_eq!(
        unsupported.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::VpnAuthorizationUnsupported,
        }
    );
}

#[test]
fn api_failure_is_not_reinterpreted_as_connected_or_proxy_effective() {
    let core = core(CoreLifecycle::Running);
    let failure = Failure::new(ErrorCode::Network, "controller refused the request", true);
    let status = project_runtime_status(
        HostKind::Android,
        SurfaceOrigin::Live,
        &core,
        Err(&failure),
        &VpnAuthorizationObservation::Granted,
        &VpnSurfaceObservation::from_android_owner(owner(true)),
        &topology(TrafficTopologyStatus::Ready, 1),
    );

    assert!(matches!(
        status.controller_api,
        ControllerApiObservation::Unavailable { .. }
    ));
    assert_eq!(
        status.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::ControllerApiUnavailable,
        }
    );
}

#[test]
fn desktop_status_does_not_invent_android_service_or_vpn_facts() {
    let core = core(CoreLifecycle::Stopped);
    let status = project_runtime_status(
        HostKind::Desktop,
        SurfaceOrigin::Live,
        &core,
        Ok(&core),
        &VpnAuthorizationObservation::NotApplicable,
        &VpnSurfaceObservation::NotApplicable,
        &topology(TrafficTopologyStatus::Empty, 0),
    );

    assert_eq!(
        status.control_service,
        infiltrator_contract::runtime_status::ControlServiceObservation::NotApplicable
    );
    assert_eq!(
        status.tun_data_plane,
        TunDataPlaneObservation::NotApplicable
    );
    assert_eq!(status.core_process, CoreProcessObservation::Stopped);
}
