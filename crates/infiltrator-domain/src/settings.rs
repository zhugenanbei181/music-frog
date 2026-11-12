//! Runtime-neutral application settings schema.
//!
//! This module contains only serializable configuration values and defaults.
//! TOML/filesystem persistence and keyring hydration live in
//! `infiltrator-core::settings_io`.

use infiltrator_contract::connection_view_preference::ConnectionViewPreference;
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_contract::runtime_settings::{RuntimeDelaySortOrder, RuntimeProbeSettings};
use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;
use infiltrator_contract::shortcuts::ShortcutBinding;
use serde::{Deserialize, Serialize};

pub const CURRENT_SETTINGS_SCHEMA_VERSION: u32 = 2;

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
#[serde(default, deny_unknown_fields)]
pub struct RuntimePanelConfig {
    pub auto_refresh: bool,
    pub delay_sort_order: RuntimeDelaySortOrder,
    pub probe_settings: RuntimeProbeSettings,
    pub connection_filter: String,
    pub connection_sort: String,
}

impl Default for RuntimePanelConfig {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            delay_sort_order: RuntimeDelaySortOrder::default(),
            probe_settings: RuntimeProbeSettings::default(),
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
    pub settings_schema_version: u32,
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
    /// Global shell-chord bindings (shared contract: `ShortcutBinding`).
    /// An empty list means "product defaults".
    #[serde(default)]
    pub shortcuts: Vec<ShortcutBinding>,
    /// Persisted Mini HUD placement (shared contract geometry, DUAL-15-04).
    #[serde(default)]
    pub mini_hud: MiniHudPlacement,
    /// Refresh the active profile on application startup.
    #[serde(default)]
    pub refresh_current_profile_on_start: bool,
    /// Custom accent color hex code (e.g. "#016888").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_accent_color: Option<String>,
    /// Tray icon style: `colored` (status-coloured disc) or `monochrome`
    /// (macOS menu bar / dark taskbar template glyph).
    #[serde(default = "default_tray_icon_color_mode")]
    pub tray_icon_color_mode: String,
    /// User-owned desktop sidebar expansion preference. The viewport decides
    /// whether the preference can be honoured; this field only stores it.
    #[serde(default)]
    pub sidebar_expansion: SidebarExpansionPreference,
    /// User-owned Connections view mode and ordered visible columns. A surface
    /// reads and mutates this only through the typed intent owner.
    #[serde(default)]
    pub connection_view: ConnectionViewPreference,
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_close_to_tray() -> bool {
    true
}

fn default_tray_icon_color_mode() -> String {
    "colored".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            settings_schema_version: CURRENT_SETTINGS_SCHEMA_VERSION,
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
            shortcuts: Vec::new(),
            mini_hud: MiniHudPlacement::default(),
            refresh_current_profile_on_start: false,
            custom_accent_color: None,
            tray_icon_color_mode: default_tray_icon_color_mode(),
            sidebar_expansion: SidebarExpansionPreference::default(),
            connection_view: ConnectionViewPreference::default(),
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
        assert_eq!(
            settings.sidebar_expansion,
            SidebarExpansionPreference::Expanded
        );
        assert_eq!(settings.admin.port, 25210);
        assert_eq!(
            settings.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            settings.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyAscending
        );
        assert_eq!(
            settings.runtime_panel.probe_settings,
            RuntimeProbeSettings::default()
        );
        assert_eq!(
            settings.connection_view,
            ConnectionViewPreference::default()
        );
        assert_eq!(settings.connection_view.mode, Default::default());
        assert_eq!(
            settings.connection_view.visible_columns.wire_ids(),
            ConnectionViewPreference::default()
                .visible_columns
                .wire_ids()
        );
    }

    #[test]
    fn old_toml_without_connection_view_defaults_to_one_typed_preference() {
        let settings: AppSettings = toml::from_str(
            r#"
                settings_schema_version = 2
                language = "zh-CN"
                theme = "system"
                sidebar_expansion = "expanded"
            "#,
        )
        .expect("old settings document still deserializes");
        assert_eq!(
            settings.connection_view,
            ConnectionViewPreference::default()
        );
    }

    #[test]
    fn connection_view_round_trips_through_toml_as_one_nested_preference() {
        let settings = AppSettings {
            connection_view: ConnectionViewPreference::from_wire(
                "table",
                &["host", "process", "uploadSpeed"],
            )
            .expect("valid connection view preference"),
            ..AppSettings::default()
        };
        let encoded = toml::to_string(&settings).expect("settings serialize");
        assert!(encoded.contains("[connection_view]"));
        assert!(encoded.contains("mode = \"table\""));
        let decoded: AppSettings = toml::from_str(&encoded).expect("settings deserialize");
        assert_eq!(decoded.connection_view, settings.connection_view);
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
