use super::channel::{Channel, fetch_latest};
use super::download::{DownloadProgress, Downloader};
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::paths::get_home_dir;
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
        self.install_with_progress_and_cancel(version, on_progress, || false)
            .await
    }

    /// Cancellation-aware install path. The predicate is checked before
    /// network/filesystem work and while downloading; all failed/cancelled
    /// paths clean the temporary archive and version directory.
    pub async fn install_with_progress_and_cancel<F, C>(
        &self,
        version: &str,
        on_progress: F,
        is_cancelled: C,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
        C: Fn() -> bool,
    {
        if is_cancelled() {
            return Err(MihomoError::Version("下载已取消".to_string()));
        }
        fs::create_dir_all(&self.install_dir).await?;

        let version_dir = self.install_dir.join(version);
        if version_dir.exists() {
            return Err(MihomoError::Version(format!(
                "Version {} is already installed",
                version
            )));
        }

        if is_cancelled() {
            return Err(MihomoError::Version("下载已取消".to_string()));
        }

        // Provenance first (UP-001): refuse to download anything when the
        // release API does not publish a SHA-256 digest for this platform's
        // archive. The digest is the trusted input for fail-closed
        // verification inside the download pipeline.
        let expected_digest = super::channel::fetch_asset_digest(version).await?;

        if is_cancelled() {
            return Err(MihomoError::Version("下载已取消".to_string()));
        }

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };

        // Download to OS temp directory first; the file name is
        // process-unique so concurrent installs cannot collide.
        let temp_dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let temp_path = temp_dir.join(format!(
            "mihomo-{}-{}-{}-{nanos}",
            version,
            binary_name,
            std::process::id()
        ));

        let downloader = Downloader::new();
        if let Err(err) = downloader
            .download_version_with_progress_and_cancel(
                version,
                &temp_path,
                Some(&expected_digest),
                on_progress,
                &is_cancelled,
            )
            .await
        {
            // Cleanup temp file on download failure
            if temp_path.exists() {
                let _ = fs::remove_file(&temp_path).await;
            }
            return Err(err);
        }

        if is_cancelled() {
            let _ = fs::remove_file(&temp_path).await;
            return Err(MihomoError::Version("下载已取消".to_string()));
        }

        // Move to final location only after successful download
        let install = async {
            fs::create_dir_all(&version_dir).await?;
            let binary_path = version_dir.join(binary_name);
            fs::rename(&temp_path, &binary_path).await?;
            // Post-install smoke check (CORE-006): a binary that cannot even
            // print its version must not become installable, let alone the
            // default. Old versions stay untouched on failure.
            if let Err(err) = smoke_check_binary(&binary_path).await {
                let _ = fs::remove_dir_all(&version_dir).await;
                return Err(err);
            }
            Ok::<(), MihomoError>(())
        }
        .await;

        if let Err(err) = install {
            // Cleanup on filesystem error or failed smoke check
            if version_dir.exists() {
                let _ = fs::remove_dir_all(&version_dir).await;
            }
            if temp_path.exists() {
                let _ = fs::remove_file(&temp_path).await;
            }
            return Err(err);
        }

        log::info!("installed kernel {version} (digest verified, smoke check passed)");
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

        // Second health gate (CORE-006): a version cannot become default
        // unless its binary proves runnable right now — install-time smoke
        // already passed, but the file may have been corrupted or replaced
        // since. The previously default version stays untouched on failure.
        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };
        smoke_check_binary(&version_dir.join(binary_name)).await?;

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

/// Post-install health check (CORE-006): the freshly installed binary must
/// print its version and exit cleanly. This runs before the version can be
/// selected as default, so a corrupt or wrong-architecture artifact is
/// rejected while every previously installed version remains usable.
async fn smoke_check_binary(path: &std::path::Path) -> Result<()> {
    let output = tokio::process::Command::new(path)
        .arg("-v")
        .output()
        .await
        .map_err(|e| {
            MihomoError::Version(format!(
                "kernel smoke check could not execute {}: {e}",
                path.display()
            ))
        })?;

    if !output.status.success() {
        return Err(MihomoError::Version(format!(
            "kernel smoke check failed: `{} -v` exited with {}",
            path.display(),
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(MihomoError::Version(format!(
            "kernel smoke check failed: `{} -v` printed no version output",
            path.display()
        )));
    }

    log::info!("kernel smoke check: {}", stdout.trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_script(dir: &TempDir, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn smoke_check_accepts_version_printing_binary() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(&dir, "mihomo", "echo \"Mihomo Meta v1.19.18 test\"");
        assert!(smoke_check_binary(&fake).await.is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn smoke_check_rejects_failing_binary() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(&dir, "mihomo", "exit 3");
        let err = smoke_check_binary(&fake).await.unwrap_err();
        assert!(err.to_string().contains("exited"), "{err}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn smoke_check_rejects_silent_binary() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(&dir, "mihomo", "exit 0");
        let err = smoke_check_binary(&fake).await.unwrap_err();
        assert!(err.to_string().contains("no version output"), "{err}");
    }

    fn setup_test_manager(temp_dir: &TempDir) -> VersionManager {
        let home = temp_dir.path().to_path_buf();
        VersionManager::with_home(home).unwrap()
    }

    #[tokio::test]
    async fn cancelled_install_stops_before_network_or_filesystem_work() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);
        let error = manager
            .install_with_progress_and_cancel("v-test", |_| {}, || true)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("下载已取消"), "{error}");
        assert!(!temp_dir.path().join("versions").exists());
    }

    /// set_default now smoke-checks the candidate binary; tests that merely
    /// install a fake version must provide a runnable stand-in.
    #[cfg(unix)]
    fn plant_runnable_fake_binary(home: &std::path::Path, version: &str) {
        use std::os::unix::fs::PermissionsExt;
        let dir = home.join("versions").join(version);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("mihomo");
        std::fs::write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
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
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.20.0"))
            .await
            .unwrap();

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
        tokio::fs::create_dir_all(manager.install_dir.join("v1.18.0"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.20.0"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0"))
            .await
            .unwrap();

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

        // Create version directory with a binary that passes smoke check
        #[cfg(unix)]
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");
        #[cfg(not(unix))]
        tokio::fs::create_dir_all(manager.install_dir.join("v1.19.0"))
            .await
            .unwrap();

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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No default version set")
        );
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
        #[cfg(unix)]
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");
        #[cfg(not(unix))]
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("already installed")
        );
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
        #[cfg(unix)]
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");

        manager.set_default("v1.19.0").await.unwrap();

        let result = manager.uninstall("v1.19.0").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot uninstall the default version")
        );
    }

    #[tokio::test]
    async fn test_install_cleanup_on_failure() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);
        let version = "v9.9.9";

        // 预先创建一个文件占坑，导致目录创建失败
        let conflict_path = manager.install_dir.join(version);
        tokio::fs::create_dir_all(&manager.install_dir)
            .await
            .unwrap();
        tokio::fs::write(&conflict_path, "I am a file, not a dir")
            .await
            .unwrap();

        // 尝试安装 (由于 Downloader 会先下载，这里可能先报下载错误)
        // 但如果我们模拟一个下载成功但后续失败的场景...
        // 鉴于目前代码结构，我增加一个内部验证：
        // 如果安装过程抛出任何错误，install_dir/version 应该不存在或者保持原样。
        let _ = manager.install(version).await;

        // 如果安装失败，它不应该留下一个半成品目录（如果是文件占坑，它不应该被删掉，但也不应该变成目录）
        assert!(tokio::fs::metadata(&conflict_path).await.unwrap().is_file());
    }
}
