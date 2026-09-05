//! Per-app routing surface: Android-only presentation plus application-owned
//! routing preferences.

use crate::host_support::{build_routing_application, map_application_failure};
use crate::ffi::FfiStatus;

// --- App Routing API ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AppRoutingMode {
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

impl From<infiltrator_domain::app_routing::AppRoutingMode> for AppRoutingMode {
    fn from(mode: infiltrator_domain::app_routing::AppRoutingMode) -> Self {
        match mode {
            infiltrator_domain::app_routing::AppRoutingMode::ProxyAll => AppRoutingMode::ProxyAll,
            infiltrator_domain::app_routing::AppRoutingMode::ProxySelected => {
                AppRoutingMode::ProxySelected
            }
            infiltrator_domain::app_routing::AppRoutingMode::BypassSelected => {
                AppRoutingMode::BypassSelected
            }
        }
    }
}

impl From<AppRoutingMode> for infiltrator_domain::app_routing::AppRoutingMode {
    fn from(mode: AppRoutingMode) -> Self {
        match mode {
            AppRoutingMode::ProxyAll => {
                infiltrator_domain::app_routing::AppRoutingMode::ProxyAll
            }
            AppRoutingMode::ProxySelected => {
                infiltrator_domain::app_routing::AppRoutingMode::ProxySelected
            }
            AppRoutingMode::BypassSelected => {
                infiltrator_domain::app_routing::AppRoutingMode::BypassSelected
            }
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
    let application = match build_routing_application() {
        Ok(application) => application,
        Err(status) => {
            return AppRoutingResult {
                status,
                config: None,
            };
        }
    };
    match application.load() {
        Ok(config) => AppRoutingResult {
            status: FfiStatus::ok(),
            config: Some(AppRoutingConfig {
                mode: config.mode.into(),
                packages: config.packages.into_iter().collect(),
            }),
        },
        Err(failure) => AppRoutingResult {
            status: map_application_failure(failure),
            config: None,
        },
    }
}

#[uniffi::export]
pub fn app_routing_save(mode: AppRoutingMode, packages: Vec<String>) -> FfiStatus {
    let config = infiltrator_domain::app_routing::AppRoutingConfig {
        mode: mode.into(),
        packages: packages.into_iter().collect(),
        ..infiltrator_domain::app_routing::AppRoutingConfig::default()
    };
    match build_routing_application().and_then(|application| {
        application
            .save(&config)
            .map_err(map_application_failure)
    }) {
        Ok(_) => FfiStatus::ok(),
        Err(status) => status,
    }
}

#[uniffi::export]
pub fn app_routing_set_mode(mode: AppRoutingMode) -> FfiStatus {
    match build_routing_application().and_then(|application| {
        application
            .set_mode(mode.into())
            .map_err(map_application_failure)
    }) {
        Ok(_) => FfiStatus::ok(),
        Err(status) => status,
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingToggleResult {
    pub status: FfiStatus,
    pub is_selected: bool,
}

#[uniffi::export]
pub fn app_routing_toggle_package(package: String) -> AppRoutingToggleResult {
    let result = build_routing_application().and_then(|application| {
        application
            .toggle_package(&package)
            .map_err(map_application_failure)
    });
    match result {
        Ok(is_selected) => AppRoutingToggleResult {
            status: FfiStatus::ok(),
            is_selected,
        },
        Err(status) => AppRoutingToggleResult {
            status,
            is_selected: false,
        },
    }
}

#[uniffi::export]
pub fn app_routing_get_allowed_packages() -> Vec<String> {
    match build_routing_application().and_then(|application| {
        application
            .allowed_packages()
            .map_err(map_application_failure)
    }) {
        Ok(packages) => packages.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Semantic categorization of Android applications for UI grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidAppCategory {
    System,
    Browser,
    Game,
    Social,
    Media,
    Productivity,
    Tool,
    Other,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidAppInfo {
    pub package_name: String,
    pub label: String,
    pub is_system_app: bool,
    pub category: AndroidAppCategory,
    pub uid: u32,
    pub user_id: u32,
    pub is_dual_app: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MobileCloudProvider {
    ICloud,
    GoogleDrive,
    CustomWebDav,
}

/// Resolved VPN split tunneling plan ready for `VpnService.Builder` application.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AndroidVpnPerAppPlan {
    pub mode: AppRoutingMode,
    pub allowed_packages: Vec<String>,
    pub disallowed_packages: Vec<String>,
    pub self_package_excluded: bool,
    pub total_selected_count: u32,
    pub warnings: Vec<String>,
}

/// Classifies an Android package into a high-level category by its package identifier.
#[uniffi::export]
pub fn app_routing_classify_package(package_name: String) -> AndroidAppCategory {
    let p = package_name.trim().to_ascii_lowercase();

    // Browsers
    if p.contains("chrome")
        || p.contains("browser")
        || p.contains("firefox")
        || p.contains("opera")
        || p.contains("via")
        || p.contains("edge")
        || p.contains("duckduckgo")
    {
        return AndroidAppCategory::Browser;
    }

    // Social & Communication
    if p.contains("tencent.mm")
        || p.contains("mobileqq")
        || p.contains("telegram")
        || p.contains("discord")
        || p.contains("whatsapp")
        || p.contains("signal")
        || p.contains("twitter")
        || p.contains("instagram")
        || p.contains("facebook")
        || p.contains("feishu")
        || p.contains("dingtalk")
    {
        return AndroidAppCategory::Social;
    }

    // Media & Streaming
    if p.contains("youtube")
        || p.contains("netflix")
        || p.contains("spotify")
        || p.contains("netease.cloudmusic")
        || p.contains("qqmusic")
        || p.contains("bilibili")
        || p.contains("vlc")
        || p.contains("tiktok")
        || p.contains("douyin")
    {
        return AndroidAppCategory::Media;
    }

    // Games
    if p.contains("mihoyo")
        || p.contains("hoyoverse")
        || p.contains("game")
        || p.contains("tencent.tmgp")
        || p.contains("supercell")
        || p.contains("unity3d")
        || p.contains("epicgames")
    {
        return AndroidAppCategory::Game;
    }

    // Productivity & Office
    if p.contains("wps")
        || p.contains("office")
        || p.contains("notion")
        || p.contains("obsidian")
        || p.contains("docs")
        || p.contains("drive")
        || p.contains("github")
    {
        return AndroidAppCategory::Productivity;
    }

    // System core
    if p.starts_with("com.android.")
        || p.starts_with("android.")
        || p.starts_with("com.google.android.gms")
        || p.starts_with("com.google.android.gsf")
    {
        return AndroidAppCategory::System;
    }

    AndroidAppCategory::Other
}

/// Calculates UID for dual-app or secondary user accounts on Android.
/// Android formula: `userId * 100000 + appId`.
#[uniffi::export]
pub fn app_routing_calculate_dual_app_uid(user_id: u32, app_id: u32) -> u32 {
    user_id
        .saturating_mul(100_000)
        .saturating_add(app_id % 100_000)
}

/// Builds the comprehensive VpnService split tunneling plan based on the current
/// routing configuration, guaranteeing self-package exclusion to prevent traffic loops.
#[uniffi::export]
pub fn app_routing_build_vpn_plan(self_package: String) -> AndroidVpnPerAppPlan {
    let clean_self = self_package.trim().to_string();
    let config = build_routing_application()
        .ok()
        .and_then(|application| application.load().ok())
        .unwrap_or_default();
    let mut warnings = Vec::new();

    let mut allowed_packages = Vec::new();
    let mut disallowed_packages = Vec::new();
    let mut self_package_excluded = false;

    match config.mode {
        infiltrator_domain::app_routing::AppRoutingMode::ProxyAll => {
            // In ProxyAll mode, we explicitly disallow our own package so the VPN daemon
            // traffic goes straight to upstream sockets without looping.
            if !clean_self.is_empty() {
                disallowed_packages.push(clean_self);
                self_package_excluded = true;
            }
        }
        infiltrator_domain::app_routing::AppRoutingMode::ProxySelected => {
            for pkg in &config.packages {
                let p = pkg.trim();
                if !p.is_empty() {
                    if !clean_self.is_empty() && p == clean_self {
                        warnings.push(format!(
                            "Self package '{clean_self}' cannot be proxied through VPN tun to prevent traffic recursion; excluded."
                        ));
                    } else {
                        allowed_packages.push(p.to_string());
                    }
                }
            }
            if !clean_self.is_empty() {
                self_package_excluded = true;
            }
        }
        infiltrator_domain::app_routing::AppRoutingMode::BypassSelected => {
            for pkg in &config.packages {
                let p = pkg.trim();
                if !p.is_empty() {
                    disallowed_packages.push(p.to_string());
                }
            }
            if !clean_self.is_empty() && !disallowed_packages.contains(&clean_self) {
                disallowed_packages.push(clean_self);
                self_package_excluded = true;
            }
        }
    }

    allowed_packages.sort();
    disallowed_packages.sort();

    let total_selected_count = config.packages.len() as u32;

    AndroidVpnPerAppPlan {
        mode: config.mode.into(),
        allowed_packages,
        disallowed_packages,
        self_package_excluded,
        total_selected_count,
        warnings,
    }
}

#[uniffi::export]
pub fn app_routing_format_sample_app() -> AndroidAppInfo {
    AndroidAppInfo {
        package_name: "com.example.app".to_string(),
        label: "Sample App".to_string(),
        is_system_app: false,
        category: AndroidAppCategory::Other,
        uid: 10123,
        user_id: 0,
        is_dual_app: false,
    }
}

#[cfg(test)]
mod tests_app_info {
    use super::*;

    #[test]
    fn test_android_app_info_and_cloud_provider() {
        let app = app_routing_format_sample_app();
        assert_eq!(app.package_name, "com.example.app");
        assert_eq!(app.label, "Sample App");
        assert!(!app.is_system_app);
        assert_eq!(app.category, AndroidAppCategory::Other);
        assert_eq!(app.uid, 10123);
        assert_eq!(app.user_id, 0);
        assert!(!app.is_dual_app);

        let provider = MobileCloudProvider::GoogleDrive;
        assert_eq!(provider, MobileCloudProvider::GoogleDrive);
    }

    #[test]
    fn test_app_routing_classify_package() {
        assert_eq!(
            app_routing_classify_package("com.android.chrome".to_string()),
            AndroidAppCategory::Browser
        );
        assert_eq!(
            app_routing_classify_package("org.mozilla.firefox".to_string()),
            AndroidAppCategory::Browser
        );
        assert_eq!(
            app_routing_classify_package("org.telegram.messenger".to_string()),
            AndroidAppCategory::Social
        );
        assert_eq!(
            app_routing_classify_package("com.tencent.mm".to_string()),
            AndroidAppCategory::Social
        );
        assert_eq!(
            app_routing_classify_package("com.google.android.youtube".to_string()),
            AndroidAppCategory::Media
        );
        assert_eq!(
            app_routing_classify_package("com.spotify.music".to_string()),
            AndroidAppCategory::Media
        );
        assert_eq!(
            app_routing_classify_package("com.miHoYo.GenshinImpact".to_string()),
            AndroidAppCategory::Game
        );
        assert_eq!(
            app_routing_classify_package("md.obsidian".to_string()),
            AndroidAppCategory::Productivity
        );
        assert_eq!(
            app_routing_classify_package("com.android.settings".to_string()),
            AndroidAppCategory::System
        );
        assert_eq!(
            app_routing_classify_package("com.custom.utility".to_string()),
            AndroidAppCategory::Other
        );
    }

    #[test]
    fn test_dual_app_uid_calculation() {
        assert_eq!(app_routing_calculate_dual_app_uid(0, 10123), 10123);
        assert_eq!(app_routing_calculate_dual_app_uid(10, 10123), 1_010_123);
        assert_eq!(app_routing_calculate_dual_app_uid(99, 10456), 9_910_456);
    }

    #[test]
    fn test_app_routing_build_vpn_plan() {
        // Default is ProxyAll
        let plan_all = app_routing_build_vpn_plan("com.infiltrator.app".to_string());
        assert_eq!(plan_all.mode, AppRoutingMode::ProxyAll);
        assert!(
            plan_all
                .disallowed_packages
                .contains(&"com.infiltrator.app".to_string())
        );
        assert!(plan_all.self_package_excluded);
    }
}
