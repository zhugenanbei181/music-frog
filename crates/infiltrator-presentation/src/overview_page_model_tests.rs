//! test-intent: behavior
use crate::overview_page_model::{
    OverviewHost, OverviewNetworkMode, OverviewNetworkStatus, OverviewNetworkWarning,
    OverviewPageModel, OverviewRoutingMode, OverviewRunState,
};
use infiltrator_contract::android_control::{
    VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
    VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::vpn::{VpnGeneration, VpnSurfaceObservation};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};

fn surface() -> SurfaceSnapshot {
    SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("surface unavailable"),
    )
}

fn vpn_owner(lifecycle: VpnOwnerLifecycle, readiness: VpnOwnerReadiness) -> VpnOwnerSnapshot {
    VpnOwnerSnapshot {
        owner_session_id: "owner-a".to_owned(),
        revision: 3,
        vpn_generation: VpnGeneration::new(1),
        vpn_state: lifecycle,
        readiness,
        resources: [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: VpnOwnerResourceState::Absent,
        })
        .collect(),
        foreground: false,
        mtu: None,
        route_count: 0,
        dns_servers: Vec::new(),
        ipv6: false,
        core: VpnOwnerCoreReadback::Unavailable {
            failure_message: "Core readback unavailable".to_owned(),
        },
    }
}

#[test]
fn the_routing_mode_is_derived_from_the_shared_routing_intent() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::FollowRules),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.routing_mode(), OverviewRoutingMode::Rule);

    snapshot.core.routing_intent.current = Some(RoutingIntent::RouteAllThroughProxy);
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.routing_mode(), OverviewRoutingMode::Global);

    snapshot.core.routing_intent.current = Some(RoutingIntent::BypassProxy);
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.routing_mode(), OverviewRoutingMode::Direct);

    snapshot.core.routing_intent.current = None;
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.routing_mode(), OverviewRoutingMode::Unknown);
}

#[test]
fn the_run_state_carries_the_reason_of_an_unavailable_core() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.core.lifecycle = CoreLifecycle::Running;
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.run_state(), &OverviewRunState::Running);
    assert!(model.is_running());

    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.run_state(), &OverviewRunState::Stopped);

    snapshot.core.lifecycle = CoreLifecycle::Failed;
    snapshot.core.failure = Some(Failure::new(
        ErrorCode::Network,
        "controller refused the connection",
        true,
    ));
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(
        model.run_state(),
        &OverviewRunState::Unavailable {
            reason: "controller refused the connection".to_owned(),
        }
    );
}

#[test]
fn the_desktop_traffic_mode_follows_the_shared_toggle_readback() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.network_mode(), OverviewNetworkMode::SystemProxy);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::SystemProxyEnabled
    );
    assert_eq!(model.network_warning(), &OverviewNetworkWarning::Confirmed);

    snapshot.system_proxy.status = SystemProxyStatus::Disabled;
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.network_mode(), OverviewNetworkMode::CoreOnly);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::SystemProxyDisabled
    );

    snapshot.runtime_control.status = RuntimeControlStatus::Ready;
    snapshot.runtime_control.tun_enabled = Some(true);
    model.observe(OverviewHost::Desktop, &snapshot);
    assert_eq!(model.network_mode(), OverviewNetworkMode::Tun);
    assert_eq!(model.network_status(), &OverviewNetworkStatus::TunEnabled);
}

#[test]
fn an_unobserved_system_proxy_warns_instead_of_claiming_a_mode() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.system_proxy.status = SystemProxyStatus::Unknown;
    model.observe(OverviewHost::Desktop, &snapshot);

    assert_eq!(model.network_mode(), OverviewNetworkMode::Unknown);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::SystemProxyUnobserved
    );
    assert!(model.network_warning().is_warned());
}

#[test]
fn the_android_host_reports_the_vpn_owner_state() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.vpn = VpnSurfaceObservation::AndroidOwner(Box::new(vpn_owner(
        VpnOwnerLifecycle::Running,
        VpnOwnerReadiness::Ready,
    )));
    model.observe(OverviewHost::Android, &snapshot);
    assert_eq!(model.network_mode(), OverviewNetworkMode::Vpn);
    // The owner has not published the foreground worker yet, so the tunnel is
    // not confirmed.
    assert_eq!(model.network_status(), &OverviewNetworkStatus::VpnStarting);

    snapshot.vpn = VpnSurfaceObservation::AndroidOwner(Box::new(vpn_owner(
        VpnOwnerLifecycle::PermissionRequired,
        VpnOwnerReadiness::PermissionRequired,
    )));
    model.observe(OverviewHost::Android, &snapshot);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::VpnPermissionRequired
    );
    assert!(model.network_warning().is_warned());

    snapshot.vpn = VpnSurfaceObservation::Unsupported {
        reason: "no VPN owner is composed".to_owned(),
    };
    model.observe(OverviewHost::Android, &snapshot);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::VpnUnsupported {
            reason: "no VPN owner is composed".to_owned(),
        }
    );
    assert!(model.network_warning().is_warned());
}

#[test]
fn an_android_host_never_reports_a_desktop_traffic_mode() {
    let mut model = OverviewPageModel::default();
    let mut snapshot = surface();

    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    snapshot.runtime_control.status = RuntimeControlStatus::Ready;
    snapshot.runtime_control.tun_enabled = Some(true);
    snapshot.vpn = VpnSurfaceObservation::NotApplicable;

    model.observe(OverviewHost::Android, &snapshot);
    assert_eq!(model.network_mode(), OverviewNetworkMode::Vpn);
    assert_eq!(
        model.network_status(),
        &OverviewNetworkStatus::VpnNotApplicable
    );
    // The owner is not composed here, so the page warns about it.
    assert!(model.network_warning().is_warned());
}
