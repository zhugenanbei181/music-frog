//! Filesystem-backed persistent store for user-owned palette preferences (FLPARITY-036).

use async_trait::async_trait;
use infiltrator_contract::palette_preference::PalettePreference;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::palette_preference::PalettePreferenceStorePort;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use tokio::fs;

pub struct FilePalettePreferenceStore {
    path: PathBuf,
}

impl FilePalettePreferenceStore {
    pub fn current() -> anyhow::Result<Self> {
        let home = get_home_dir()?;
        Ok(Self {
            path: home.join("palette_preference.json"),
        })
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl PalettePreferenceStorePort for FilePalettePreferenceStore {
    async fn load_preference(&self) -> Result<PalettePreference, PortError> {
        if !self.path.exists() {
            return Ok(PalettePreference::default());
        }
        let content = fs::read_to_string(&self.path)
            .await
            .map_err(|e| PortError::Io(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| PortError::Io(format!("invalid palette preference JSON: {e}")))
    }

    async fn save_preference(&self, preference: &PalettePreference) -> Result<(), PortError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| PortError::Io(e.to_string()))?;
        }
        let content =
            serde_json::to_string_pretty(preference).map_err(|e| PortError::Io(e.to_string()))?;
        fs::write(&self.path, content)
            .await
            .map_err(|e| PortError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::palette_preference::{PaletteStylePreference, SeedColorSource};
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_defaults_when_file_does_not_exist() {
        let dir = tempdir().expect("tempdir");
        let store = FilePalettePreferenceStore::at(dir.path().join("palette.json"));
        let pref = store.load_preference().await.expect("load defaults");
        assert_eq!(pref, PalettePreference::default());
    }

    #[tokio::test]
    async fn save_and_reload_round_trip() {
        let dir = tempdir().expect("tempdir");
        let store = FilePalettePreferenceStore::at(dir.path().join("palette.json"));
        let pref = PalettePreference {
            seed: SeedColorSource::Custom { rgb: 0x33691E },
            style: PaletteStylePreference::FruitSalad,
            revision: 5,
            last_saved_epoch_ms: 123456,
        };
        store.save_preference(&pref).await.expect("save");
        let reloaded = store.load_preference().await.expect("reload");
        assert_eq!(reloaded, pref);
    }

    #[tokio::test]
    async fn corrupted_json_returns_port_error() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("palette.json");
        fs::write(&path, "not a valid json").await.expect("write");
        let store = FilePalettePreferenceStore::at(path);
        assert!(store.load_preference().await.is_err());
    }
}
