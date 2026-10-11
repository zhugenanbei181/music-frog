//! Android per-app routing projections for the unique `:vpn` Core owner.

use super::owner_operation::app_routing_revision;
use super::owner_operation::begin_vpn_routing_activation;
use super::owner_operation::finish_vpn_routing_activation;
use super::owner_operation::try_lock_app_routing;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_routing_application, map_application_failure};
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_domain::app_routing::{self, AppRoutingConfig, AppRoutingPackageSelection};
use mihomo_platform::android_bridge::get_android_bridge;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidAppRoutingMode {
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

impl From<app_routing::AppRoutingMode> for AndroidAppRoutingMode {
    fn from(mode: app_routing::AppRoutingMode) -> Self {
        match mode {
            app_routing::AppRoutingMode::ProxyAll => Self::ProxyAll,
            app_routing::AppRoutingMode::ProxySelected => Self::ProxySelected,
            app_routing::AppRoutingMode::BypassSelected => Self::BypassSelected,
        }
    }
}

impl From<AndroidAppRoutingMode> for app_routing::AppRoutingMode {
    fn from(mode: AndroidAppRoutingMode) -> Self {
        match mode {
            AndroidAppRoutingMode::ProxyAll => Self::ProxyAll,
            AndroidAppRoutingMode::ProxySelected => Self::ProxySelected,
            AndroidAppRoutingMode::BypassSelected => Self::BypassSelected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidAppRoutingPackageSelection {
    Selected,
    Unselected,
}

impl From<AndroidAppRoutingPackageSelection> for AppRoutingPackageSelection {
    fn from(selection: AndroidAppRoutingPackageSelection) -> Self {
        match selection {
            AndroidAppRoutingPackageSelection::Selected => Self::Selected,
            AndroidAppRoutingPackageSelection::Unselected => Self::Unselected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidAppRoutingSnapshot {
    pub mode: AndroidAppRoutingMode,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidAppRoutingResult {
    pub status: FfiStatus,
    pub snapshot: Option<AndroidAppRoutingSnapshot>,
}

/// Service-local projection used while assembling a VpnService.Builder.
/// Product UI must use `AndroidOwnerOperation::AppRoutingSnapshot` instead.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AndroidVpnRoutingSnapshot {
    pub result: AndroidAppRoutingResult,
    pub routing_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AndroidVpnRoutingActivationLease {
    pub lease_id: u64,
    pub routing_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidVpnRoutingActivationResult {
    Acquired {
        lease: AndroidVpnRoutingActivationLease,
    },
    Stale {
        observed_routing_revision: u64,
    },
    Busy,
    Failed {
        status: FfiStatus,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidVpnRoutingActivationReleaseResult {
    Released,
    LeaseMismatch,
    LeaseMissing,
    Failed { status: FfiStatus },
}

fn routing_snapshot(config: AppRoutingConfig) -> AndroidAppRoutingSnapshot {
    let mut packages = config.packages.into_iter().collect::<Vec<_>>();
    packages.sort();
    AndroidAppRoutingSnapshot {
        mode: config.mode.into(),
        packages,
    }
}

pub(super) fn load_android_app_routing() -> AndroidAppRoutingResult {
    let application = match build_routing_application() {
        Ok(application) => application,
        Err(status) => {
            return AndroidAppRoutingResult {
                status,
                snapshot: None,
            };
        }
    };
    match application.load() {
        Ok(config) => AndroidAppRoutingResult {
            status: FfiStatus::ok(),
            snapshot: Some(routing_snapshot(config)),
        },
        Err(failure) => AndroidAppRoutingResult {
            status: map_application_failure(failure),
            snapshot: None,
        },
    }
}

pub(super) fn set_android_app_routing_mode(mode: AndroidAppRoutingMode) -> FfiStatus {
    match build_routing_application().and_then(|application| {
        application
            .set_mode(mode.into())
            .map_err(map_application_failure)
    }) {
        Ok(()) => FfiStatus::ok(),
        Err(status) => status,
    }
}

pub(super) fn set_android_app_routing_packages(
    packages: Vec<String>,
    selection: AndroidAppRoutingPackageSelection,
) -> FfiStatus {
    match build_routing_application().and_then(|application| {
        application
            .set_packages(&packages, selection.into())
            .map_err(map_application_failure)
    }) {
        Ok(()) => FfiStatus::ok(),
        Err(status) => status,
    }
}

/// Reads routing only inside the Android Core-owner process. The operation
/// serializer prevents a routing mutation from overlapping this read; a busy
/// owner fails closed so VPN construction can be retried with a fresh snapshot.
#[uniffi::export]
pub fn android_vpn_routing_snapshot() -> AndroidVpnRoutingSnapshot {
    let failure = |status| AndroidVpnRoutingSnapshot {
        result: AndroidAppRoutingResult {
            status,
            snapshot: None,
        },
        routing_revision: app_routing_revision(),
    };

    let Some(bridge) = get_android_bridge() else {
        return failure(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner bridge is unavailable",
        ));
    };
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return failure(FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN routing can only be read in the :vpn Core owner process",
        ));
    }
    let Ok(_routing_operation) = try_lock_app_routing() else {
        return failure(FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN routing is being updated; retry after the owner operation completes",
        ));
    };

    AndroidVpnRoutingSnapshot {
        result: load_android_app_routing(),
        routing_revision: app_routing_revision(),
    }
}

/// Fences per-app routing changes across Android tunnel establishment and
/// Rust data-path startup. The lease is acquired only by the unique Core owner.
#[uniffi::export]
pub fn android_begin_vpn_routing_activation(
    expected_routing_revision: u64,
) -> AndroidVpnRoutingActivationResult {
    let Some(bridge) = get_android_bridge() else {
        return AndroidVpnRoutingActivationResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner bridge is unavailable",
            ),
        };
    };
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return AndroidVpnRoutingActivationResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::NotReady,
                "VPN routing activation can only be leased by the :vpn Core owner",
            ),
        };
    }
    let Ok(_routing_operation) = try_lock_app_routing() else {
        return AndroidVpnRoutingActivationResult::Busy;
    };
    begin_vpn_routing_activation(expected_routing_revision)
}

/// Releases a lease acquired for a particular VPN start transaction.
#[uniffi::export]
pub fn android_finish_vpn_routing_activation(
    lease_id: u64,
) -> AndroidVpnRoutingActivationReleaseResult {
    finish_vpn_routing_activation(lease_id)
}
