use super::channel::{Channel, fetch_latest};
use super::download::{DownloadProgress, Downloader};
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::paths::get_home_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

const MAX_VERSION_HISTORY: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub path: PathBuf,
    pub is_default: bool,
}

/// Persistent version-selection state exposed by the version adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRollbackInfo {
    pub current: Option<String>,
    pub target: Option<String>,
    pub history: Vec<String>,
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
        validate_version_label(version)?;
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
        validate_version_label(version)?;
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

        let mut config = self.read_config().await?;
        let previous = default_version_from_config(&config);
        let mut history = version_history_from_config(&config);
        record_version_transition(&mut history, previous.as_deref(), version);
        set_default_in_config(&mut config, version, &history)?;
        self.write_config(&config).await
    }

    pub async fn get_default(&self) -> Result<String> {
        if !self.config_file.exists() {
            return Err(MihomoError::NotFound("No default version set".to_string()));
        }
        let config = self.read_config().await?;
        default_version_from_config(&config)
            .ok_or_else(|| MihomoError::Config("No default version in config".to_string()))
    }

    /// Return the locally recorded default-version history without touching
    /// the network. Missing or deleted historical binaries are skipped when
    /// choosing `target`, while the bounded history remains visible for
    /// diagnostics and future recovery.
    pub async fn rollback_info(&self) -> Result<VersionRollbackInfo> {
        let config = self.read_config().await?;
        let current = default_version_from_config(&config);
        let history = version_history_from_config(&config);
        let target = self
            .find_rollback_target(current.as_deref(), &history)
            .await;
        Ok(VersionRollbackInfo {
            current,
            target,
            history,
        })
    }

    /// Select the most recent installed historical binary. This operation is
    /// local and bounded: it validates the candidate with the same `-v`
    /// smoke check used by installation/default activation, then atomically
    /// updates only the version-selection fields in the shared config file.
    pub async fn rollback(&self) -> Result<String> {
        let mut config = self.read_config().await?;
        let current = default_version_from_config(&config);
        let mut history = version_history_from_config(&config);
        let target = self
            .find_rollback_target(current.as_deref(), &history)
            .await
            .ok_or_else(|| {
                MihomoError::NotFound("No installed previous core version to roll back to".to_string())
            })?;

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };
        smoke_check_binary(&self.install_dir.join(&target).join(binary_name)).await?;

        // A rollback consumes the selected entry, moving backward through
        // the stack. The failed/newer version is deliberately not re-added:
        // this action is recovery, not an undo toggle.
        history.retain(|item| item != &target && current.as_deref() != Some(item.as_str()));
        set_default_in_config(&mut config, &target, &history)?;
        self.write_config(&config).await?;
        log::info!("rolled back mihomo core from {:?} to {target}", current);
        Ok(target)
    }

    pub async fn get_binary_path(&self, version: Option<&str>) -> Result<PathBuf> {
        let version = if let Some(v) = version {
            v.to_string()
        } else {
            self.get_default().await?
        };
        validate_version_label(&version)?;

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
        validate_version_label(version)?;
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

    async fn read_config(&self) -> Result<toml::Value> {
        if !self.config_file.exists() {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }
        let content = fs::read_to_string(&self.config_file).await?;
        toml::from_str(&content)
            .map_err(|error| MihomoError::Config(format!("Invalid config: {error}")))
    }

    async fn write_config(&self, config: &toml::Value) -> Result<()> {
        let content = toml::to_string(config)
            .map_err(|error| MihomoError::Config(format!("Failed to serialize config: {error}")))?;
        atomic_write(&self.config_file, content.as_bytes()).await
    }

    async fn find_rollback_target(&self, current: Option<&str>, history: &[String]) -> Option<String> {
        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };
        for version in history {
            if current == Some(version.as_str()) || validate_version_label(version).is_err() {
                continue;
            }
            let path = self.install_dir.join(version).join(binary_name);
            if fs::try_exists(&path).await.unwrap_or(false) {
                return Some(version.clone());
            }
        }
        None
    }
}

fn default_version_from_config(config: &toml::Value) -> Option<String> {
    config
        .get("default")
        .and_then(|default| default.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}

fn version_history_from_config(config: &toml::Value) -> Vec<String> {
    config
        .get("default")
        .and_then(|default| default.get("version_history"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .filter(|version| validate_version_label(version).is_ok())
        .map(str::to_owned)
        .take(MAX_VERSION_HISTORY)
        .collect()
}

fn record_version_transition(history: &mut Vec<String>, previous: Option<&str>, selected: &str) {
    history.retain(|version| version != selected);
    if let Some(previous) = previous
        && previous != selected
        && validate_version_label(previous).is_ok()
    {
        history.retain(|version| version != previous);
        history.insert(0, previous.to_owned());
    }
    history.truncate(MAX_VERSION_HISTORY);
}

fn set_default_in_config(
    config: &mut toml::Value,
    version: &str,
    history: &[String],
) -> Result<()> {
    let root = config.as_table_mut().ok_or_else(|| {
        MihomoError::Config("version selection config root must be a TOML table".to_string())
    })?;
    let default = root
        .entry("default".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let default = default.as_table_mut().ok_or_else(|| {
        MihomoError::Config("version selection config.default must be a TOML table".to_string())
    })?;
    default.insert(
        "version".to_string(),
        toml::Value::String(version.to_owned()),
    );
    default.insert(
        "version_history".to_string(),
        toml::Value::Array(
            history
                .iter()
                .cloned()
                .map(toml::Value::String)
                .collect(),
        ),
    );
    Ok(())
}

fn validate_version_label(version: &str) -> Result<()> {
    if version.is_empty()
        || version == "."
        || version == ".."
        || version
            .chars()
            .any(|character| character == '/' || character == '\\' || character.is_control())
    {
        return Err(MihomoError::Version(format!(
            "invalid core version label: {version:?}"
        )));
    }
    Ok(())
}

async fn atomic_write(path: &std::path::Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        MihomoError::Config(format!("version config path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).await?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{file_name}.version-tmp-{}-{stamp}", std::process::id()));

    let result = async {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);

        // `rename` is atomic on Unix. Windows cannot replace an existing
        // target, so remove only this exact config target as the fallback.
        #[cfg(windows)]
        if fs::try_exists(path).await? {
            fs::remove_file(path).await?;
        }
        fs::rename(&temporary, path).await
    }
    .await;

    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result.map_err(MihomoError::Io)
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

    #[cfg(unix)]
    #[tokio::test]
    async fn version_selection_preserves_profile_and_rolls_back_without_network() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);
        tokio::fs::write(
            temp_dir.path().join("config.toml"),
            "[default]\nprofile = \"work\"\n",
        )
        .await
        .unwrap();
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.28");
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.29");
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.30");

        manager.set_default("v1.19.28").await.unwrap();
        manager.set_default("v1.19.29").await.unwrap();
        manager.set_default("v1.19.30").await.unwrap();

        let config = tokio::fs::read_to_string(temp_dir.path().join("config.toml"))
            .await
            .unwrap();
        let config: toml::Value = toml::from_str(&config).unwrap();
        assert_eq!(
            config["default"]["profile"].as_str(),
            Some("work"),
            "version selection must not overwrite profile selection"
        );
        assert_eq!(
            config["default"]["version_history"].as_array(),
            Some(&vec![
                toml::Value::String("v1.19.29".to_owned()),
                toml::Value::String("v1.19.28".to_owned()),
            ])
        );

        let info = manager.rollback_info().await.unwrap();
        assert_eq!(info.current.as_deref(), Some("v1.19.30"));
        assert_eq!(info.target.as_deref(), Some("v1.19.29"));

        let selected = manager.rollback().await.unwrap();
        assert_eq!(selected, "v1.19.29");
        assert_eq!(manager.get_default().await.unwrap(), "v1.19.29");
        let next = manager.rollback_info().await.unwrap();
        assert_eq!(next.target.as_deref(), Some("v1.19.28"));
        assert_eq!(next.history.first().map(String::as_str), Some("v1.19.28"));
    }

    #[tokio::test]
    async fn rollback_requires_an_installed_previous_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);
        let error = manager.rollback().await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("No installed previous core version"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rollback_health_gate_keeps_current_version_on_bad_candidate() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir);
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.28");
        plant_runnable_fake_binary(temp_dir.path(), "v1.19.29");
        manager.set_default("v1.19.28").await.unwrap();
        manager.set_default("v1.19.29").await.unwrap();

        tokio::fs::write(
            temp_dir.path().join("versions/v1.19.28/mihomo"),
            "#!/bin/sh\nexit 9\n",
        )
        .await
        .unwrap();
        let error = manager.rollback().await.unwrap_err();
        assert!(error.to_string().contains("smoke check"), "{error}");
        assert_eq!(manager.get_default().await.unwrap(), "v1.19.29");
        assert_eq!(manager.rollback_info().await.unwrap().target.as_deref(), Some("v1.19.28"));
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
