//! Runtime-neutral application settings schema.
//!
//! This module contains only serializable configuration values and defaults.
//! TOML/filesystem persistence and keyring hydration live in
//! `infiltrator-core::settings_io`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    /// Password is an in-memory projection only and is never serialized.
    #[serde(skip_serializing)]
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            sync_interval_mins: 60,
            sync_on_startup: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct RuntimePanelConfig {
    pub auto_refresh: bool,
    pub delay_sort: String,
    pub delay_test_url: String,
    pub delay_timeout_ms: u32,
    pub connection_filter: String,
    pub connection_sort: String,
}

impl Default for RuntimePanelConfig {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            delay_sort: "delay_asc".to_string(),
            delay_test_url: "http://www.gstatic.com/generate_204".to_string(),
            delay_timeout_ms: 5000,
            connection_filter: String::new(),
            connection_sort: "download_desc".to_string(),
        }
    }
}

/// Loopback Admin API settings shared by all native surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AdminServerConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 25210,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub editor_path: Option<String>,
    pub use_bundled_core: bool,
    pub core_channel: String,
    pub language: String,
    pub theme: String,
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    pub webdav: WebDavConfig,
    pub runtime_panel: RuntimePanelConfig,
    pub admin: AdminServerConfig,
    /// Optional profile/config directory override. Resolution precedence is
    /// handled by the config adapter, not by this value object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_proxy_bypass: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_size: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_position: Option<(i32, i32)>,
    #[serde(default)]
    pub window_maximized: bool,
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_close_to_tray() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_path: None,
            use_bundled_core: true,
            core_channel: "stable".to_string(),
            language: "zh-CN".to_string(),
            theme: "system".to_string(),
            notifications_enabled: default_notifications_enabled(),
            close_to_tray: default_close_to_tray(),
            webdav: WebDavConfig::default(),
            runtime_panel: RuntimePanelConfig::default(),
            admin: AdminServerConfig::default(),
            configs_dir: None,
            system_proxy_bypass: None,
            window_size: None,
            window_position: None,
            window_maximized: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_product_contract() {
        let settings = AppSettings::default();
        assert!(settings.use_bundled_core);
        assert!(settings.notifications_enabled);
        assert!(settings.close_to_tray);
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.admin.port, 25210);
        assert_eq!(settings.runtime_panel.delay_sort, "delay_asc");
    }

    #[test]
    fn webdav_password_is_not_serialized() {
        let mut settings = AppSettings::default();
        settings.webdav.password = "secret-value".to_string();
        let encoded = toml::to_string(&settings).expect("settings serialize");
        assert!(!encoded.contains("secret-value"));
        assert!(!encoded.contains("password"));
    }
}
