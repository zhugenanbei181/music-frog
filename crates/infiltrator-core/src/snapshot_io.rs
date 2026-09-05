//! Filesystem adapter for configuration snapshot history.

use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::error::PortError;
use infiltrator_ports::snapshot_store::SnapshotStore;
use std::path::{Path, PathBuf};

pub struct FileSnapshotStore {
    config_dir: PathBuf,
}

impl FileSnapshotStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub async fn current() -> anyhow::Result<Self> {
        Ok(Self::new(crate::profile_store_io::config_dir().await?))
    }

    async fn safe_path(&self, profile: &str, path: &Path) -> Result<PathBuf, PortError> {
        let root = crate::history::snapshot_dir(&self.config_dir, profile);
        let root = tokio::fs::canonicalize(&root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let path = tokio::fs::canonicalize(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        if !path.starts_with(&root) {
            return Err(PortError::PermissionDenied(
                "snapshot path is outside the profile history directory".to_string(),
            ));
        }
        Ok(path)
    }
}

#[async_trait::async_trait]
impl SnapshotStore for FileSnapshotStore {
    async fn save(&self, profile: &str, content: &str) -> Result<SnapshotMeta, PortError> {
        crate::history::save_snapshot(&self.config_dir, profile, content)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError> {
        crate::history::list_snapshots(&self.config_dir, profile)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError> {
        let path = self.safe_path(profile, path).await?;
        crate::history::read_snapshot(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}
