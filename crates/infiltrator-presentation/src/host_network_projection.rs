//! One fact-to-copy owner for host VPN, privileged transactions and PAC observations.
use infiltrator_contract::android_control::{
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::privileged_network::{PrivilegedNetworkSnapshot, PrivilegedNetworkState};
use infiltrator_contract::vpn::VpnSurfaceObservation;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct HostActionPresentation {
    pub status: String,
    pub details: String,
    pub start_key: &'static str,
    pub stop_key: &'static str,
    pub start_enabled: bool,
    pub stop_enabled: bool,
}
fn status_detail(key: &str, reason: Option<&str>, code: &str) -> String {
    let state = Lang(code).tr(key).into_owned();
    reason.map_or_else(
        || state.clone(),
        |reason| {
            localize(
                code,
                "network_status_detail",
                &[("state", state.clone()), ("reason", reason.into())],
            )
        },
    )
}
pub fn vpn(observation: &VpnSurfaceObservation, code: &str) -> HostActionPresentation {
    let presentation = vpn_presentation(observation);
    let (key, reason) = match &presentation {
        VpnPresentation::PermissionRequired => ("vpn_status_permission", None),
        VpnPresentation::Starting => ("vpn_status_starting", None),
        VpnPresentation::Running => ("vpn_status_running", None),
        VpnPresentation::Stopping => ("vpn_status_stopping", None),
        VpnPresentation::Stopped => ("vpn_status_stopped", None),
        VpnPresentation::Revoked => ("vpn_status_revoked", None),
        VpnPresentation::Unsupported(reason) => ("vpn_status_unsupported", Some(reason.as_str())),
        VpnPresentation::Failed(reason) => ("vpn_status_failed", Some(reason.as_str())),
        VpnPresentation::CleanupPending(reason) => ("vpn_status_failed", Some(reason.as_str())),
    };
    let details = match observation {
        VpnSurfaceObservation::AndroidOwner(snapshot) => vpn_owner_details(snapshot, code),
        VpnSurfaceObservation::Unsupported { .. }
        | VpnSurfaceObservation::NotApplicable
        | VpnSurfaceObservation::Unavailable { .. } => localize(
            code,
            "network_vpn_details",
            &[
                (
                    "foreground",
                    Lang(code).tr("shell_readout_unknown").into_owned(),
                ),
                ("mtu", Lang(code).tr("shell_readout_unknown").into_owned()),
                (
                    "routes",
                    Lang(code).tr("shell_readout_unknown").into_owned(),
                ),
                ("ipv6", Lang(code).tr("shell_readout_unknown").into_owned()),
            ],
        ),
    };
    let start_enabled = matches!(
        &presentation,
        VpnPresentation::PermissionRequired
            | VpnPresentation::Stopped
            | VpnPresentation::Revoked
            | VpnPresentation::Failed(_)
    );
    let stop_enabled = matches!(
        &presentation,
        VpnPresentation::Running | VpnPresentation::Starting | VpnPresentation::CleanupPending(_)
    );
    let start_active = matches!(
        &presentation,
        VpnPresentation::Running | VpnPresentation::Starting
    );
    let stop_active = matches!(&presentation, VpnPresentation::Stopping);
    HostActionPresentation {
        status: status_detail(key, reason, code),
        details,
        start_key: if start_active {
            "vpn_start_active"
        } else {
            "vpn_start"
        },
        stop_key: if stop_enabled || stop_active {
            "vpn_stop"
        } else {
            "vpn_stopped"
        },
        start_enabled,
        stop_enabled,
    }
}

enum VpnPresentation {
    PermissionRequired,
    Starting,
    Running,
    Stopping,
    CleanupPending(String),
    Stopped,
    Revoked,
    Unsupported(String),
    Failed(String),
}

fn vpn_presentation(observation: &VpnSurfaceObservation) -> VpnPresentation {
    match observation {
        VpnSurfaceObservation::Unsupported { reason } => {
            VpnPresentation::Unsupported(reason.clone())
        }
        VpnSurfaceObservation::NotApplicable => {
            VpnPresentation::Unsupported("VPN is not applicable to this surface".to_owned())
        }
        VpnSurfaceObservation::Unavailable { failure } => {
            VpnPresentation::Failed(failure.message.clone())
        }
        VpnSurfaceObservation::AndroidOwner(snapshot) => vpn_owner_presentation(snapshot),
    }
}

fn vpn_owner_presentation(snapshot: &VpnOwnerSnapshot) -> VpnPresentation {
    if let Some(message) = snapshot
        .resources
        .iter()
        .find_map(|resource| match &resource.state {
            VpnOwnerResourceState::CleanupPending {
                failure_message, ..
            } => Some(failure_message.clone()),
            VpnOwnerResourceState::Absent | VpnOwnerResourceState::Held { .. } => None,
        })
    {
        return VpnPresentation::CleanupPending(message);
    }
    match (&snapshot.vpn_state, snapshot.readiness) {
        (VpnOwnerLifecycle::Unavailable { failure_message }, _) => {
            VpnPresentation::Unsupported(failure_message.clone())
        }
        (_, VpnOwnerReadiness::Unavailable) => {
            VpnPresentation::Unsupported("VPN owner readback is unavailable".to_owned())
        }
        (VpnOwnerLifecycle::Failed { failure_message }, _) => {
            VpnPresentation::Failed(failure_message.clone())
        }
        (_, VpnOwnerReadiness::Failed) => VpnPresentation::Failed(
            "VPN owner published failed readiness without failure detail".to_owned(),
        ),
        (_, VpnOwnerReadiness::CleanupPending) => {
            VpnPresentation::CleanupPending("VPN owner cleanup is pending".to_owned())
        }
        (VpnOwnerLifecycle::PermissionRequired, _) | (_, VpnOwnerReadiness::PermissionRequired) => {
            VpnPresentation::PermissionRequired
        }
        (
            VpnOwnerLifecycle::StartRequested
            | VpnOwnerLifecycle::Starting
            | VpnOwnerLifecycle::Foreground
            | VpnOwnerLifecycle::TunnelEstablished,
            _,
        )
        | (
            _,
            VpnOwnerReadiness::AwaitingForeground
            | VpnOwnerReadiness::AwaitingTunnel
            | VpnOwnerReadiness::AwaitingDataPlane,
        ) => VpnPresentation::Starting,
        (VpnOwnerLifecycle::Stopping, _) | (_, VpnOwnerReadiness::Stopping) => {
            VpnPresentation::Stopping
        }
        (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Ready) if snapshot.is_running() => {
            VpnPresentation::Running
        }
        (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Ready) => VpnPresentation::Failed(
            "VPN owner Running state lacks its worker or tunnel resource".to_owned(),
        ),
        (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Stopped | VpnOwnerReadiness::Revoked) => {
            VpnPresentation::Failed(
                "VPN owner Running state conflicts with its terminal readiness".to_owned(),
            )
        }
        (VpnOwnerLifecycle::Stopped, _) => VpnPresentation::Stopped,
        (VpnOwnerLifecycle::Revoked, _) => VpnPresentation::Revoked,
    }
}

fn vpn_owner_details(snapshot: &VpnOwnerSnapshot, code: &str) -> String {
    let unknown = Lang(code).tr("shell_readout_unknown").into_owned();
    localize(
        code,
        "network_vpn_details",
        &[
            ("foreground", snapshot.foreground.to_string()),
            (
                "mtu",
                snapshot
                    .mtu
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| unknown.clone()),
            ),
            ("routes", snapshot.route_count.to_string()),
            ("ipv6", snapshot.ipv6.to_string()),
        ],
    )
}
pub fn privileged(snapshot: &PrivilegedNetworkSnapshot, code: &str) -> HostActionPresentation {
    let (key, reason) = match &snapshot.state {
        PrivilegedNetworkState::Idle => ("privileged_network_status_idle", None),
        PrivilegedNetworkState::Injecting => ("privileged_network_status_injecting", None),
        PrivilegedNetworkState::Active => ("privileged_network_status_active", None),
        PrivilegedNetworkState::RollingBack => ("privileged_network_status_rolling_back", None),
        PrivilegedNetworkState::Cleaned => ("privileged_network_status_cleaned", None),
        PrivilegedNetworkState::Unsupported { reason } => (
            "privileged_network_status_unsupported",
            Some(reason.as_str()),
        ),
        PrivilegedNetworkState::Failed { failure } => (
            "privileged_network_status_failed",
            Some(failure.message.as_str()),
        ),
    };
    HostActionPresentation {
        status: status_detail(key, reason, code),
        details: localize(
            code,
            "network_privileged_details",
            &[
                ("operations", snapshot.operation_count.to_string()),
                ("injected", snapshot.injected.to_string()),
                ("cleanup", snapshot.cleanup_attempted.to_string()),
                ("rollback", snapshot.rollback_attempted.to_string()),
            ],
        ),
        start_key: "privileged_network_run",
        stop_key: "",
        start_enabled: matches!(
            snapshot.state,
            PrivilegedNetworkState::Idle
                | PrivilegedNetworkState::Cleaned
                | PrivilegedNetworkState::Failed { .. }
        ),
        stop_enabled: false,
    }
}
pub fn pac(snapshot: &PacSnapshot, code: &str) -> String {
    match &snapshot.state {
        PacServiceState::Running { url } => localize(
            code,
            "network_pac_running",
            &[
                ("url", url.clone()),
                ("bytes", snapshot.script_bytes.to_string()),
            ],
        ),
        PacServiceState::Disabled => Lang(code).tr("network_pac_disabled").into_owned(),
        PacServiceState::Unavailable { reason } => reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{privileged, vpn};
    use infiltrator_contract::android_control::{
        VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
        VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
    };
    use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::privileged_network::{
        PrivilegedNetworkSnapshot, PrivilegedNetworkState,
    };
    use infiltrator_contract::session::SessionToken;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::vpn::{VpnGeneration, VpnSurfaceObservation};

    #[test]
    fn host_lifecycle_facts_keep_unknown_mtu_errors_and_inflight_actions_distinct() {
        let mut observation = running_owner();
        let absent = vpn(
            &VpnSurfaceObservation::from_android_owner(observation.clone()),
            "en-US",
        );
        assert!(absent.details.contains("MTU=Not observed"));
        assert!(!absent.start_enabled);
        assert!(absent.stop_enabled);
        observation.mtu = Some(0);
        assert!(
            vpn(
                &VpnSurfaceObservation::from_android_owner(observation.clone()),
                "en-US"
            )
            .details
            .contains("MTU=0")
        );
        observation.vpn_state = VpnOwnerLifecycle::Stopping;
        observation.readiness = VpnOwnerReadiness::Stopping;
        let stopping = vpn(
            &VpnSurfaceObservation::from_android_owner(observation.clone()),
            "en-US",
        );
        assert!(!stopping.start_enabled && !stopping.stop_enabled);
        let reason = "permission {state} / 中文🙂";
        let unsupported = VpnSurfaceObservation::Unsupported {
            reason: reason.into(),
        };
        assert_eq!(
            vpn(&unsupported, "en-US").status,
            format!("Host unsupported · {reason}")
        );
        let failed = VpnSurfaceObservation::Unavailable {
            failure: Failure::new(ErrorCode::Permission, reason, false),
        };
        assert_eq!(vpn(&failed, "en-US").status, format!("Failed · {reason}"));
        let snapshot = PrivilegedNetworkSnapshot {
            state: PrivilegedNetworkState::RollingBack,
            ..Default::default()
        };
        assert!(!privileged(&snapshot, "en-US").start_enabled);
    }

    fn running_owner() -> VpnOwnerSnapshot {
        use infiltrator_contract::snapshot::CoreLifecycleSnapshot;

        VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 4,
            vpn_generation: VpnGeneration::new(3),
            vpn_state: VpnOwnerLifecycle::Running,
            readiness: VpnOwnerReadiness::Ready,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: match kind {
                    VpnOwnerResourceKind::Worker | VpnOwnerResourceKind::TunnelDescriptor => {
                        VpnOwnerResourceState::Held { generation: 3 }
                    }
                    VpnOwnerResourceKind::PreparedConfiguration
                    | VpnOwnerResourceKind::RoutingLease => VpnOwnerResourceState::Absent,
                },
            })
            .collect(),
            foreground: true,
            mtu: None,
            route_count: 1,
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
                identity: Some(CoreIdentity::new(
                    EngineId::new("mihomo").expect("engine ID"),
                    CoreInstanceId::new("owner-a").expect("instance ID"),
                    None,
                )),
                lifecycle: CoreLifecycle::Running,
                generation: 3,
                session_token: Some(SessionToken::new(3)),
                revision: 4,
                failure: None,
            }),
        }
    }
}
