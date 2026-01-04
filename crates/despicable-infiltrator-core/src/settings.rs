use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub open_webui_on_startup: bool,
    pub editor_path: Option<String>,
    pub use_bundled_core: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            open_webui_on_startup: false,
            editor_path: None,
            use_bundled_core: true,
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
