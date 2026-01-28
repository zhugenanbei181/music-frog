use super::channel::{fetch_latest, Channel};
use super::download::{DownloadProgress, Downloader};
use mihomo_platform::get_home_dir;
use mihomo_api::{MihomoError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub path: PathBuf,
    pub is_default: bool,
}

pub struct VersionManager {
    install_dir: PathBuf,
    config_file: PathBuf,
}

impl VersionManager {
    pub fn new() -> Result<Self> {
        let home = get_home_dir()?;
        Self::with_home(home)
    }

    pub fn with_home(home: PathBuf) -> Result<Self> {
        let install_dir = home.join("versions");
        let config_file = home.join("config.toml");

        Ok(Self {
            install_dir,
            config_file,
        })
    }

    pub async fn install(&self, version: &str) -> Result<()> {
        self.install_with_progress(version, |_| {}).await
    }

    pub async fn install_with_progress<F>(&self, version: &str, on_progress: F) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        fs::create_dir_all(&self.install_dir).await?;

        let version_dir = self.install_dir.join(version);
        if version_dir.exists() {
            return Err(MihomoError::Version(format!(
                "Version {} is already installed",
                version
            )));
        }

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };

        // Download to OS temp directory first
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("mihomo-{}-{}", version, binary_name));

        let downloader = Downloader::new();
        downloader
            .download_version_with_progress(version, &temp_path, on_progress)
            .await?;

        // Move to final location only after successful download
        fs::create_dir_all(&version_dir).await?;
        let binary_path = version_dir.join(binary_name);
        fs::rename(&temp_path, &binary_path).await?;

        Ok(())
    }

    pub async fn install_channel(&self, channel: Channel) -> Result<String> {
        let info = fetch_latest(channel).await?;
        self.install(&info.version).await?;
        Ok(info.version)
    }

    pub async fn list_installed(&self) -> Result<Vec<VersionInfo>> {
        if !self.install_dir.exists() {
            return Ok(vec![]);
        }

        let mut versions = vec![];
        let default_version = self.get_default().await.ok();

        let mut entries = fs::read_dir(&self.install_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let version = entry.file_name().to_string_lossy().to_string();
                let is_default = default_version.as_ref() == Some(&version);
                versions.push(VersionInfo {
                    version,
                    path: entry.path(),
                    is_default,
                });
            }
        }

        versions.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(versions)
    }

    pub async fn set_default(&self, version: &str) -> Result<()> {
        let version_dir = self.install_dir.join(version);
        if !version_dir.exists() {
            return Err(MihomoError::NotFound(format!(
                "Version {} is not installed",
                version
            )));
        }

        if let Some(parent) = self.config_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let config = format!("[default]\nversion = \"{}\"\n", version);
        fs::write(&self.config_file, config).await?;

        Ok(())
    }

    pub async fn get_default(&self) -> Result<String> {
        if !self.config_file.exists() {
            return Err(MihomoError::NotFound("No default version set".to_string()));
        }

        let content = fs::read_to_string(&self.config_file).await?;
        let config: toml::Value = toml::from_str(&content)
            .map_err(|e| MihomoError::Config(format!("Invalid config: {}", e)))?;

        config
            .get("default")
            .and_then(|d| d.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| MihomoError::Config("No default version in config".to_string()))
    }

    pub async fn get_binary_path(&self, version: Option<&str>) -> Result<PathBuf> {
        let version = if let Some(v) = version {
            v.to_string()
        } else {
            self.get_default().await?
        };

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };

        let path = self.install_dir.join(&version).join(binary_name);
        if !path.exists() {
            return Err(MihomoError::NotFound(format!(
                "Binary not found for version {}",
                version
            )));
        }

        Ok(path)
    }

    pub async fn uninstall(&self, version: &str) -> Result<()> {
        let version_dir = self.install_dir.join(version);
        if !version_dir.exists() {
            return Err(MihomoError::NotFound(format!(
                "Version {} is not installed",
                version
            )));
        }

        let default_version = self.get_default().await.ok();
        if default_version.as_ref() == Some(&version.to_string()) {
            return Err(MihomoError::Version(
                "Cannot uninstall the default version".to_string(),
            ));
        }

        fs::remove_dir_all(version_dir).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_manager(temp_dir: &TempDir) -> VersionManager {
        let home = temp_dir.path().to_path_buf();
        VersionManager::with_home(home).unwrap()
    }

    #[test]
    fn test_version_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        assert_eq!(manager.install_dir, temp_dir.path().join("versions"));
        assert_eq!(manager.config_file, temp_dir.path().join("config.toml"));
    }

    #[tokio::test]
    async fn test_list_installed_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let result = manager.list_installed().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_installed_with_versions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        // Create version directories
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0")).await.unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.20.0")).await.unwrap();

        let result = manager.list_installed().await;
        assert!(result.is_ok());
        let versions = result.unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions.iter().any(|v| v.version == "v1.19.0"));
        assert!(versions.iter().any(|v| v.version == "v1.20.0"));
    }

    #[tokio::test]
    async fn test_list_installed_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        // Create version directories
        tokio::fs::create_dir_all(manager.install_dir.join("v1.18.0")).await.unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.20.0")).await.unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0")).await.unwrap();

        let result = manager.list_installed().await;
        assert!(result.is_ok());
        let versions = result.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, "v1.20.0");
        assert_eq!(versions[1].version, "v1.19.0");
        assert_eq!(versions[2].version, "v1.18.0");
    }

    #[tokio::test]
    async fn test_set_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        // Create version directory
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0")).await.unwrap();

        let result = manager.set_default("v1.19.0").await;
        assert!(result.is_ok());

        let default = manager.get_default().await;
        assert!(default.is_ok());
        assert_eq!(default.unwrap(), "v1.19.0");
    }

    #[tokio::test]
    async fn test_set_nonexistent_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let result = manager.set_default("v1.19.0").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not installed"));
    }

    #[tokio::test]
    async fn test_get_default_not_set() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let result = manager.get_default().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No default version set"));
    }

    #[tokio::test]
    async fn test_get_binary_path_with_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };
        let binary_path = version_dir.join(binary_name);
        tokio::fs::write(&binary_path, "binary").await.unwrap();

        let result = manager.get_binary_path(Some("v1.19.0")).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), binary_path);
    }

    #[tokio::test]
    async fn test_get_binary_path_with_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };
        let binary_path = version_dir.join(binary_name);
        tokio::fs::write(&binary_path, "binary").await.unwrap();

        manager.set_default("v1.19.0").await.unwrap();

        let result = manager.get_binary_path(None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), binary_path);
    }

    #[tokio::test]
    async fn test_get_binary_path_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        let result = manager.get_binary_path(Some("v1.19.0")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Binary not found"));
    }

    #[tokio::test]
    async fn test_install_version_already_installed() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        let result = manager.install("v1.19.0").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already installed"));
    }

    #[tokio::test]
    async fn test_uninstall_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        let result = manager.uninstall("v1.19.0").await;
        assert!(result.is_ok());
        assert!(!version_dir.exists());
    }

    #[tokio::test]
    async fn test_uninstall_nonexistent_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let result = manager.uninstall("v1.19.0").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not installed"));
    }

    #[tokio::test]
    async fn test_uninstall_default_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);

        let version_dir = manager.install_dir.join("v1.19.0");
        tokio::fs::create_dir_all(&version_dir).await.unwrap();

        manager.set_default("v1.19.0").await.unwrap();

        let result = manager.uninstall("v1.19.0").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot uninstall the default version"));
    }
}