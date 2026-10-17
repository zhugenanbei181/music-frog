//! Single fold of settings observations into localized peer-product status text.
use crate::text::localize;
use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::LanSecuritySnapshot;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, OfflineStartupState,
};
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::resources::{
    CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus, CoreResourceSnapshot, ResourceAvailability,
};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreChannelStatus, CoreVersionSnapshot,
};
use infiltrator_domain::redact::redact_line;
use lurker_ui_presentation::copy::TextCatalog;

fn tr(catalog: &dyn TextCatalog, key: &str) -> String {
    catalog.text(key).into_owned()
}

pub fn optional_copy<T: ToString>(value: Option<&T>, catalog: &dyn TextCatalog) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| tr(catalog, "shell_readout_unknown"))
}

pub fn format_runtime_status(status: &RuntimeControlStatus, catalog: &dyn TextCatalog) -> String {
    match status {
        RuntimeControlStatus::Ready => tr(catalog, "settings_runtime_observed"),
        RuntimeControlStatus::Unobserved => tr(catalog, "shell_readout_unknown"),
        RuntimeControlStatus::Failed { failure }
        | RuntimeControlStatus::Unsupported { failure } => localize(
            catalog,
            "settings_runtime_unavailable",
            &[("reason", safe(&failure.message))],
        ),
    }
}

pub fn format_lan_auth(value: Option<&LanSecuritySnapshot>, catalog: &dyn TextCatalog) -> String {
    match value {
        None => tr(catalog, "shell_readout_unknown"),
        Some(value) if value.authentication_enabled => localize(
            catalog,
            "settings_lan_auth_observed",
            &[("count", value.authentication_user_count.to_string())],
        ),
        Some(_) => tr(catalog, "settings_lan_auth_disabled"),
    }
}

pub fn format_ipv6(value: Option<&Ipv6RoutingSnapshot>, catalog: &dyn TextCatalog) -> String {
    let Some(value) = value else {
        return tr(catalog, "shell_readout_unknown");
    };
    localize(
        catalog,
        "settings_ipv6_observed",
        &[
            (
                "policy",
                tr(
                    catalog,
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
                    catalog,
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
pub fn format_integrity(snapshot: &CoreArtifactVerification, catalog: &dyn TextCatalog) -> String {
    match snapshot {
        CoreArtifactVerification::Unknown => tr(catalog, "core_integrity_unknown"),
        CoreArtifactVerification::Verified { version, .. } => localize(
            catalog,
            "core_integrity_verified",
            &[("version", version.clone())],
        ),
        CoreArtifactVerification::Rejected {
            version, failure, ..
        } => localize(
            catalog,
            "core_integrity_rejected",
            &[
                ("version", version.clone()),
                ("message", safe(&failure.message)),
            ],
        ),
    }
}
pub fn format_controller_auth(
    snapshot: &ControllerAuthSnapshot,
    catalog: &dyn TextCatalog,
) -> String {
    tr(
        catalog,
        match snapshot.status {
            ControllerAuthStatus::Unknown => "core_status_not_probed",
            ControllerAuthStatus::Secured => "core_auth_secured",
            ControllerAuthStatus::Missing => "core_auth_missing",
            ControllerAuthStatus::Unavailable => "core_status_host_unavailable",
        },
    )
}
pub fn format_core_versions(snapshot: &CoreVersionSnapshot, catalog: &dyn TextCatalog) -> String {
    if snapshot.channels.is_empty() {
        return tr(catalog, "core_status_not_probed");
    }
    snapshot
        .channels
        .iter()
        .map(|channel| {
            let version = match &channel.status {
                CoreChannelStatus::Ready { release } => release.version.clone(),
                CoreChannelStatus::Failed { .. } => tr(catalog, "common_unavailable"),
            };
            format!("{}={version}", channel.channel.channel_id.as_str())
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
pub fn format_rollback_target(snapshot: &CoreVersionSnapshot, catalog: &dyn TextCatalog) -> String {
    snapshot.rollback.target.as_ref().map_or_else(
        || tr(catalog, "core_rollback_none"),
        |version| {
            localize(
                catalog,
                "core_rollback_target",
                &[("version", version.clone())],
            )
        },
    )
}
pub fn format_service_mode(snapshot: &ServiceModeSnapshot, catalog: &dyn TextCatalog) -> String {
    let platform = match snapshot.platform {
        ServiceModePlatform::WindowsService => "Windows Service".into(),
        ServiceModePlatform::LinuxPolkit => "Linux Polkit".into(),
        ServiceModePlatform::MacosLaunchd => "macOS launchd".into(),
        ServiceModePlatform::Unsupported => tr(catalog, "core_status_host_unsupported"),
    };
    let state = tr(
        catalog,
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
pub fn format_port_conflicts(snapshot: &PortConflictSnapshot, catalog: &dyn TextCatalog) -> String {
    if snapshot.conflicts.is_empty() {
        return tr(catalog, "core_status_not_probed");
    }
    snapshot
        .conflicts
        .iter()
        .map(|conflict| {
            let status = tr(
                catalog,
                if conflict.available {
                    "core_port_available"
                } else {
                    "core_port_occupied"
                },
            );
            let owner = conflict.owner_pid.map_or_else(
                || tr(catalog, "core_port_owner_unknown"),
                |pid| {
                    let name = conflict
                        .owner_name
                        .as_deref()
                        .map(safe)
                        .unwrap_or_else(|| tr(catalog, "proxy_inspection_unknown"));
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
pub fn format_core_resources(snapshot: &CoreResourceSnapshot, catalog: &dyn TextCatalog) -> String {
    let (memory, cpu, gc, memory_limit) = match snapshot {
        CoreResourceSnapshot::NotObserved => (
            tr(catalog, "core_resource_not_observed"),
            tr(catalog, "core_resource_not_observed"),
            tr(catalog, "core_gc_not_attempted"),
            format!("{:.1}", CORE_MEMORY_SOFT_LIMIT_BYTES as f64 / 1_048_576.0),
        ),
        CoreResourceSnapshot::Observed {
            memory,
            cpu_percent,
            memory_soft_limit_bytes,
            gc,
            ..
        } => (
            format_resource_availability(memory, catalog, |bytes| {
                format!("{:.1} MiB", *bytes as f64 / 1_048_576.0)
            }),
            format_resource_availability(cpu_percent, catalog, |value| {
                if value.is_finite() {
                    format!("{value:.1}%")
                } else {
                    tr(catalog, "core_resource_invalid")
                }
            }),
            format_gc_status(gc, catalog),
            format!("{:.1}", *memory_soft_limit_bytes as f64 / 1_048_576.0),
        ),
    };
    localize(
        catalog,
        "core_resources_status",
        &[
            ("memory", memory),
            ("cpu", cpu),
            ("limit", memory_limit),
            ("gc", gc),
        ],
    )
}

fn format_resource_availability<T>(
    availability: &ResourceAvailability<T>,
    catalog: &dyn TextCatalog,
    format_value: impl FnOnce(&T) -> String,
) -> String {
    match availability {
        ResourceAvailability::NotObserved => tr(catalog, "core_resource_not_observed"),
        ResourceAvailability::Available(value) => format_value(value),
        ResourceAvailability::Unsupported => tr(catalog, "core_resource_unsupported"),
        ResourceAvailability::Failed(failure) => localize(
            catalog,
            "core_resource_failed",
            &[("message", safe(&failure.message))],
        ),
    }
}

fn format_gc_status(status: &CoreGcStatus, catalog: &dyn TextCatalog) -> String {
    match status {
        CoreGcStatus::NotAttempted => tr(catalog, "core_gc_not_attempted"),
        CoreGcStatus::NotNeeded => tr(catalog, "core_gc_not_needed"),
        CoreGcStatus::Collecting => tr(catalog, "core_gc_collecting"),
        CoreGcStatus::Collected { after_bytes, .. } => localize(
            catalog,
            "core_gc_collected_after",
            &[(
                "memory",
                format!("{:.1}", *after_bytes as f64 / 1_048_576.0),
            )],
        ),
        CoreGcStatus::Failed { failure } => localize(
            catalog,
            "core_gc_failed",
            &[("message", safe(&failure.message))],
        ),
        CoreGcStatus::Unsupported => tr(catalog, "core_gc_unsupported"),
    }
}
pub fn format_mtu(snapshot: &MtuNegotiationSnapshot, catalog: &dyn TextCatalog) -> String {
    match &snapshot.state {
        MtuProbeState::Unknown => tr(catalog, "core_status_not_probed"),
        MtuProbeState::Probing => tr(catalog, "core_status_probing"),
        MtuProbeState::Unsupported => tr(catalog, "core_status_host_unsupported"),
        MtuProbeState::Failed { failure } => localize(
            catalog,
            "core_status_failed",
            &[("message", safe(&failure.message))],
        ),
        MtuProbeState::Ready => localize(
            catalog,
            "core_mtu_ready",
            &[
                (
                    "interface",
                    snapshot
                        .physical_interface
                        .as_deref()
                        .map(safe)
                        .unwrap_or_else(|| tr(catalog, "core_mtu_active_link")),
                ),
                ("physical", observed(snapshot.physical_mtu)),
                ("tun", observed(snapshot.tun_mtu)),
                ("mss", observed(snapshot.tcp_mss)),
                ("overhead", snapshot.overhead_bytes.to_string()),
                (
                    "applied",
                    snapshot.applied_tun_mtu.map_or_else(
                        || tr(catalog, "core_mtu_not_applied"),
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
pub fn format_offline_startup(
    snapshot: &OfflineStartupSnapshot,
    catalog: &dyn TextCatalog,
) -> String {
    let state = tr(
        catalog,
        match snapshot.state {
            OfflineStartupState::Unknown => "core_status_not_probed",
            OfflineStartupState::Checking => "core_offline_checking",
            OfflineStartupState::Ready => "core_offline_ready",
            OfflineStartupState::Degraded => "core_offline_degraded",
            OfflineStartupState::Blocked => "core_offline_blocked",
        },
    );
    let config = tr(
        catalog,
        if snapshot.config_valid {
            "core_config_valid"
        } else {
            "core_config_invalid"
        },
    );
    let binary = tr(
        catalog,
        if snapshot.binary_available {
            "core_binary_available"
        } else {
            "core_binary_missing"
        },
    );
    let geoip = tr(
        catalog,
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
        catalog,
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
