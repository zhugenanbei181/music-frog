use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "".to_string(),
            username: "".to_string(),
            password: "".to_string(),
            sync_interval_mins: 60,
            sync_on_startup: false,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
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

/// Admin Web UI server settings, shared by both desktop frontends.
///
/// Defaults mirror the legacy Tauri client, which always serves the admin UI
/// on loopback starting at port 25210. The server only ever binds 127.0.0.1.
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub open_webui_on_startup: bool,
    pub editor_path: Option<String>,
    pub use_bundled_core: bool,
    pub language: String,
    pub theme: String,
    pub webdav: WebDavConfig,
    pub runtime_panel: RuntimePanelConfig,
    pub admin: AdminServerConfig,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            open_webui_on_startup: false,
            editor_path: None,
            use_bundled_core: true,
            language: "zh-CN".to_string(),
            theme: "system".to_string(),
            webdav: WebDavConfig::default(),
            runtime_panel: RuntimePanelConfig::default(),
            admin: AdminServerConfig::default(),
        }
    }
}

pub async fn load_settings(path: &Path) -> anyhow::Result<AppSettings> {
    if path.exists() {
        let content = tokio::fs::read_to_string(path).await?;
        let settings: AppSettings = toml::from_str(&content)?;
        Ok(settings)
    } else {
        let legacy_path = path.with_extension("json");
        if legacy_path.exists() {
            let content = tokio::fs::read_to_string(&legacy_path).await?;
            let settings: AppSettings = serde_json::from_str(&content)?;
            if let Err(err) = save_settings(path, &settings).await {
                log::warn!("failed to migrate settings to toml: {err:#}");
            }
            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }
}

pub async fn save_settings(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = toml::to_string_pretty(settings)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

pub fn settings_path(base_dir: &Path) -> anyhow::Result<std::path::PathBuf> {
    if base_dir.as_os_str().is_empty() {
        return Err(anyhow!("settings base dir is empty"));
    }
    Ok(base_dir.join("settings.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(!settings.open_webui_on_startup);
        assert!(settings.use_bundled_core);
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.theme, "system");
        assert!(settings.runtime_panel.auto_refresh);
        assert_eq!(settings.runtime_panel.delay_sort, "delay_asc");
        assert_eq!(
            settings.runtime_panel.delay_test_url,
            "http://www.gstatic.com/generate_204"
        );
        assert_eq!(settings.runtime_panel.delay_timeout_ms, 5000);
        assert_eq!(settings.runtime_panel.connection_sort, "download_desc");
    }

    #[test]
    fn test_admin_server_config_default_matches_tauri_behavior() {
        let admin = AdminServerConfig::default();
        assert!(admin.enabled, "admin server is on by default like src-tauri");
        assert_eq!(admin.port, 25210);
    }

    #[tokio::test]
    async fn test_settings_without_admin_section_get_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        // A pre-admin settings file: must deserialize with admin defaults
        // instead of failing, so old installs keep working (serde back-compat).
        let legacy = "[runtime_panel]\nauto_refresh = true\n";
        std::fs::write(&settings_file, legacy).unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.admin, AdminServerConfig::default());
    }

    #[tokio::test]
    async fn test_save_and_load_admin_section() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings::default();
        settings.admin.enabled = false;
        settings.admin.port = 3000;

        save_settings(&settings_file, &settings).await.unwrap();
        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(!loaded.admin.enabled);
        assert_eq!(loaded.admin.port, 3000);
    }

    #[test]
    fn test_settings_path() {
        let base_dir = PathBuf::from("test_dir");
        let path = settings_path(&base_dir).expect("valid base dir should work");
        assert_eq!(path, base_dir.join("settings.toml"));

        let empty_dir = PathBuf::from("");
        assert!(settings_path(&empty_dir).is_err());
    }

    #[tokio::test]
    async fn test_save_and_load_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings {
            language: "en-US".to_string(),
            ..AppSettings::default()
        };
        settings.webdav.enabled = true;

        save_settings(&settings_file, &settings).await.unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.language, "en-US");
        assert!(loaded.webdav.enabled);
        assert!(loaded.runtime_panel.auto_refresh);
    }
}
