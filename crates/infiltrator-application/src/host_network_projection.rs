//! One fact-to-copy owner for host VPN, privileged transactions and PAC observations.
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::privileged_network::{PrivilegedNetworkSnapshot, PrivilegedNetworkState};
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
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
pub fn vpn(snapshot: &VpnSessionSnapshot, code: &str) -> HostActionPresentation {
    let (key, reason) = match &snapshot.state {
        VpnSessionState::Idle => ("vpn_status_idle", None),
        VpnSessionState::PermissionRequired => ("vpn_status_permission", None),
        VpnSessionState::Starting => ("vpn_status_starting", None),
        VpnSessionState::Running => ("vpn_status_running", None),
        VpnSessionState::Stopping => ("vpn_status_stopping", None),
        VpnSessionState::Stopped => ("vpn_status_stopped", None),
        VpnSessionState::Revoked => ("vpn_status_revoked", None),
        VpnSessionState::Unsupported { reason } => {
            ("vpn_status_unsupported", Some(reason.as_str()))
        }
        VpnSessionState::Failed { failure } => {
            ("vpn_status_failed", Some(failure.message.as_str()))
        }
    };
    let active = matches!(
        snapshot.state,
        VpnSessionState::Running | VpnSessionState::Starting
    );
    HostActionPresentation {
        status: status_detail(key, reason, code),
        details: localize(
            code,
            "network_vpn_details",
            &[
                ("foreground", snapshot.foreground.to_string()),
                (
                    "mtu",
                    snapshot
                        .mtu
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| Lang(code).tr("shell_readout_unknown").into_owned()),
                ),
                ("routes", snapshot.route_count.to_string()),
                ("ipv6", snapshot.ipv6.to_string()),
            ],
        ),
        start_key: if active {
            "vpn_start_active"
        } else {
            "vpn_start"
        },
        stop_key: if active || matches!(snapshot.state, VpnSessionState::Stopping) {
            "vpn_stop"
        } else {
            "vpn_stopped"
        },
        start_enabled: matches!(
            snapshot.state,
            VpnSessionState::Idle
                | VpnSessionState::PermissionRequired
                | VpnSessionState::Stopped
                | VpnSessionState::Revoked
                | VpnSessionState::Failed { .. }
        ),
        stop_enabled: active,
    }
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
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::privileged_network::{
        PrivilegedNetworkSnapshot, PrivilegedNetworkState,
    };
    use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};

    #[test]
    fn host_lifecycle_facts_keep_unknown_mtu_errors_and_inflight_actions_distinct() {
        let mut snapshot = VpnSessionSnapshot {
            state: VpnSessionState::Running,
            ..Default::default()
        };
        let absent = vpn(&snapshot, "en-US");
        assert!(absent.details.contains("MTU=Not observed"));
        assert!(!absent.start_enabled);
        assert!(absent.stop_enabled);
        snapshot.mtu = Some(0);
        assert!(vpn(&snapshot, "en-US").details.contains("MTU=0"));
        snapshot.state = VpnSessionState::Stopping;
        let stopping = vpn(&snapshot, "en-US");
        assert!(!stopping.start_enabled && !stopping.stop_enabled);
        let reason = "permission {state} / 中文🙂";
        snapshot.state = VpnSessionState::Unsupported {
            reason: reason.into(),
        };
        assert_eq!(
            vpn(&snapshot, "en-US").status,
            format!("Host unsupported · {reason}")
        );
        snapshot.state = VpnSessionState::Failed {
            failure: Failure::new(ErrorCode::Permission, reason, false),
        };
        assert_eq!(vpn(&snapshot, "en-US").status, format!("Failed · {reason}"));
        let snapshot = PrivilegedNetworkSnapshot {
            state: PrivilegedNetworkState::RollingBack,
            ..Default::default()
        };
        assert!(!privileged(&snapshot, "en-US").start_enabled);
    }
}
