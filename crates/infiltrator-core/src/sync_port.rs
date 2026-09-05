//! File/keyring/WebDAV adapter for the runtime-neutral sync port.

use dav_client::{DavClient, client::WebDavClient};
use infiltrator_contract::sync::{SyncConflict, SyncProgress, SyncReport, SyncTransferReport};
use infiltrator_domain::sandbox::{PathValidationResult, SandboxValidator};
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_ports::sync::{SyncPort, SyncProgressSink, SyncRequest, SyncTransferRequest};
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use state_store::StateStore;
use sync_engine::{SyncAction, SyncPlanner, executor::SyncExecutor};
use tokio::io::AsyncWriteExt;

pub struct FileWebDavSync<S = DefaultCredentialStore> {
    home: PathBuf,
    secure_store: S,
}

impl<S> FileWebDavSync<S> {
    pub fn new(home: PathBuf, secure_store: S) -> Self {
        Self { home, secure_store }
    }
}

impl FileWebDavSync<DefaultCredentialStore> {
    pub fn current() -> anyhow::Result<Self> {
        Ok(Self::new(
            mihomo_platform::paths::get_home_dir()?,
            DefaultCredentialStore::default(),
        ))
    }
}

#[async_trait::async_trait]
impl<S> SyncPort for FileWebDavSync<S>
where
    S: SecureStore,
{
    async fn test(&self, config: WebDavConfig) -> Result<usize, PortError> {
        let password = self.password(&config).await;
        let client = WebDavClient::new(&config.url, &config.username, &password)
            .map_err(|error| PortError::Failed(format!("invalid WebDAV config: {error}")))?;
        client
            .list("")
            .await
            .map(|entries| entries.len())
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn sync(&self, request: SyncRequest) -> Result<SyncReport, PortError> {
        let config = request.config;
        if !config.enabled {
            return Ok(SyncReport::default());
        }
        if config.url.trim().is_empty() {
            return Err(PortError::Failed("WebDAV URL is empty".to_string()));
        }

        let password = self.password(&config).await;
        let dav = WebDavClient::new(&config.url, &config.username, &password)
            .map_err(|error| PortError::Failed(format!("invalid WebDAV config: {error}")))?;
        let local_root = resolve_configs_dir(
            request.configs_dir.as_deref(),
            &self.home,
        )?;
        tokio::fs::create_dir_all(&local_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;

        let db_path = self.home.join("sync_state.db");
        let store = StateStore::new(&db_path.to_string_lossy())
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let planner = SyncPlanner::new(local_root, "/".to_string(), &dav, &store);
        let actions = planner
            .build_plan()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;

        let total_actions = actions.len() as u64;
        let executor = SyncExecutor::new(&dav, &store);
        let mut report = SyncReport {
            total_actions,
            ..SyncReport::default()
        };
        for action in actions {
            let kind = action_kind(&action);
            match executor.execute(action).await {
                Ok(()) => {
                    report.success_count += 1;
                    match kind {
                        SyncActionKind::Upload => report.uploaded += 1,
                        SyncActionKind::Download => report.downloaded += 1,
                        SyncActionKind::Conflict => report.conflicts += 1,
                        SyncActionKind::Other => {}
                    }
                }
                Err(error) => {
                    report.failed_count += 1;
                    log::warn!("WebDAV sync action failed: {error:#}");
                }
            }
        }
        Ok(report)
    }

    async fn upload(
        &self,
        request: SyncTransferRequest,
    ) -> Result<SyncTransferReport, PortError> {
        let SyncTransferRequest {
            config,
            configs_dir,
            observer,
            ..
        } = request;
        let password = self.password(&config).await;
        let dav = self.client(&config, &password)?;
        let manager = self.manager(configs_dir.as_deref())?;
        let profiles = manager
            .list_profiles()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let total = profiles.len() as u64;
        observer.progress(SyncProgress {
            phase: "上传配置".to_string(),
            current: 0,
            total,
        });

        let mut report = SyncTransferReport::default();
        for (index, profile) in profiles.into_iter().enumerate() {
            ensure_not_cancelled(observer.as_ref())?;
            let content = tokio::fs::read(&profile.path)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            ensure_not_cancelled(observer.as_ref())?;
            dav.put(&format!("{}.yaml", profile.name), &content, None)
                .await
                .map_err(|error| PortError::Network(error.to_string()))?;
            report.uploaded += 1;
            observer.progress(SyncProgress {
                phase: "上传配置".to_string(),
                current: (index + 1) as u64,
                total,
            });
        }
        Ok(report)
    }

    async fn download(
        &self,
        request: SyncTransferRequest,
    ) -> Result<SyncTransferReport, PortError> {
        let SyncTransferRequest {
            config,
            configs_dir,
            runtime_present,
            observer,
        } = request;
        let password = self.password(&config).await;
        let dav = self.client(&config, &password)?;
        let manager = self.manager(configs_dir.as_deref())?;
        let files = dav
            .list("")
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let mut remote_profiles = Vec::new();
        let mut remote_names = HashSet::new();
        for file in files {
            if let Some(profile_name) = safe_remote_profile_name(&file.path)? {
                if !remote_names.insert(profile_name.clone()) {
                    return Err(PortError::Failed(format!(
                        "远端配置路径映射冲突: {profile_name}"
                    )));
                }
                remote_profiles.push((file.path, profile_name));
            }
        }

        let config_root = manager.config_dir().to_path_buf();
        let sandbox = SandboxValidator::new(config_root.clone());
        let active_profile = manager
            .get_current()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let total = remote_profiles.len() as u64;
        observer.progress(SyncProgress {
            phase: "下载配置".to_string(),
            current: 0,
            total,
        });

        let mut report = SyncTransferReport::default();
        for (index, (remote_path, profile_name)) in remote_profiles.into_iter().enumerate() {
            ensure_not_cancelled(observer.as_ref())?;
            let content = dav
                .get(&remote_path)
                .await
                .map_err(|error| PortError::Network(error.to_string()))?;
            let content = String::from_utf8(content)
                .map_err(|error| PortError::Failed(format!("远端配置不是 UTF-8 YAML: {error}")))?;
            infiltrator_domain::config::validate_yaml(&content)
                .map_err(|error| PortError::Failed(error.to_string()))?;

            let path = config_root.join(format!("{profile_name}.yaml"));
            if sandbox.validate_path(&path) != PathValidationResult::Allowed {
                return Err(PortError::Failed(format!(
                    "远端配置目标超出本地配置目录: {}",
                    path.display()
                )));
            }
            match tokio::fs::read_to_string(&path).await {
                Ok(local) if local == content => {
                    observer.progress(SyncProgress {
                        phase: "下载配置".to_string(),
                        current: (index + 1) as u64,
                        total,
                    });
                    continue;
                }
                Ok(_local) => {
                    let conflict_path = conflict_backup_path(&path);
                    atomic_write_file(&conflict_path, content.as_bytes()).await?;
                    report.conflicts += 1;
                    report.conflict_files.push(SyncConflict {
                        profile: profile_name,
                        remote_path: conflict_path.to_string_lossy().into_owned(),
                    });
                    observer.progress(SyncProgress {
                        phase: "下载配置".to_string(),
                        current: (index + 1) as u64,
                        total,
                    });
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PortError::Io(error.to_string())),
            }

            ensure_not_cancelled(observer.as_ref())?;
            manager
                .save(&profile_name, &content)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            let is_active = active_profile == profile_name;
            if is_active {
                report.active_profile_changed = true;
            }
            if !is_active || !runtime_present {
                manager
                    .clear_backup(&profile_name)
                    .await
                    .map_err(|error| PortError::Io(error.to_string()))?;
            }
            report.downloaded += 1;
            observer.progress(SyncProgress {
                phase: "下载配置".to_string(),
                current: (index + 1) as u64,
                total,
            });
        }
        Ok(report)
    }

    async fn read_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<String, PortError> {
        let path = validate_conflict_path(&configs_dir, &remote_path)?;
        tokio::fs::read_to_string(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn delete_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<(), PortError> {
        let path = validate_conflict_path(&configs_dir, &remote_path)?;
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

impl<S: SecureStore> FileWebDavSync<S> {
    fn manager(
        &self,
        configs_dir: Option<&str>,
    ) -> Result<ConfigManager<DefaultCredentialStore>, PortError> {
        ConfigManager::with_home_configs_dir_and_store(
            self.home.clone(),
            configs_dir,
            DefaultCredentialStore::default(),
        )
        .map_err(|error| PortError::Io(error.to_string()))
    }

    fn client(
        &self,
        config: &WebDavConfig,
        password: &str,
    ) -> Result<WebDavClient, PortError> {
        WebDavClient::new(&config.url, &config.username, password)
            .map_err(|error| PortError::Failed(format!("invalid WebDAV config: {error}")))
    }

    async fn password(&self, config: &WebDavConfig) -> String {
        if config.password.is_empty() {
            crate::settings_io::load_webdav_password(&self.secure_store)
                .await
                .unwrap_or_default()
        } else {
            config.password.clone()
        }
    }
}

fn resolve_configs_dir(configs_dir: Option<&str>, home: &Path) -> Result<PathBuf, PortError> {
    mihomo_config::manager::paths::resolve_configs_dir_in(configs_dir, home)
        .map_err(|error| PortError::Io(error.to_string()))
}

fn validate_conflict_path(configs_dir: &str, remote_path: &str) -> Result<PathBuf, PortError> {
    let root = std::fs::canonicalize(configs_dir)
        .map_err(|error| PortError::Io(format!("配置目录不可用: {error}")))?;
    let path = std::fs::canonicalize(remote_path)
        .map_err(|error| PortError::Io(format!("冲突文件不可用: {error}")))?;
    if !path.starts_with(&root) {
        return Err(PortError::Failed(format!(
            "冲突文件超出配置目录: {remote_path}"
        )));
    }
    Ok(path)
}

#[derive(Clone, Copy)]
enum SyncActionKind {
    Upload,
    Download,
    Conflict,
    Other,
}

fn action_kind(action: &SyncAction) -> SyncActionKind {
    match action {
        SyncAction::Upload { .. } => SyncActionKind::Upload,
        SyncAction::Download { .. } => SyncActionKind::Download,
        SyncAction::Conflict { .. } => SyncActionKind::Conflict,
        SyncAction::DeleteRemote { .. } | SyncAction::DeleteLocal { .. } => {
            SyncActionKind::Other
        }
    }
}

fn ensure_not_cancelled(observer: &dyn SyncProgressSink) -> Result<(), PortError> {
    if observer.is_cancelled() {
        Err(PortError::Failed("同步已取消".to_string()))
    } else {
        Ok(())
    }
}

fn safe_remote_profile_name(remote_path: &str) -> Result<Option<String>, PortError> {
    let trimmed = remote_path.trim_matches('/');
    if remote_path.contains('\\')
        || remote_path.contains("://")
        || trimmed.split('/').any(|part| part == "..")
    {
        return Err(PortError::Failed(format!(
            "拒绝不安全的远端配置路径: {remote_path}"
        )));
    }
    let Some(file_name) = trimmed.rsplit('/').next() else {
        return Ok(None);
    };
    if !file_name.ends_with(".yaml") && !file_name.ends_with(".yml") {
        return Ok(None);
    }
    if file_name == ".yaml" || file_name == ".yml" || file_name.contains("..") {
        return Err(PortError::Failed(format!(
            "拒绝不安全的远端配置路径: {remote_path}"
        )));
    }
    let profile_name = file_name
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or_default();
    infiltrator_domain::profiles::sanitize_profile_name(profile_name)
        .map(Some)
        .map_err(|error| PortError::Failed(error.to_string()))
}

fn conflict_backup_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("profile");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!("{stem}.remote-conflict-{stamp}.yaml"))
}

async fn atomic_write_file(path: &Path, content: &[u8]) -> Result<(), PortError> {
    let parent = path
        .parent()
        .ok_or_else(|| PortError::Io(format!("路径没有父目录: {}", path.display())))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let temp = path.with_file_name(format!(
        ".{}.sync-tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        #[cfg(windows)]
        if tokio::fs::try_exists(path).await? {
            tokio::fs::remove_file(path).await?;
        }
        tokio::fs::rename(&temp, path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    result.map_err(|error| PortError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_ports::sync::SyncPort;

    #[tokio::test]
    async fn conflict_file_access_reads_and_deletes_inside_the_config_root() {
        let temp = tempfile::tempdir().expect("temp root");
        let root = temp.path().join("configs");
        tokio::fs::create_dir_all(&root).await.expect("config root");
        let conflict = root.join("demo.remote-conflict.yaml");
        tokio::fs::write(&conflict, "mode: rule\n")
            .await
            .expect("conflict file");

        let adapter = FileWebDavSync::new(
            temp.path().to_path_buf(),
            DefaultCredentialStore::default(),
        );
        let content = adapter
            .read_conflict(
                root.to_string_lossy().into_owned(),
                conflict.to_string_lossy().into_owned(),
            )
            .await
            .expect("read conflict");
        assert_eq!(content, "mode: rule\n");
        adapter
            .delete_conflict(
                root.to_string_lossy().into_owned(),
                conflict.to_string_lossy().into_owned(),
            )
            .await
            .expect("delete conflict");
        assert!(!conflict.exists());
    }

    #[tokio::test]
    async fn conflict_file_access_rejects_a_file_outside_the_config_root() {
        let temp = tempfile::tempdir().expect("temp root");
        let root = temp.path().join("configs");
        let outside = temp.path().join("outside.yaml");
        tokio::fs::create_dir_all(&root).await.expect("config root");
        tokio::fs::write(&outside, "mode: direct\n")
            .await
            .expect("outside file");

        let adapter = FileWebDavSync::new(
            temp.path().to_path_buf(),
            DefaultCredentialStore::default(),
        );
        let error = adapter
            .read_conflict(
                root.to_string_lossy().into_owned(),
                outside.to_string_lossy().into_owned(),
            )
            .await
            .expect_err("outside conflict must be rejected");
        assert!(error.to_string().contains("超出配置目录"));
    }
}
