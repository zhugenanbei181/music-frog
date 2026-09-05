//! File/keyring/WebDAV adapter for the runtime-neutral sync port.

use dav_client::{DavClient, client::WebDavClient};
use infiltrator_contract::sync::SyncReport;
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_ports::sync::{SyncPort, SyncRequest};
use mihomo_platform::defaults::DefaultCredentialStore;
use std::path::{Path, PathBuf};
use state_store::StateStore;
use sync_engine::{SyncAction, SyncPlanner, executor::SyncExecutor};

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
}

impl<S: SecureStore> FileWebDavSync<S> {
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
