//! Formatting helpers for runtime snapshots shown on the Settings page.

use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, OfflineStartupState, StartupRemoteDependency,
};
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::resources::{CoreGcStatus, CoreResourceSnapshot};
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreChannelStatus, CoreVersionSnapshot,
};

pub(super) fn format_core_versions(snapshot: &CoreVersionSnapshot) -> String {
    if snapshot.channels.is_empty() {
        return "not probed".to_owned();
    }
    snapshot
        .channels
        .iter()
        .map(|channel| match &channel.status {
            CoreChannelStatus::Ready { release } => {
                format!("{}={}", channel.channel.as_str(), release.version)
            }
            CoreChannelStatus::Failed { .. } => {
                format!("{}=unavailable", channel.channel.as_str())
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) fn format_integrity(verification: &CoreArtifactVerification) -> String {
    match verification {
        CoreArtifactVerification::Unknown => "not verified".to_owned(),
        CoreArtifactVerification::Verified { version } => {
            format!("verified ({version})")
        }
        CoreArtifactVerification::Rejected { version, failure } => {
            format!(
                "rejected ({version}: {})",
                crate::utils::sanitize_ui_text(&failure.message)
            )
        }
    }
}

pub(super) fn format_controller_auth(snapshot: &ControllerAuthSnapshot) -> String {
    match snapshot.status {
        ControllerAuthStatus::Unknown => "not probed".to_owned(),
        ControllerAuthStatus::Secured => "secured · Bearer".to_owned(),
        ControllerAuthStatus::Missing => "missing secret".to_owned(),
        ControllerAuthStatus::Unavailable => "host unavailable".to_owned(),
    }
}

pub(super) fn format_service_mode(snapshot: &ServiceModeSnapshot) -> String {
    let platform = match snapshot.platform {
        ServiceModePlatform::WindowsService => "Windows Service",
        ServiceModePlatform::LinuxPolkit => "Linux Polkit",
        ServiceModePlatform::MacosLaunchd => "macOS launchd",
        ServiceModePlatform::Unsupported => "Unsupported host",
    };
    let state = match snapshot.state {
        ServiceModeState::Ready => "ready",
        ServiceModeState::InstalledStopped => "installed · stopped",
        ServiceModeState::NotInstalled => "not installed",
        ServiceModeState::MissingPrivilege => "missing privilege",
        ServiceModeState::Unavailable => "unavailable",
        ServiceModeState::Unsupported => "unsupported",
    };
    format!("{platform} · {state}")
}

pub(super) fn format_port_conflicts(snapshot: &PortConflictSnapshot) -> String {
    if snapshot.conflicts.is_empty() {
        return "not probed".to_owned();
    }
    snapshot
        .conflicts
        .iter()
        .map(|conflict| {
            let status = if conflict.available {
                "available"
            } else {
                "occupied"
            };
            let owner = conflict.owner_pid.map_or_else(
                || "unknown owner".to_owned(),
                |pid| {
                    let name = conflict
                        .owner_name
                        .as_deref()
                        .map(crate::utils::sanitize_ui_text)
                        .unwrap_or_else(|| "unknown".to_owned());
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

pub(super) fn format_core_resources(snapshot: &CoreResourceSnapshot) -> String {
    let memory = snapshot.memory_bytes.map_or_else(
        || "memory=?".to_owned(),
        |bytes| format!("memory={:.1} MiB", bytes as f64 / 1_048_576.0),
    );
    let cpu = snapshot.cpu_percent.map_or_else(
        || "cpu=?".to_owned(),
        |percent| format!("cpu={percent:.1}%"),
    );
    let gc = match &snapshot.gc {
        CoreGcStatus::Unknown => "gc=not sampled".to_owned(),
        CoreGcStatus::NotNeeded => "gc=not needed".to_owned(),
        CoreGcStatus::Triggered { after_bytes, .. } => after_bytes.map_or_else(
            || "gc=triggered".to_owned(),
            |bytes| format!("gc=triggered, after={:.1} MiB", bytes as f64 / 1_048_576.0),
        ),
        CoreGcStatus::Failed { failure } => format!(
            "gc=failed ({})",
            crate::utils::sanitize_ui_text(&failure.message)
        ),
        CoreGcStatus::Unsupported => "gc=unsupported".to_owned(),
    };
    format!("{memory} · {cpu} · limit=512 MiB · {gc}")
}

pub(super) fn format_offline_startup(snapshot: &OfflineStartupSnapshot) -> String {
    let state = match snapshot.state {
        OfflineStartupState::Unknown => "not probed",
        OfflineStartupState::Checking => "checking",
        OfflineStartupState::Ready => "offline-ready",
        OfflineStartupState::Degraded => "offline-ready · degraded",
        OfflineStartupState::Blocked => "blocked",
    };
    let config = if snapshot.config_valid {
        "config=valid"
    } else {
        "config=invalid"
    };
    let binary = if snapshot.binary_available {
        "core=available"
    } else {
        "core=missing"
    };
    let geoip = match snapshot.geoip {
        LocalAssetStatus::NotRequired => "geoip=not-required",
        LocalAssetStatus::Available => "geoip=local",
        LocalAssetStatus::Missing => "geoip=missing",
    };
    let remote = match snapshot.remote_dependency {
        StartupRemoteDependency::Optional => "remote=optional",
    };
    let failure = snapshot
        .failure
        .as_ref()
        .map_or_else(String::new, |failure| {
            format!(" · {}", crate::utils::sanitize_ui_text(&failure.message))
        });
    format!("offline-first · {state} · {config} · {binary} · {geoip} · {remote}{failure}")
}
