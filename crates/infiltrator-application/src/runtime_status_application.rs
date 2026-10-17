//! Pure projection of independently observed Core, API, VPN, and traffic facts.

use infiltrator_contract::android_control::{
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceKind, VpnOwnerResourceState,
    VpnOwnerSnapshot,
};
use infiltrator_contract::runtime_status::{
    ControlServiceObservation, ControllerApiObservation, CoreProcessObservation,
    ProxyEffectivenessObservation, ProxyReadinessBlocker, RuntimeStatusSnapshot,
    TunDataPlaneObservation, VpnAuthorizationObservation,
};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_contract::traffic_topology::{TrafficTopologySnapshot, TrafficTopologyStatus};
use infiltrator_contract::vpn::VpnSurfaceObservation;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;

pub fn project_runtime_status(
    host: HostKind,
    origin: SurfaceOrigin,
    core: &CoreSnapshot,
    telemetry: Result<&CoreSnapshot, &Failure>,
    vpn_authorization: &VpnAuthorizationObservation,
    vpn: &VpnSurfaceObservation,
    topology: &TrafficTopologySnapshot,
) -> RuntimeStatusSnapshot {
    let control_service = project_control_service(host, vpn);
    let core_process = project_core_process(core);
    let controller_api = project_controller_api(core, telemetry);
    let tun_data_plane = project_tun_data_plane(host, vpn);
    let proxy_effectiveness = project_proxy_effectiveness(
        host,
        origin,
        &core_process,
        &controller_api,
        vpn_authorization,
        &control_service,
        &tun_data_plane,
        topology,
    );

    RuntimeStatusSnapshot {
        control_service,
        core_process,
        controller_api,
        vpn_authorization: vpn_authorization.clone(),
        tun_data_plane,
        proxy_effectiveness,
    }
}

fn project_control_service(
    host: HostKind,
    vpn: &VpnSurfaceObservation,
) -> ControlServiceObservation {
    if host != HostKind::Android {
        return ControlServiceObservation::NotApplicable;
    }

    match vpn {
        VpnSurfaceObservation::AndroidOwner(owner)
            if !owner.owner_session_id.trim().is_empty() && owner.revision > 0 =>
        {
            ControlServiceObservation::Available {
                owner_session_id: owner.owner_session_id.clone(),
                revision: owner.revision,
            }
        }
        VpnSurfaceObservation::AndroidOwner(_) => ControlServiceObservation::Unavailable {
            failure: Failure::new(
                ErrorCode::InvalidState,
                "Android control service returned an owner snapshot without session identity",
                true,
            ),
        },
        VpnSurfaceObservation::Unavailable { failure } => ControlServiceObservation::Unavailable {
            failure: failure.clone(),
        },
        VpnSurfaceObservation::Unsupported { reason } => ControlServiceObservation::Unsupported {
            reason: reason.clone(),
        },
        VpnSurfaceObservation::NotApplicable => ControlServiceObservation::Unavailable {
            failure: Failure::new(
                ErrorCode::NotReady,
                "Android control service has no owner observation",
                true,
            ),
        },
    }
}

fn project_core_process(core: &CoreSnapshot) -> CoreProcessObservation {
    if core.identity.is_none() && core.lifecycle == CoreLifecycle::Stopped {
        return CoreProcessObservation::Unselected;
    }
    match core.lifecycle {
        CoreLifecycle::Stopped => CoreProcessObservation::Stopped,
        CoreLifecycle::Starting => CoreProcessObservation::Starting,
        CoreLifecycle::Ready => CoreProcessObservation::Ready,
        CoreLifecycle::Running => CoreProcessObservation::Running,
        CoreLifecycle::Stopping => CoreProcessObservation::Stopping,
        CoreLifecycle::Failed => CoreProcessObservation::Failed {
            failure: core.failure.clone(),
        },
    }
}

fn project_controller_api(
    core: &CoreSnapshot,
    telemetry: Result<&CoreSnapshot, &Failure>,
) -> ControllerApiObservation {
    if !matches!(
        core.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    ) {
        return ControllerApiObservation::NotRunning;
    }

    match telemetry {
        Ok(observed)
            if observed.generation == core.generation
                && observed.session_token == core.session_token
                && observed.identity == core.identity =>
        {
            ControllerApiObservation::Connected {
                core_generation: observed.generation,
                core_revision: observed.revision,
            }
        }
        Ok(_) => ControllerApiObservation::Unavailable {
            failure: Failure::new(
                ErrorCode::NotReady,
                "Controller API observation belongs to a different Core session",
                true,
            ),
        },
        Err(failure) if failure.code == ErrorCode::Unsupported => {
            ControllerApiObservation::Unsupported {
                failure: failure.clone(),
            }
        }
        Err(failure) => ControllerApiObservation::Unavailable {
            failure: failure.clone(),
        },
    }
}

fn project_tun_data_plane(host: HostKind, vpn: &VpnSurfaceObservation) -> TunDataPlaneObservation {
    if host != HostKind::Android {
        return TunDataPlaneObservation::NotApplicable;
    }
    match vpn {
        VpnSurfaceObservation::AndroidOwner(owner) => project_owner_tun(owner),
        VpnSurfaceObservation::Unsupported { reason } => TunDataPlaneObservation::Unsupported {
            reason: reason.clone(),
        },
        VpnSurfaceObservation::Unavailable { failure } => TunDataPlaneObservation::Unavailable {
            failure: failure.message.clone(),
        },
        VpnSurfaceObservation::NotApplicable => TunDataPlaneObservation::Unavailable {
            failure: "Android VPN owner observation is not available".to_owned(),
        },
    }
}

fn project_owner_tun(owner: &VpnOwnerSnapshot) -> TunDataPlaneObservation {
    if let Some(resource) = owner.resources.iter().find_map(|fact| match &fact.state {
        VpnOwnerResourceState::CleanupPending { generation, .. }
            if *generation == owner.vpn_generation.value() =>
        {
            Some(fact.kind)
        }
        VpnOwnerResourceState::Absent
        | VpnOwnerResourceState::Held { .. }
        | VpnOwnerResourceState::CleanupPending { .. } => None,
    }) {
        return TunDataPlaneObservation::CleanupPending {
            generation: owner.vpn_generation,
            resource,
        };
    }

    match &owner.vpn_state {
        VpnOwnerLifecycle::PermissionRequired => TunDataPlaneObservation::PermissionRequired,
        VpnOwnerLifecycle::StartRequested => TunDataPlaneObservation::StartRequested,
        VpnOwnerLifecycle::Starting
        | VpnOwnerLifecycle::Foreground
        | VpnOwnerLifecycle::TunnelEstablished => TunDataPlaneObservation::Starting {
            readiness: owner.readiness,
        },
        VpnOwnerLifecycle::Running
            if owner.readiness == VpnOwnerReadiness::Ready
                && owner.foreground
                && resource_held_for_generation(owner, VpnOwnerResourceKind::Worker)
                && resource_held_for_generation(owner, VpnOwnerResourceKind::TunnelDescriptor)
                && resource_held_for_generation(owner, VpnOwnerResourceKind::RoutingLease) =>
        {
            TunDataPlaneObservation::Ready {
                generation: owner.vpn_generation,
            }
        }
        VpnOwnerLifecycle::Running => TunDataPlaneObservation::Inconsistent {
            lifecycle: owner.vpn_state.clone(),
            readiness: owner.readiness,
        },
        VpnOwnerLifecycle::Stopping => TunDataPlaneObservation::Stopping,
        VpnOwnerLifecycle::Stopped if owner.readiness == VpnOwnerReadiness::Stopped => {
            TunDataPlaneObservation::Stopped
        }
        VpnOwnerLifecycle::Stopped => TunDataPlaneObservation::Inconsistent {
            lifecycle: owner.vpn_state.clone(),
            readiness: owner.readiness,
        },
        VpnOwnerLifecycle::Revoked if owner.readiness == VpnOwnerReadiness::Revoked => {
            TunDataPlaneObservation::Revoked
        }
        VpnOwnerLifecycle::Revoked => TunDataPlaneObservation::Inconsistent {
            lifecycle: owner.vpn_state.clone(),
            readiness: owner.readiness,
        },
        VpnOwnerLifecycle::Failed { failure_message } => TunDataPlaneObservation::Failed {
            failure: failure_message.clone(),
        },
        VpnOwnerLifecycle::Unavailable { failure_message } => {
            TunDataPlaneObservation::Unavailable {
                failure: failure_message.clone(),
            }
        }
    }
}

fn resource_held_for_generation(owner: &VpnOwnerSnapshot, kind: VpnOwnerResourceKind) -> bool {
    let mut facts = owner.resources.iter().filter(|fact| fact.kind == kind);
    matches!(
        (facts.next(), facts.next()),
        (Some(fact), None)
            if matches!(&fact.state, VpnOwnerResourceState::Held { generation }
                if *generation == owner.vpn_generation.value())
    )
}

fn project_proxy_effectiveness(
    host: HostKind,
    origin: SurfaceOrigin,
    core: &CoreProcessObservation,
    controller_api: &ControllerApiObservation,
    vpn_authorization: &VpnAuthorizationObservation,
    control_service: &ControlServiceObservation,
    tun: &TunDataPlaneObservation,
    topology: &TrafficTopologySnapshot,
) -> ProxyEffectivenessObservation {
    if origin == SurfaceOrigin::Demo {
        return ProxyEffectivenessObservation::DemoFixture;
    }
    if !matches!(
        core,
        CoreProcessObservation::Ready | CoreProcessObservation::Running
    ) {
        return ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::CoreNotRunning,
        };
    }
    if !matches!(controller_api, ControllerApiObservation::Connected { .. }) {
        return ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::ControllerApiUnavailable,
        };
    }
    if host == HostKind::Android {
        if !matches!(control_service, ControlServiceObservation::Available { .. }) {
            return ProxyEffectivenessObservation::NotReady {
                blocker: ProxyReadinessBlocker::OwnerServiceUnavailable,
            };
        }
        match vpn_authorization {
            VpnAuthorizationObservation::Granted => {}
            VpnAuthorizationObservation::Required => {
                return ProxyEffectivenessObservation::NotReady {
                    blocker: ProxyReadinessBlocker::VpnAuthorizationRequired,
                };
            }
            VpnAuthorizationObservation::Unavailable { .. } => {
                return ProxyEffectivenessObservation::NotReady {
                    blocker: ProxyReadinessBlocker::VpnAuthorizationUnavailable,
                };
            }
            VpnAuthorizationObservation::Unsupported { .. } => {
                return ProxyEffectivenessObservation::NotReady {
                    blocker: ProxyReadinessBlocker::VpnAuthorizationUnsupported,
                };
            }
            VpnAuthorizationObservation::Unknown | VpnAuthorizationObservation::NotApplicable => {
                return ProxyEffectivenessObservation::NotReady {
                    blocker: ProxyReadinessBlocker::VpnAuthorizationUnknown,
                };
            }
        }
        if !matches!(tun, TunDataPlaneObservation::Ready { .. }) {
            return ProxyEffectivenessObservation::NotReady {
                blocker: ProxyReadinessBlocker::TunDataPlaneNotReady,
            };
        }
    }

    match topology.status {
        TrafficTopologyStatus::Empty => ProxyEffectivenessObservation::NoActiveTraffic,
        TrafficTopologyStatus::Ready if topology.active_connections > 0 => {
            ProxyEffectivenessObservation::ActiveTrafficUnattributed {
                active_connections: topology.active_connections,
            }
        }
        TrafficTopologyStatus::Unknown if topology.active_connections > 0 => {
            ProxyEffectivenessObservation::ActiveTrafficUnattributed {
                active_connections: topology.active_connections,
            }
        }
        TrafficTopologyStatus::Ready | TrafficTopologyStatus::Unknown => {
            ProxyEffectivenessObservation::Unknown
        }
        TrafficTopologyStatus::Unsupported => ProxyEffectivenessObservation::Unsupported {
            reason: topology
                .failure
                .clone()
                .unwrap_or_else(|| "traffic route observation is unsupported".to_owned()),
        },
        TrafficTopologyStatus::Failed => ProxyEffectivenessObservation::Unavailable {
            failure: Failure::new(
                ErrorCode::Network,
                topology
                    .failure
                    .clone()
                    .unwrap_or_else(|| "traffic route observation failed".to_owned()),
                true,
            ),
        },
    }
}

#[cfg(test)]
#[path = "runtime_status_application_test.rs"]
mod tests;
