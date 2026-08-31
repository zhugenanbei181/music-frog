//! Per-app routing surface: routing mode and package selection lists stored
//! via infiltrator-core's app-routing config.

use infiltrator_core::app_routing::{
    load_app_routing, save_app_routing,
    };

use crate::ffi::{FfiErrorCode, FfiStatus};

// --- App Routing API ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AppRoutingMode {
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

impl From<infiltrator_core::app_routing::AppRoutingMode> for AppRoutingMode {
    fn from(mode: infiltrator_core::app_routing::AppRoutingMode) -> Self {
        match mode {
            infiltrator_core::app_routing::AppRoutingMode::ProxyAll => AppRoutingMode::ProxyAll,
            infiltrator_core::app_routing::AppRoutingMode::ProxySelected => AppRoutingMode::ProxySelected,
            infiltrator_core::app_routing::AppRoutingMode::BypassSelected => AppRoutingMode::BypassSelected,
        }
    }
}

impl From<AppRoutingMode> for infiltrator_core::app_routing::AppRoutingMode {
    fn from(mode: AppRoutingMode) -> Self {
        match mode {
            AppRoutingMode::ProxyAll => infiltrator_core::app_routing::AppRoutingMode::ProxyAll,
            AppRoutingMode::ProxySelected => infiltrator_core::app_routing::AppRoutingMode::ProxySelected,
            AppRoutingMode::BypassSelected => infiltrator_core::app_routing::AppRoutingMode::BypassSelected,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingConfig {
    pub mode: AppRoutingMode,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingResult {
    pub status: FfiStatus,
    pub config: Option<AppRoutingConfig>,
}

#[uniffi::export]
pub fn app_routing_load() -> AppRoutingResult {
    match load_app_routing() {
        Ok(config) => AppRoutingResult {
            status: FfiStatus::ok(),
            config: Some(AppRoutingConfig {
                mode: config.mode.into(),
                packages: config.packages.into_iter().collect(),
            }),
        },
        Err(e) => AppRoutingResult {
            status: FfiStatus::err(FfiErrorCode::Io, e.to_string()),
            config: None,
        },
    }
}

#[uniffi::export]
pub fn app_routing_save(mode: AppRoutingMode, packages: Vec<String>) -> FfiStatus {
    let config = infiltrator_core::app_routing::AppRoutingConfig {
        mode: mode.into(),
        packages: packages.into_iter().collect(),
    };
    match save_app_routing(&config) {
        Ok(_) => FfiStatus::ok(),
        Err(e) => FfiStatus::err(FfiErrorCode::Io, e.to_string()),
    }
}

#[uniffi::export]
pub fn app_routing_set_mode(mode: AppRoutingMode) -> FfiStatus {
    match infiltrator_core::app_routing::set_routing_mode(mode.into()) {
        Ok(_) => FfiStatus::ok(),
        Err(e) => FfiStatus::err(FfiErrorCode::Io, e.to_string()),
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingToggleResult {
    pub status: FfiStatus,
    pub is_selected: bool,
}

#[uniffi::export]
pub fn app_routing_toggle_package(package: String) -> AppRoutingToggleResult {
    match infiltrator_core::app_routing::toggle_package(&package) {
        Ok(is_selected) => AppRoutingToggleResult {
            status: FfiStatus::ok(),
            is_selected,
        },
        Err(e) => AppRoutingToggleResult {
            status: FfiStatus::err(FfiErrorCode::Io, e.to_string()),
            is_selected: false,
        },
    }
}

#[uniffi::export]
pub fn app_routing_get_allowed_packages() -> Vec<String> {
    match load_app_routing() {
        Ok(config) => config.get_allowed_packages().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}
