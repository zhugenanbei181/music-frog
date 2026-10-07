//! Single fold of settings observations into localized peer-product status text.
use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::LanSecuritySnapshot;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, OfflineStartupState,
};
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::resources::{CoreGcStatus, CoreResourceSnapshot};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreChannelStatus, CoreVersionSnapshot,
};
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

fn tr(code: &str, key: &str) -> String {
    Lang(code).tr(key).into_owned()
}

pub fn optional_copy<T: ToString>(value: Option<&T>, code: &str) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| tr(code, "shell_readout_unknown"))
}

pub fn format_runtime_status(status: &RuntimeControlStatus, code: &str) -> String {
    match status {
        RuntimeControlStatus::Ready => tr(code, "settings_runtime_observed"),
        RuntimeControlStatus::Unobserved => tr(code, "shell_readout_unknown"),
        RuntimeControlStatus::Failed { failure }
        | RuntimeControlStatus::Unsupported { failure } => localize(
            code,
            "settings_runtime_unavailable",
            &[("reason", safe(&failure.message))],
        ),
    }
}

pub fn format_lan_auth(value: Option<&LanSecuritySnapshot>, code: &str) -> String {
    match value {
        None => tr(code, "shell_readout_unknown"),
        Some(value) if value.authentication_enabled => localize(
            code,
            "settings_lan_auth_observed",
            &[("count", value.authentication_user_count.to_string())],
        ),
        Some(_) => tr(code, "settings_lan_auth_disabled"),
    }
}

pub fn format_ipv6(value: Option<&Ipv6RoutingSnapshot>, code: &str) -> String {
    let Some(value) = value else {
        return tr(code, "shell_readout_unknown");
    };
    localize(
        code,
        "settings_ipv6_observed",
        &[
            (
                "policy",
                tr(
                    code,
                    if value.enabled {
                        "settings_ipv6_allowed"
                    } else {
                        "settings_ipv6_disabled"
                    },
                ),
            ),
            (
                "tun",
                tr(
                    code,
                    if value.tun_enabled {
                        "settings_tun_on"
                    } else {
                        "settings_tun_off"
                    },
                ),
            ),
        ],
    )
}
fn safe(value: &str) -> String {
    redact_line(value, &[])
}
pub fn format_integrity(snapshot: &CoreArtifactVerification, code: &str) -> String {
    match snapshot {
        CoreArtifactVerification::Unknown => tr(code, "core_integrity_unknown"),
        CoreArtifactVerification::Verified { version } => localize(
            code,
            "core_integrity_verified",
            &[("version", version.clone())],
        ),
        CoreArtifactVerification::Rejected { version, failure } => localize(
            code,
            "core_integrity_rejected",
            &[
                ("version", version.clone()),
                ("message", safe(&failure.message)),
            ],
        ),
    }
}
pub fn format_controller_auth(snapshot: &ControllerAuthSnapshot, code: &str) -> String {
    tr(
        code,
        match snapshot.status {
            ControllerAuthStatus::Unknown => "core_status_not_probed",
            ControllerAuthStatus::Secured => "core_auth_secured",
            ControllerAuthStatus::Missing => "core_auth_missing",
            ControllerAuthStatus::Unavailable => "core_status_host_unavailable",
        },
    )
}
pub fn format_core_versions(snapshot: &CoreVersionSnapshot, code: &str) -> String {
    if snapshot.channels.is_empty() {
        return tr(code, "core_status_not_probed");
    }
    snapshot
        .channels
        .iter()
        .map(|channel| {
            let version = match &channel.status {
                CoreChannelStatus::Ready { release } => release.version.clone(),
                CoreChannelStatus::Failed { .. } => tr(code, "common_unavailable"),
            };
            format!("{}={version}", channel.channel.as_str())
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
pub fn format_rollback_target(snapshot: &CoreVersionSnapshot, code: &str) -> String {
    snapshot.rollback.target.as_ref().map_or_else(
        || tr(code, "core_rollback_none"),
        |version| {
            localize(
                code,
                "core_rollback_target",
                &[("version", version.clone())],
            )
        },
    )
}
pub fn format_service_mode(snapshot: &ServiceModeSnapshot, code: &str) -> String {
    let platform = match snapshot.platform {
        ServiceModePlatform::WindowsService => "Windows Service".into(),
        ServiceModePlatform::LinuxPolkit => "Linux Polkit".into(),
        ServiceModePlatform::MacosLaunchd => "macOS launchd".into(),
        ServiceModePlatform::Unsupported => tr(code, "core_status_host_unsupported"),
    };
    let state = tr(
        code,
        match snapshot.state {
            ServiceModeState::Ready => "core_service_ready",
            ServiceModeState::InstalledStopped => "core_service_stopped",
            ServiceModeState::NotInstalled => "core_service_not_installed",
            ServiceModeState::MissingPrivilege => "core_service_missing_privilege",
            ServiceModeState::Unavailable => "common_unavailable",
            ServiceModeState::Unsupported => "core_status_host_unsupported",
        },
    );
    format!("{platform} · {state}")
}
pub fn format_port_conflicts(snapshot: &PortConflictSnapshot, code: &str) -> String {
    if snapshot.conflicts.is_empty() {
        return tr(code, "core_status_not_probed");
    }
    snapshot
        .conflicts
        .iter()
        .map(|conflict| {
            let status = tr(
                code,
                if conflict.available {
                    "core_port_available"
                } else {
                    "core_port_occupied"
                },
            );
            let owner = conflict.owner_pid.map_or_else(
                || tr(code, "core_port_owner_unknown"),
                |pid| {
                    let name = conflict
                        .owner_name
                        .as_deref()
                        .map(safe)
                        .unwrap_or_else(|| tr(code, "proxy_inspection_unknown"));
                    format!("{name} pid={pid}")
                },
            );
            format!(
                "{} {} ({status}, {owner})",
                conflict.binding.as_str(),
                conflict.port
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
pub fn format_core_resources(snapshot: &CoreResourceSnapshot, code: &str) -> String {
    let memory = snapshot.memory_bytes.map_or_else(
        || "?".into(),
        |bytes| format!("{:.1}", bytes as f64 / 1_048_576.0),
    );
    let cpu = snapshot
        .cpu_percent
        .filter(|value| value.is_finite())
        .map_or_else(|| "?".into(), |value| format!("{value:.1}"));
    let gc = match &snapshot.gc {
        CoreGcStatus::Unknown => tr(code, "core_gc_unknown"),
        CoreGcStatus::NotNeeded => tr(code, "core_gc_not_needed"),
        CoreGcStatus::Triggered { after_bytes, .. } => after_bytes.map_or_else(
            || tr(code, "core_gc_triggered"),
            |bytes| {
                localize(
                    code,
                    "core_gc_after",
                    &[("memory", format!("{:.1}", bytes as f64 / 1_048_576.0))],
                )
            },
        ),
        CoreGcStatus::Failed { failure } => localize(
            code,
            "core_gc_failed",
            &[("message", safe(&failure.message))],
        ),
        CoreGcStatus::Unsupported => tr(code, "core_gc_unsupported"),
    };
    localize(
        code,
        "core_resources_status",
        &[
            ("memory", memory),
            ("cpu", cpu),
            (
                "limit",
                format!(
                    "{:.1}",
                    snapshot.memory_soft_limit_bytes as f64 / 1_048_576.0
                ),
            ),
            ("gc", gc),
        ],
    )
}
pub fn format_mtu(snapshot: &MtuNegotiationSnapshot, code: &str) -> String {
    match &snapshot.state {
        MtuProbeState::Unknown => tr(code, "core_status_not_probed"),
        MtuProbeState::Probing => tr(code, "core_status_probing"),
        MtuProbeState::Unsupported => tr(code, "core_status_host_unsupported"),
        MtuProbeState::Failed { failure } => localize(
            code,
            "core_status_failed",
            &[("message", safe(&failure.message))],
        ),
        MtuProbeState::Ready => localize(
            code,
            "core_mtu_ready",
            &[
                (
                    "interface",
                    snapshot
                        .physical_interface
                        .as_deref()
                        .map(safe)
                        .unwrap_or_else(|| tr(code, "core_mtu_active_link")),
                ),
                ("physical", observed(snapshot.physical_mtu)),
                ("tun", observed(snapshot.tun_mtu)),
                ("mss", observed(snapshot.tcp_mss)),
                ("overhead", snapshot.overhead_bytes.to_string()),
                (
                    "applied",
                    snapshot.applied_tun_mtu.map_or_else(
                        || tr(code, "core_mtu_not_applied"),
                        |value| value.to_string(),
                    ),
                ),
            ],
        ),
    }
}
fn observed(value: Option<u32>) -> String {
    value.map_or_else(|| "?".into(), |value| value.to_string())
}

#[cfg(test)]
#[path = "settings_status_projection_test.rs"]
mod tests;
pub fn format_offline_startup(snapshot: &OfflineStartupSnapshot, code: &str) -> String {
    let state = tr(
        code,
        match snapshot.state {
            OfflineStartupState::Unknown => "core_status_not_probed",
            OfflineStartupState::Checking => "core_offline_checking",
            OfflineStartupState::Ready => "core_offline_ready",
            OfflineStartupState::Degraded => "core_offline_degraded",
            OfflineStartupState::Blocked => "core_offline_blocked",
        },
    );
    let config = tr(
        code,
        if snapshot.config_valid {
            "core_config_valid"
        } else {
            "core_config_invalid"
        },
    );
    let binary = tr(
        code,
        if snapshot.binary_available {
            "core_binary_available"
        } else {
            "core_binary_missing"
        },
    );
    let geoip = tr(
        code,
        match snapshot.geoip {
            LocalAssetStatus::NotRequired => "core_geoip_not_required",
            LocalAssetStatus::Available => "core_geoip_local",
            LocalAssetStatus::Missing => "core_binary_missing",
        },
    );
    let failure = snapshot
        .failure
        .as_ref()
        .map_or_else(String::new, |failure| {
            format!(" · {}", safe(&failure.message))
        });
    localize(
        code,
        "core_offline_status",
        &[
            ("state", state),
            ("config", config),
            ("binary", binary),
            ("geoip", geoip),
            ("failure", failure),
        ],
    )
}
