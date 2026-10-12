//! File/keyring/WebDAV adapter for the runtime-neutral sync port.

use crate::remote_profile_namespace::{migrate_remote_profile_namespace, remote_profile_path};
use crate::settings_io::load_webdav_password;
use dav_client::DavClient;
use dav_client::client::WebDavClient;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::sync::{SyncConflict, SyncProgress, SyncReport, SyncTransferReport};
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::profiles::sanitize_profile_name;
use infiltrator_domain::sandbox::{PathValidationResult, SandboxValidator};
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_ports::sync::{SyncPort, SyncProgressSink, SyncRequest, SyncTransferRequest};
use mihomo_config::manager::ConfigManager;
use mihomo_config::manager::paths::engine_config_dir;
use state_store::StateStore;
use std::collections::HashSet;
use std::fs::canonicalize;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sync_engine::executor::SyncExecutor;
use sync_engine::{SyncAction, SyncPlanner};
#[cfg(windows)]
use tokio::fs::try_exists;
use tokio::fs::{OpenOptions, create_dir_all, read, read_to_string, remove_file, rename};
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
            get_home_dir()?,
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
        let manager = self.manager(request.configs_dir.as_deref()).await?;
        migrate_remote_profile_namespace(&dav, &manager.engine_id()).await?;
        let local_root = manager.config_dir().to_path_buf();
        create_dir_all(&local_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;

        let db_path = sync_state_path(manager.config_root(), &manager.engine_id());
        let store = StateStore::new(&db_path.to_string_lossy())
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let remote_root = format!("/{}", manager.engine_id().as_str());
        let planner = SyncPlanner::new(local_root, remote_root, &dav, &store);
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

    async fn upload(&self, request: SyncTransferRequest) -> Result<SyncTransferReport, PortError> {
        let SyncTransferRequest {
            config,
            configs_dir,
            observer,
            ..
        } = request;
        let password = self.password(&config).await;
        let dav = self.client(&config, &password)?;
        let manager = self.manager(configs_dir.as_deref()).await?;
        let engine = manager.engine_id();
        migrate_remote_profile_namespace(&dav, &engine).await?;
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
            let content = read(&profile.path)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            ensure_not_cancelled(observer.as_ref())?;
            dav.put(&remote_profile_path(&engine, &profile.name), &content, None)
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
            observer,
        } = request;
        let password = self.password(&config).await;
        let dav = self.client(&config, &password)?;
        let manager = self.manager(configs_dir.as_deref()).await?;
        let engine = manager.engine_id();
        migrate_remote_profile_namespace(&dav, &engine).await?;
        let files = dav
            .list(engine.as_str())
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let mut remote_profiles = Vec::new();
        let mut remote_names = HashSet::new();
        for file in files {
            if let Some(profile_name) = safe_remote_profile_name_in_engine(&file.path, &engine)? {
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
        let active_reference = ProfileStore::read_active_profile_reference(&manager)
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
            validate_yaml(&content).map_err(|error| PortError::Failed(error.to_string()))?;

            let path = config_root.join(format!("{profile_name}.yaml"));
            let selected_active = matches!(
                &active_reference,
                ActiveProfileReferenceState::Selected(reference)
                    if reference.engine_id == engine
                        && reference.profile_id.as_str() == profile_name
            );
            if sandbox.validate_path(&path) != PathValidationResult::Allowed {
                return Err(PortError::Failed(format!(
                    "远端配置目标超出本地配置目录: {}",
                    path.display()
                )));
            }
            match read_to_string(&path).await {
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
                Err(error) if error.kind() == ErrorKind::NotFound && selected_active => {
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
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(PortError::Io(error.to_string())),
            }

            ensure_not_cancelled(observer.as_ref())?;
            manager
                .save_inactive_profile(&profile_name, &content)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            manager
                .clear_backup(&profile_name)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
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
        let engine = mihomo_config::engine_id();
        let config_dir = scoped_conflict_root(Path::new(&configs_dir), &engine)?;
        let path = validate_conflict_path(&config_dir, &remote_path)?;
        read_to_string(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn delete_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<(), PortError> {
        let engine = mihomo_config::engine_id();
        let config_dir = scoped_conflict_root(Path::new(&configs_dir), &engine)?;
        let path = validate_conflict_path(&config_dir, &remote_path)?;
        remove_file(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

impl<S: SecureStore> FileWebDavSync<S> {
    async fn manager(
        &self,
        configs_dir: Option<&str>,
    ) -> Result<ConfigManager<DefaultCredentialStore>, PortError> {
        let manager = ConfigManager::with_home_configs_dir_and_store(
            self.home.clone(),
            configs_dir,
            DefaultCredentialStore::default(),
        )
        .map_err(|error| PortError::Io(error.to_string()))?;
        manager
            .initialize_profile_namespace()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        Ok(manager)
    }

    fn client(&self, config: &WebDavConfig, password: &str) -> Result<WebDavClient, PortError> {
        WebDavClient::new(&config.url, &config.username, password)
            .map_err(|error| PortError::Failed(format!("invalid WebDAV config: {error}")))
    }

    async fn password(&self, config: &WebDavConfig) -> String {
        if config.password.is_empty() {
            load_webdav_password(&self.secure_store)
                .await
                .unwrap_or_default()
        } else {
            config.password.clone()
        }
    }
}

fn scoped_conflict_root(
    configs_root_or_scoped: &Path,
    engine: &infiltrator_contract::core_identity::EngineId,
) -> Result<PathBuf, PortError> {
    let engine_name = engine.as_str();
    let file_name = configs_root_or_scoped
        .file_name()
        .and_then(|name| name.to_str());
    let parent_name = configs_root_or_scoped
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    match (parent_name, file_name) {
        (Some("engines"), Some(namespace)) if namespace == engine_name => {
            Ok(configs_root_or_scoped.to_path_buf())
        }
        (Some("engines"), Some(namespace)) => Err(PortError::PermissionDenied(format!(
            "snapshot/profile namespace `{namespace}` does not match `{engine_name}`"
        ))),
        _ => Ok(engine_config_dir(configs_root_or_scoped, engine)),
    }
}

fn sync_state_path(config_root: &Path, engine: &EngineId) -> PathBuf {
    engine_config_dir(config_root, engine).join("sync_state.db")
}

fn validate_conflict_path(configs_dir: &Path, remote_path: &str) -> Result<PathBuf, PortError> {
    let root = canonicalize(configs_dir)
        .map_err(|error| PortError::Io(format!("配置目录不可用: {error}")))?;
    let path = canonicalize(remote_path)
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
        SyncAction::DeleteRemote { .. } | SyncAction::DeleteLocal { .. } => SyncActionKind::Other,
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
    sanitize_profile_name(profile_name)
        .map(Some)
        .map_err(|error| PortError::Failed(error.to_string()))
}

fn safe_remote_profile_name_in_engine(
    remote_path: &str,
    engine: &infiltrator_contract::core_identity::EngineId,
) -> Result<Option<String>, PortError> {
    let trimmed = remote_path.trim_matches('/');
    let scoped = match trimmed.split_once('/') {
        Some((namespace, path)) if namespace == engine.as_str() => path,
        Some((_, _)) => return Ok(None),
        None => trimmed,
    };
    if scoped.contains('/') {
        return Err(PortError::Failed(format!(
            "拒绝不安全的远端配置路径: {remote_path}"
        )));
    }
    safe_remote_profile_name(scoped)
}

fn conflict_backup_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("profile");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!("{stem}.remote-conflict-{stamp}.yaml"))
}

async fn atomic_write_file(path: &Path, content: &[u8]) -> Result<(), PortError> {
    let parent = path
        .parent()
        .ok_or_else(|| PortError::Io(format!("路径没有父目录: {}", path.display())))?;
    create_dir_all(parent)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let temp = path.with_file_name(format!(
        ".{}.sync-tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ));
    let result = async {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        #[cfg(windows)]
        if try_exists(path).await? {
            remove_file(path).await?;
        }
        rename(&temp, path).await
    }
    .await;
    if result.is_err() {
        let _ = remove_file(&temp).await;
    }
    result.map_err(|error| PortError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_ports::sync::SyncPort;
    #[cfg(test)]
    use tokio::fs::write;

    #[tokio::test]
    async fn conflict_file_access_reads_and_deletes_inside_the_config_root() {
        let temp = tempfile::tempdir().expect("temp root");
        let root = temp.path().join("configs");
        let scoped = engine_config_dir(&root, &mihomo_config::engine_id());
        create_dir_all(&scoped).await.expect("config root");
        let conflict = scoped.join("demo.remote-conflict.yaml");
        write(&conflict, "mode: rule\n")
            .await
            .expect("conflict file");

        let adapter =
            FileWebDavSync::new(temp.path().to_path_buf(), DefaultCredentialStore::default());
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
        create_dir_all(engine_config_dir(&root, &mihomo_config::engine_id()))
            .await
            .expect("config root");
        write(&outside, "mode: direct\n")
            .await
            .expect("outside file");

        let adapter =
            FileWebDavSync::new(temp.path().to_path_buf(), DefaultCredentialStore::default());
        let error = adapter
            .read_conflict(
                root.to_string_lossy().into_owned(),
                outside.to_string_lossy().into_owned(),
            )
            .await
            .expect_err("outside conflict must be rejected");
        assert!(error.to_string().contains("超出配置目录"));
    }

    #[test]
    fn conflict_file_root_rejects_a_foreign_engine_namespace() {
        let root = PathBuf::from("/shared/configs/engines/sing-box");
        let error = scoped_conflict_root(&root, &mihomo_config::engine_id()).unwrap_err();
        assert!(matches!(error, PortError::PermissionDenied(_)));
    }

    #[test]
    fn remote_profile_listing_ignores_another_engine_namespace() {
        let engine = mihomo_config::engine_id();
        assert_eq!(
            safe_remote_profile_name_in_engine("mihomo/main.yaml", &engine).unwrap(),
            Some("main".into())
        );
        assert_eq!(
            safe_remote_profile_name_in_engine("sing-box/main.yaml", &engine).unwrap(),
            None
        );
        assert_eq!(
            safe_remote_profile_name_in_engine("main.yaml", &engine).unwrap(),
            Some("main".into())
        );
    }

    #[test]
    fn same_profile_engine_remote_paths_are_disjoint() {
        let mihomo = EngineId::new("mihomo").expect("valid engine ID");
        let sing_box = EngineId::new("sing-box").expect("valid engine ID");
        assert_eq!(remote_profile_path(&mihomo, "main"), "mihomo/main.yaml");
        assert_eq!(remote_profile_path(&sing_box, "main"), "sing-box/main.yaml");
        assert_ne!(
            remote_profile_path(&mihomo, "main"),
            remote_profile_path(&sing_box, "main")
        );
    }

    #[tokio::test]
    async fn same_profile_engine_sync_indexes_use_disjoint_paths() {
        let temp = tempfile::tempdir().expect("temp root");
        let root = temp.path().join("configs");
        let mihomo = sync_state_path(&root, &EngineId::new("mihomo").expect("valid engine ID"));
        let sing_box = sync_state_path(&root, &EngineId::new("sing-box").expect("valid engine ID"));
        create_dir_all(mihomo.parent().unwrap()).await.unwrap();
        create_dir_all(sing_box.parent().unwrap()).await.unwrap();
        write(&mihomo, b"mihomo index").await.unwrap();
        write(&sing_box, b"sing-box index").await.unwrap();

        assert_ne!(mihomo, sing_box);
        assert_eq!(read(&mihomo).await.unwrap(), b"mihomo index");
        assert_eq!(read(&sing_box).await.unwrap(), b"sing-box index");
    }
}
