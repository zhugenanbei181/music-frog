//! The one typed page model of the overview page, shared by every surface.
//!
//! The overview page shows two facts that used to be derived privately: the
//! routing mode (rule, global, direct) and the traffic mode (VPN, TUN,
//! system proxy, core only) together with its status and its warning. Slint
//! re-derived all four from the raw snapshot while the shared
//! [`crate::routing_intent_projection`] and [`crate::system_toggle_projection`]
//! already owned the underlying facts.
//!
//! This module folds those shared projections into the page's own state. The
//! model never reads a raw snapshot field to answer a question the shared
//! projections already answer, and it never guesses: an unobserved mode is
//! `Unknown`, not "disabled".

use crate::routing_intent_projection;
use crate::system_toggle_projection;
use infiltrator_contract::android_control::{
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceState,
};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_toggle::{SystemToggleSnapshot, SystemToggleState};
use infiltrator_contract::vpn::VpnSurfaceObservation;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};

/// The host the overview page renders for.
///
/// The traffic mode depends on the host: a desktop host redirects traffic
/// through the TUN device or the system proxy, an Android host through the
/// VPN service. The host is an input, not a compile-time decision, so every
/// surface answers with the same rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewHost {
    /// A desktop host.
    #[default]
    Desktop,
    /// An Android host.
    Android,
}

/// The typed run state of the overview page.
///
/// The reason rides the variant: a page can no longer show an unavailable
/// core without saying why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OverviewRunState {
    /// The core is reachable and proxying traffic.
    #[default]
    Running,
    /// The core is reachable but not proxying.
    Stopped,
    /// The core could not be read at all.
    Unavailable {
        /// Why the core could not be read.
        reason: String,
    },
}

/// The routing mode the overview page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewRoutingMode {
    /// Traffic follows the rule set.
    Rule,
    /// All traffic goes through the proxy.
    Global,
    /// Traffic bypasses the proxy.
    Direct,
    /// The engine owns the policy, or the owner has not reported one.
    #[default]
    Unknown,
}

impl OverviewRoutingMode {
    /// The routing mode the owner's routing intent expresses.
    pub fn from_intent(intent: &RoutingIntentSnapshot) -> Self {
        match intent.current.as_ref() {
            Some(RoutingIntent::FollowRules) => Self::Rule,
            Some(RoutingIntent::RouteAllThroughProxy) => Self::Global,
            Some(RoutingIntent::BypassProxy) => Self::Direct,
            Some(RoutingIntent::EngineDefined(_)) | None => Self::Unknown,
        }
    }
}

/// The traffic mode the host actually applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewNetworkMode {
    /// The Android VPN service carries the traffic.
    Vpn,
    /// The TUN device carries the traffic.
    Tun,
    /// The host system proxy carries the traffic.
    SystemProxy,
    /// Only the local proxy port is open; the host is not redirected.
    CoreOnly,
    /// The owner has not reported a mode.
    #[default]
    Unknown,
}

/// The typed status of the traffic mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OverviewNetworkStatus {
    /// The Android VPN service is connected.
    VpnConnected,
    /// The Android VPN service is starting.
    VpnStarting,
    /// The Android VPN service is stopping.
    VpnStopping,
    /// The Android VPN service is not running.
    VpnStopped,
    /// The Android VPN consent is still missing.
    VpnPermissionRequired,
    /// The Android VPN consent was revoked.
    VpnRevoked,
    /// The Android VPN owner failed.
    VpnFailed {
        /// Why the VPN owner failed.
        reason: String,
    },
    /// The Android VPN owner could not be read.
    VpnUnavailable {
        /// Why the VPN owner could not be read.
        reason: String,
    },
    /// The host does not offer the VPN service.
    VpnUnsupported {
        /// Why the host does not offer the VPN service.
        reason: String,
    },
    /// The host does not compose the VPN service at all.
    VpnNotApplicable,
    /// The TUN device is enabled.
    TunEnabled,
    /// The host system proxy is enabled.
    SystemProxyEnabled,
    /// The host system proxy is disabled.
    SystemProxyDisabled,
    /// The host system proxy state has not been observed.
    SystemProxyUnobserved,
    /// The host system proxy state could not be read.
    SystemProxyUnavailable {
        /// Why the host system proxy state could not be read.
        reason: String,
    },
    /// No traffic mode has been observed.
    #[default]
    Unknown,
}

/// Whether the overview page warns that the traffic mode is not confirmed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OverviewNetworkWarning {
    /// The owner confirmed the mode; nothing to warn about.
    #[default]
    Confirmed,
    /// The owner could not confirm the mode; the reason says why.
    Unconfirmed {
        /// Why the mode could not be confirmed.
        reason: String,
    },
}

impl OverviewNetworkWarning {
    /// Whether the page shows the warning.
    pub fn is_warned(&self) -> bool {
        matches!(self, Self::Unconfirmed { .. })
    }
}

/// The one page model of the overview page.
///
/// A surface renders the overview page from this model alone: the run state,
/// the routing mode, the traffic mode, its status and its warning come from
/// here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct OverviewPageModel {
    run_state: OverviewRunState,
    routing_mode: OverviewRoutingMode,
    network_mode: OverviewNetworkMode,
    network_status: OverviewNetworkStatus,
    network_warning: OverviewNetworkWarning,
    toggles: SystemToggleSnapshot,
    routing_intent: RoutingIntentSnapshot,
}

impl OverviewPageModel {
    /// Fold one accepted surface snapshot into the page model.
    pub fn observe(&mut self, host: OverviewHost, snapshot: &SurfaceSnapshot) {
        self.run_state = run_state(snapshot);
        self.toggles = system_toggle_projection::from_surface(snapshot);
        self.routing_intent = routing_intent_projection::from_surface(snapshot);
        self.routing_mode = OverviewRoutingMode::from_intent(&self.routing_intent);
        self.network_mode = network_mode(host, &self.toggles);
        self.network_status = network_status(host, &self.toggles, snapshot);
        self.network_warning = network_warning(host, &self.toggles, &self.network_status);
    }

    /// The typed run state of the page.
    pub fn run_state(&self) -> &OverviewRunState {
        &self.run_state
    }

    /// Whether the core is reachable and proxying traffic.
    pub fn is_running(&self) -> bool {
        matches!(&self.run_state, OverviewRunState::Running)
    }

    /// The routing mode the page shows.
    pub fn routing_mode(&self) -> OverviewRoutingMode {
        self.routing_mode
    }

    /// The traffic mode the page shows.
    pub fn network_mode(&self) -> OverviewNetworkMode {
        self.network_mode
    }

    /// The typed status of the traffic mode.
    pub fn network_status(&self) -> &OverviewNetworkStatus {
        &self.network_status
    }

    /// The warning the page shows for the traffic mode.
    pub fn network_warning(&self) -> &OverviewNetworkWarning {
        &self.network_warning
    }

    /// The shared system toggle read model the page renders.
    pub fn toggles(&self) -> &SystemToggleSnapshot {
        &self.toggles
    }

    /// The shared routing intent read model the page renders.
    pub fn routing_intent(&self) -> &RoutingIntentSnapshot {
        &self.routing_intent
    }
}

/// The run state the owner's lifecycle and failure express.
fn run_state(snapshot: &SurfaceSnapshot) -> OverviewRunState {
    match snapshot.core.lifecycle {
        CoreLifecycle::Running | CoreLifecycle::Ready => OverviewRunState::Running,
        CoreLifecycle::Stopped | CoreLifecycle::Starting | CoreLifecycle::Stopping => {
            OverviewRunState::Stopped
        }
        CoreLifecycle::Failed => OverviewRunState::Unavailable {
            reason: snapshot
                .core
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .or_else(|| {
                    snapshot
                        .core
                        .routing_intent
                        .failure
                        .as_ref()
                        .map(|failure| failure.message.clone())
                })
                .unwrap_or_else(|| "the core failed".to_owned()),
        },
    }
}

/// The traffic mode the shared toggle read model expresses.
fn network_mode(host: OverviewHost, toggles: &SystemToggleSnapshot) -> OverviewNetworkMode {
    if host == OverviewHost::Android {
        return OverviewNetworkMode::Vpn;
    }
    match toggles.tun {
        SystemToggleState::Enabled | SystemToggleState::Pending { desired: true } => {
            OverviewNetworkMode::Tun
        }
        _ => match toggles.system_proxy {
            SystemToggleState::Enabled | SystemToggleState::Pending { desired: true } => {
                OverviewNetworkMode::SystemProxy
            }
            SystemToggleState::Disabled => OverviewNetworkMode::CoreOnly,
            SystemToggleState::Unknown
            | SystemToggleState::Pending { desired: false }
            | SystemToggleState::Unsupported { .. }
            | SystemToggleState::Failed { .. } => OverviewNetworkMode::Unknown,
        },
    }
}

/// The typed status of the traffic mode.
fn network_status(
    host: OverviewHost,
    toggles: &SystemToggleSnapshot,
    snapshot: &SurfaceSnapshot,
) -> OverviewNetworkStatus {
    if host == OverviewHost::Android {
        return vpn_status(&snapshot.vpn);
    }
    match toggles.tun {
        SystemToggleState::Enabled => return OverviewNetworkStatus::TunEnabled,
        SystemToggleState::Pending { desired: true } => return OverviewNetworkStatus::TunEnabled,
        _ => {}
    }
    match &toggles.system_proxy {
        SystemToggleState::Enabled | SystemToggleState::Pending { desired: true } => {
            OverviewNetworkStatus::SystemProxyEnabled
        }
        SystemToggleState::Disabled | SystemToggleState::Pending { desired: false } => {
            OverviewNetworkStatus::SystemProxyDisabled
        }
        SystemToggleState::Unknown => OverviewNetworkStatus::SystemProxyUnobserved,
        SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
            OverviewNetworkStatus::SystemProxyUnavailable {
                reason: failure.message.clone(),
            }
        }
    }
}

/// The typed status of the Android VPN owner.
fn vpn_status(observation: &VpnSurfaceObservation) -> OverviewNetworkStatus {
    match observation {
        VpnSurfaceObservation::AndroidOwner(owner) => {
            if let Some(reason) =
                owner
                    .resources
                    .iter()
                    .find_map(|resource| match &resource.state {
                        VpnOwnerResourceState::CleanupPending {
                            failure_message, ..
                        } => Some(String::clone(failure_message)),
                        VpnOwnerResourceState::Absent | VpnOwnerResourceState::Held { .. } => None,
                    })
            {
                return OverviewNetworkStatus::VpnFailed { reason };
            }
            match owner.readiness {
                VpnOwnerReadiness::Unavailable => return OverviewNetworkStatus::Unknown,
                VpnOwnerReadiness::PermissionRequired => {
                    return OverviewNetworkStatus::VpnPermissionRequired;
                }
                VpnOwnerReadiness::Revoked => return OverviewNetworkStatus::VpnRevoked,
                VpnOwnerReadiness::Stopping | VpnOwnerReadiness::CleanupPending => {
                    return OverviewNetworkStatus::VpnStopping;
                }
                VpnOwnerReadiness::Stopped => return OverviewNetworkStatus::VpnStopped,
                VpnOwnerReadiness::Failed => return OverviewNetworkStatus::VpnStopped,
                VpnOwnerReadiness::AwaitingForeground
                | VpnOwnerReadiness::AwaitingTunnel
                | VpnOwnerReadiness::AwaitingDataPlane => {
                    return OverviewNetworkStatus::VpnStarting;
                }
                VpnOwnerReadiness::Ready => {}
            }
            match &owner.vpn_state {
                VpnOwnerLifecycle::PermissionRequired => {
                    OverviewNetworkStatus::VpnPermissionRequired
                }
                VpnOwnerLifecycle::Revoked => OverviewNetworkStatus::VpnRevoked,
                VpnOwnerLifecycle::Failed { failure_message } => OverviewNetworkStatus::VpnFailed {
                    reason: String::clone(failure_message),
                },
                VpnOwnerLifecycle::Unavailable { failure_message } => {
                    OverviewNetworkStatus::VpnUnavailable {
                        reason: String::clone(failure_message),
                    }
                }
                VpnOwnerLifecycle::Stopping => OverviewNetworkStatus::VpnStopping,
                VpnOwnerLifecycle::Stopped => OverviewNetworkStatus::VpnStopped,
                VpnOwnerLifecycle::Running => {
                    if owner.is_running() {
                        OverviewNetworkStatus::VpnConnected
                    } else {
                        OverviewNetworkStatus::VpnStarting
                    }
                }
                VpnOwnerLifecycle::StartRequested
                | VpnOwnerLifecycle::Starting
                | VpnOwnerLifecycle::Foreground
                | VpnOwnerLifecycle::TunnelEstablished => OverviewNetworkStatus::VpnStarting,
            }
        }
        VpnSurfaceObservation::Unsupported { reason } => OverviewNetworkStatus::VpnUnsupported {
            reason: String::clone(reason),
        },
        VpnSurfaceObservation::NotApplicable => OverviewNetworkStatus::VpnNotApplicable,
        VpnSurfaceObservation::Unavailable { failure } => OverviewNetworkStatus::VpnUnavailable {
            reason: failure.message.clone(),
        },
    }
}

/// The warning the traffic mode carries.
fn network_warning(
    host: OverviewHost,
    toggles: &SystemToggleSnapshot,
    status: &OverviewNetworkStatus,
) -> OverviewNetworkWarning {
    if host == OverviewHost::Android {
        return match status {
            OverviewNetworkStatus::VpnConnected
            | OverviewNetworkStatus::VpnStarting
            | OverviewNetworkStatus::VpnStopping
            | OverviewNetworkStatus::VpnStopped => OverviewNetworkWarning::Confirmed,
            OverviewNetworkStatus::VpnPermissionRequired => OverviewNetworkWarning::Unconfirmed {
                reason: "the VPN consent is still missing".to_owned(),
            },
            OverviewNetworkStatus::VpnRevoked => OverviewNetworkWarning::Unconfirmed {
                reason: "the VPN consent was revoked".to_owned(),
            },
            OverviewNetworkStatus::VpnFailed { reason }
            | OverviewNetworkStatus::VpnUnavailable { reason }
            | OverviewNetworkStatus::VpnUnsupported { reason } => {
                OverviewNetworkWarning::Unconfirmed {
                    reason: String::clone(reason),
                }
            }
            OverviewNetworkStatus::VpnNotApplicable => OverviewNetworkWarning::Unconfirmed {
                reason: "the VPN owner is not composed for this host".to_owned(),
            },
            OverviewNetworkStatus::TunEnabled
            | OverviewNetworkStatus::SystemProxyEnabled
            | OverviewNetworkStatus::SystemProxyDisabled
            | OverviewNetworkStatus::SystemProxyUnobserved
            | OverviewNetworkStatus::SystemProxyUnavailable { .. }
            | OverviewNetworkStatus::Unknown => OverviewNetworkWarning::Unconfirmed {
                reason: "the VPN owner reported no confirmed state".to_owned(),
            },
        };
    }
    match &toggles.system_proxy {
        SystemToggleState::Unknown => OverviewNetworkWarning::Unconfirmed {
            reason: "the host system proxy state has not been observed".to_owned(),
        },
        SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
            OverviewNetworkWarning::Unconfirmed {
                reason: failure.message.clone(),
            }
        }
        SystemToggleState::Enabled
        | SystemToggleState::Disabled
        | SystemToggleState::Pending { .. } => OverviewNetworkWarning::Confirmed,
    }
}

#[cfg(test)]
#[path = "overview_page_model_tests.rs"]
mod tests;
