//! Typed owner commands produced by the Slint surface callbacks.
//!
//! Every function in this module is the single implementation of one surface
//! behaviour. The Slint callbacks call these functions, so a test that drives
//! the surface and a test that calls the function observe exactly the same
//! command and the same typed failure. No surface rule may be re-implemented
//! outside this module.

pub mod workbench;

use crate::theme_util;
use crate::{CommandIntent, SlintAction, SurfaceSnapshot};
use infiltrator_contract::android_control::{
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceState,
};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::vpn::VpnSurfaceObservation;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundContractError;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::preferences::{
    OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
};
use lurker_core_contract::outbound::probe::{
    OutboundProbeBatchScope, OutboundProbeConcurrency, OutboundSelectionRequest,
    ProbeOutboundBatchRequest, ProbeOutboundRequest,
};
use lurker_core_contract::routing_intent::RoutingIntent;

/// The message the surface shows when the accent colour is not a six digit
/// hexadecimal colour.
pub const INVALID_ACCENT_COLOR: &str = "Enter a six-digit hexadecimal color such as #016888";

/// Maps a routing mode label to the shared routing intent command.
pub fn route_mode_action(mode: &str) -> Result<SlintAction, Failure> {
    let intent = match mode {
        "Rule" => RoutingIntent::FollowRules,
        "Global" => RoutingIntent::RouteAllThroughProxy,
        "Direct" => RoutingIntent::BypassProxy,
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown routing mode",
                false,
            ));
        }
    };
    Ok(SlintAction::Command(CommandIntent::SetRoutingIntent {
        intent,
    }))
}

/// Maps a network mode label to the owner command sequence that switches the
/// desktop capture mode while preserving the running state.
pub fn network_mode_action(
    snapshot: &SurfaceSnapshot,
    requested: &str,
) -> Result<SlintAction, Failure> {
    if cfg!(target_os = "android") {
        return if requested != "VPN" {
            Err(Failure::unsupported(
                "Android Slint supports the owner-managed VPN mode only",
            ))
        } else {
            match &snapshot.vpn {
                VpnSurfaceObservation::AndroidOwner(owner) if owner.is_running() => {
                    Ok(SlintAction::Command(CommandIntent::StopVpn))
                }
                VpnSurfaceObservation::AndroidOwner(owner)
                    if owner.readiness == VpnOwnerReadiness::CleanupPending
                        || owner.resources.iter().any(|resource| {
                            matches!(
                                &resource.state,
                                VpnOwnerResourceState::CleanupPending { .. }
                            )
                        }) =>
                {
                    Ok(SlintAction::Command(CommandIntent::StopVpn))
                }
                VpnSurfaceObservation::AndroidOwner(owner) => match &owner.vpn_state {
                    VpnOwnerLifecycle::PermissionRequired
                    | VpnOwnerLifecycle::Stopped
                    | VpnOwnerLifecycle::Revoked
                    | VpnOwnerLifecycle::Failed { .. } => {
                        Ok(SlintAction::Command(CommandIntent::StartVpn))
                    }
                    VpnOwnerLifecycle::Unavailable { failure_message } => {
                        Err(Failure::unsupported(failure_message.clone()))
                    }
                    VpnOwnerLifecycle::StartRequested
                    | VpnOwnerLifecycle::Starting
                    | VpnOwnerLifecycle::Foreground
                    | VpnOwnerLifecycle::TunnelEstablished
                    | VpnOwnerLifecycle::Running
                    | VpnOwnerLifecycle::Stopping => Err(Failure::new(
                        ErrorCode::NotReady,
                        "VPN owner is in a transition and cannot accept a duplicate mode change",
                        true,
                    )),
                },
                VpnSurfaceObservation::Unsupported { reason } => {
                    Err(Failure::unsupported(reason.clone()))
                }
                VpnSurfaceObservation::NotApplicable => Err(Failure::unsupported(
                    "VPN is not applicable to this surface",
                )),
                VpnSurfaceObservation::Unavailable { failure } => Err(failure.clone()),
            }
        };
    }

    let settings = snapshot.pages.settings.data.as_ref();
    let tun_enabled = settings
        .and_then(|settings| settings.tun_enabled)
        .unwrap_or(false);
    let proxy_enabled = matches!(&snapshot.system_proxy.status, SystemProxyStatus::Enabled);
    let running = matches!(
        &snapshot.core.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    );
    let mut commands = Vec::new();
    if running {
        commands.push(CommandIntent::StopProxy);
    }
    match requested {
        "System Proxy" => {
            if tun_enabled {
                commands.push(CommandIntent::ToggleTun { enabled: false });
            }
            commands.push(CommandIntent::SetSystemProxy { enabled: true });
        }
        "TUN" => {
            if proxy_enabled {
                commands.push(CommandIntent::SetSystemProxy { enabled: false });
            }
            commands.push(CommandIntent::ToggleTun { enabled: true });
        }
        "Core only" => {
            if proxy_enabled {
                commands.push(CommandIntent::SetSystemProxy { enabled: false });
            }
            if tun_enabled {
                commands.push(CommandIntent::ToggleTun { enabled: false });
            }
        }
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown network mode",
                false,
            ));
        }
    }
    if running {
        commands.push(CommandIntent::StartProxy);
    }
    Ok(if commands.len() == 1 {
        SlintAction::Command(commands.remove(0))
    } else {
        SlintAction::Sequence(commands)
    })
}

/// Builds the owner command that selects one outbound inside a collection.
pub fn selection_action(
    snapshot: &SurfaceSnapshot,
    group: &str,
    node: &str,
) -> Result<SlintAction, Failure> {
    let inventory = snapshot.outbound_inventory.data.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is unavailable",
            true,
        )
    })?;
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is not ready",
            true,
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| collection.id.value() == group)
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy group", false))?;
    let outbound = entries
        .iter()
        .find(|entry| entry.id.value() == node)
        .map(|entry| entry.id.clone())
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy node", false))?;
    let request =
        OutboundSelectionRequest::new(collection.id.clone(), outbound, inventory.revision())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(SlintAction::Command(CommandIntent::SelectOutbound {
        request,
    }))
}

/// Builds the owner command that updates the outbound presentation preferences.
pub fn outbound_preference_action(
    snapshot: &SurfaceSnapshot,
    intent: OutboundUiPreferenceIntent,
) -> Result<SlintAction, Failure> {
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_ref()
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Outbound presentation preferences are not available",
                true,
            )
        })?;
    let request = OutboundUiPreferenceRequest::new(preferences.revision(), intent)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(SlintAction::Command(
        CommandIntent::ApplyOutboundUiPreference { request },
    ))
}

/// Maps a proxy sort label to the shared outbound presentation preference.
pub fn proxy_sort_action(sort: &str) -> Result<OutboundUiPreferenceIntent, Failure> {
    let order = match sort {
        "latency_asc" => OutboundSortOrder::LatencyAscending,
        "latency_desc" => OutboundSortOrder::LatencyDescending,
        "name_asc" => OutboundSortOrder::NameAscending,
        "name_desc" => OutboundSortOrder::NameDescending,
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown proxy sort order",
                false,
            ));
        }
    };
    Ok(OutboundUiPreferenceIntent::SetSortOrder(order))
}

/// Builds the owner command that probes one outbound or a whole collection.
pub fn probe_action(
    snapshot: &SurfaceSnapshot,
    group: &str,
    node: &str,
) -> Result<SlintAction, Failure> {
    use infiltrator_contract::runtime_settings::RuntimeProbeSettingsState;

    let inventory = snapshot.outbound_inventory.data.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is unavailable",
            true,
        )
    })?;
    let settings = match &snapshot.probe_settings {
        RuntimeProbeSettingsState::Available { settings } => settings,
        RuntimeProbeSettingsState::Unavailable { failure } => return Err(failure.clone()),
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Outbound inventory is not ready",
            true,
        ));
    };
    let map_error = |error: OutboundContractError| {
        Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
    };
    if node.is_empty() {
        let collection = collections
            .iter()
            .find(|collection| collection.id.value() == group)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy group", false))?;
        let concurrency = OutboundProbeConcurrency::new(8).map_err(map_error)?;
        let request = ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::Collection(collection.id.clone()),
            inventory.revision(),
            settings.url(),
            settings.timeout_ms() as u64,
            concurrency,
        )
        .map_err(map_error)?;
        return Ok(SlintAction::Command(CommandIntent::ProbeOutboundBatch {
            request,
        }));
    }
    let outbound = entries
        .iter()
        .find(|entry| entry.id.value() == node)
        .map(|entry| entry.id.clone())
        .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Unknown proxy node", false))?;
    let request = ProbeOutboundRequest::new(
        outbound,
        inventory.revision(),
        settings.url(),
        settings.timeout_ms() as u64,
    )
    .map_err(map_error)?;
    Ok(SlintAction::Command(CommandIntent::ProbeOutbound {
        request,
    }))
}

/// Maps a log level filter label to the shared log filter command.
pub fn log_level_action(level: &str) -> CommandIntent {
    CommandIntent::SetLogLevelFilter {
        level: if level == "all" {
            None
        } else {
            Some(level.to_owned())
        },
    }
}

/// Builds the owner command that toggles one observed rule.
///
/// `None` means the observed rule already carries the requested state, so the
/// surface must not ask the owner to change anything.
pub fn rule_enabled_action(
    snapshot: &SurfaceSnapshot,
    index: &str,
    enabled: bool,
) -> Result<Option<CommandIntent>, Failure> {
    let index = index
        .parse::<usize>()
        .map_err(|_| Failure::new(ErrorCode::InvalidInput, "Invalid rule identity", false))?;
    let rule = snapshot
        .pages
        .rules
        .data
        .as_ref()
        .and_then(|page| page.rules.iter().find(|rule| rule.id == index))
        .ok_or_else(|| Failure::new(ErrorCode::NotReady, "Rule was not observed", true))?;
    if rule.is_enabled == enabled {
        return Ok(None);
    }
    Ok(Some(CommandIntent::ToggleRuleEnabled { index }))
}

/// Maps a theme mode label to the shared theme setting command.
pub fn theme_mode_action(mode: &str) -> Result<SlintAction, Failure> {
    let intent = match mode {
        "system" | "light" | "dark" | "forest" | "amoled" => CommandIntent::UpdateSetting {
            key: "theme".to_owned(),
            value: mode.to_owned(),
        },
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown theme mode",
                false,
            ));
        }
    };
    Ok(SlintAction::Command(intent))
}

/// Maps an accent colour to the shared accent setting command.
pub fn theme_color_action(hex: &str) -> Result<SlintAction, Failure> {
    if theme_util::parse_color(hex).is_none() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            INVALID_ACCENT_COLOR,
            false,
        ));
    }
    Ok(SlintAction::Command(CommandIntent::UpdateSetting {
        key: "custom_accent_color".to_owned(),
        value: hex.to_owned(),
    }))
}
