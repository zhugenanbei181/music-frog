use super::channel::{Channel, fetch_asset_digest, fetch_latest};
use super::download::{DownloadProgress, Downloader};
use crate::engine;
use infiltrator_contract::core_identity::EngineId;
use mihomo_api::error::{MihomoError, Result};
use serde::{Deserialize, Serialize};
use std::env::temp_dir;
use std::path;
use std::path::PathBuf;
use std::process::id;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::{fs, process};
use toml::map::Map;

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
    engine_id: EngineId,
    install_dir: PathBuf,
    config_file: PathBuf,
    legacy_install_dir: PathBuf,
    legacy_config_file: PathBuf,
}

impl VersionManager {
    pub fn for_engine(home: PathBuf, engine_id: EngineId) -> Result<Self> {
        if engine_id != engine::engine_id() {
            return Err(MihomoError::Version(format!(
                "mihomo version manager does not support engine {}",
                engine_id.as_str()
            )));
        }
        let engine_root = home.join("engines").join(engine_id.as_str());
        let install_dir = engine_root.join("versions");
        let config_file = engine_root.join("config.toml");

        Ok(Self {
            engine_id,
            install_dir,
            config_file,
            legacy_install_dir: home.join("versions"),
            legacy_config_file: home.join("config.toml"),
        })
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
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
            return Err(MihomoError::Canceled);
        }
        self.ensure_engine_namespace().await?;
        fs::create_dir_all(&self.install_dir).await?;

        let version_dir = self.install_dir.join(version);
        if version_dir.exists() {
            return Err(MihomoError::Version(format!(
                "Version {} is already installed",
                version
            )));
        }

        if is_cancelled() {
            return Err(MihomoError::Canceled);
        }

        // Provenance first (UP-001): refuse to download anything when the
        // release API does not publish a SHA-256 digest for this platform's
        // archive. The digest is the trusted input for fail-closed
        // verification inside the download pipeline.
        let expected_digest = fetch_asset_digest(version).await?;

        if is_cancelled() {
            return Err(MihomoError::Canceled);
        }

        let binary_name = if cfg!(windows) {
            "mihomo.exe"
        } else {
            "mihomo"
        };

        // Download to OS temp directory first; the file name is
        // process-unique so concurrent installs cannot collide.
        let temp_dir = temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let temp_path = temp_dir.join(format!(
            "mihomo-{}-{}-{}-{nanos}",
            version,
            binary_name,
            id()
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
            return Err(MihomoError::Canceled);
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
        let mut config = self.read_config().await?;
        let current = default_version_from_config(&config);
        let mut history = version_history_from_config(&config);
        let target = self
            .find_rollback_target(current.as_deref(), &history)
            .await
            .ok_or_else(|| {
                MihomoError::NotFound(
                    "No installed previous core version to roll back to".to_string(),
                )
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
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
        self.ensure_engine_namespace().await?;
        if !self.config_file.exists() {
            return Ok(toml::Value::Table(Map::new()));
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

    async fn find_rollback_target(
        &self,
        current: Option<&str>,
        history: &[String],
    ) -> Option<String> {
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

    async fn ensure_engine_namespace(&self) -> Result<()> {
        let parent = self.install_dir.parent().ok_or_else(|| {
            MihomoError::Config("engine-scoped version directory has no parent".to_string())
        })?;
        fs::create_dir_all(parent).await?;

        if fs::try_exists(&self.legacy_install_dir).await? {
            if !fs::try_exists(&self.install_dir).await? {
                fs::rename(&self.legacy_install_dir, &self.install_dir).await?;
            } else {
                let mut legacy = fs::read_dir(&self.legacy_install_dir).await?;
                while let Some(entry) = legacy.next_entry().await? {
                    if entry.file_type().await?.is_dir() {
                        let destination = self.install_dir.join(entry.file_name());
                        if !fs::try_exists(&destination).await? {
                            fs::rename(entry.path(), destination).await?;
                        }
                    }
                }
            }
        }

        if !fs::try_exists(&self.config_file).await?
            && fs::try_exists(&self.legacy_config_file).await?
        {
            let legacy_config = fs::read(&self.legacy_config_file).await?;
            atomic_write(&self.config_file, &legacy_config).await?;
        }
        Ok(())
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
        .or_insert_with(|| toml::Value::Table(Map::new()));
    let default = default.as_table_mut().ok_or_else(|| {
        MihomoError::Config("version selection config.default must be a TOML table".to_string())
    })?;
    default.insert(
        "version".to_string(),
        toml::Value::String(version.to_owned()),
    );
    default.insert(
        "version_history".to_string(),
        toml::Value::Array(history.iter().cloned().map(toml::Value::String).collect()),
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

async fn atomic_write(path: &path::Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        MihomoError::Config(format!(
            "version config path has no parent: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).await?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{file_name}.version-tmp-{}-{stamp}", id()));

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
async fn smoke_check_binary(path: &path::Path) -> Result<()> {
    let output = process::Command::new(path)
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
#[path = "manager_tests.rs"]
mod tests;
