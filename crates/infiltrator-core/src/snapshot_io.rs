//! Filesystem adapter for configuration snapshot history.

use crate::history::{list_snapshots, read_snapshot, save_snapshot, snapshot_dir};
use crate::profile_store_io::open;
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::error::PortError;
use infiltrator_ports::snapshot_store::SnapshotStore;
use lurker_core_contract::identity::EngineId;
use mihomo_config::manager::paths::engine_config_dir;
use std::path::{Path, PathBuf};
use tokio::fs::{canonicalize, remove_file};

pub struct FileSnapshotStore {
    engine_id: EngineId,
    config_dir: PathBuf,
}

impl FileSnapshotStore {
    /// Construct the current Mihomo history store from either a shared config
    /// root or an already engine-scoped path. A sibling namespace path is
    /// normalized back to this adapter's own namespace.
    pub fn new(config_root_or_scoped: PathBuf) -> Self {
        let engine_id = mihomo_config::engine_id();
        let config_dir = scoped_root_for_current_engine(&config_root_or_scoped, &engine_id);
        Self::new_scoped(engine_id, config_dir)
    }

    /// Resolve one engine's history root below a shared config directory.
    pub fn for_engine(engine_id: EngineId, config_root: PathBuf) -> Self {
        let config_dir = engine_config_dir(&config_root, &engine_id);
        Self {
            engine_id,
            config_dir,
        }
    }

    fn new_scoped(engine_id: EngineId, config_dir: PathBuf) -> Self {
        Self {
            engine_id,
            config_dir,
        }
    }

    pub async fn current() -> anyhow::Result<Self> {
        let profiles = open().await?;
        Ok(Self::new_scoped(
            profiles.engine_id(),
            profiles.config_dir(),
        ))
    }

    async fn safe_path(&self, profile: &str, path: &Path) -> Result<PathBuf, PortError> {
        let root = snapshot_dir(&self.config_dir, profile);
        let root = canonicalize(&root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let path = canonicalize(path)
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

fn scoped_root_for_current_engine(path: &Path, engine_id: &EngineId) -> PathBuf {
    let parent_is_engines = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("engines");
    if parent_is_engines {
        let config_root = path.parent().and_then(Path::parent).unwrap_or(path);
        engine_config_dir(config_root, engine_id)
    } else {
        engine_config_dir(path, engine_id)
    }
}

#[async_trait::async_trait]
impl SnapshotStore for FileSnapshotStore {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn save(&self, profile: &str, content: &str) -> Result<SnapshotMeta, PortError> {
        save_snapshot(&self.engine_id, &self.config_dir, profile, content)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError> {
        list_snapshots(&self.engine_id, &self.config_dir, profile)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError> {
        let path = self.safe_path(profile, path).await?;
        read_snapshot(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn delete(&self, profile: &str, path: &Path) -> Result<(), PortError> {
        // `safe_path` canonicalizes first, so the shared prune policy can only
        // ever delete inside `<config_dir>/snapshots/<profile>`.
        let path = self.safe_path(profile, path).await?;
        remove_file(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::FileSnapshotStore;
    use infiltrator_ports::snapshot_store::SnapshotStore;
    use lurker_core_contract::identity::EngineId;

    #[tokio::test]
    async fn same_profile_id_has_engine_scoped_snapshot_history() {
        let root = tempfile::tempdir().unwrap();
        let mihomo = FileSnapshotStore::for_engine(
            EngineId::new("mihomo").unwrap(),
            root.path().to_path_buf(),
        );
        let sing_box = FileSnapshotStore::for_engine(
            EngineId::new("sing-box").unwrap(),
            root.path().to_path_buf(),
        );

        let mihomo_snapshot = mihomo.save("main", "mode: rule\n").await.unwrap();
        let sing_box_snapshot = sing_box
            .save("main", "{\"log\": {\"level\": \"info\"}}")
            .await
            .unwrap();

        assert_eq!(mihomo.engine_id().as_str(), "mihomo");
        assert_eq!(sing_box.engine_id().as_str(), "sing-box");
        assert_ne!(mihomo_snapshot.path, sing_box_snapshot.path);
        assert_eq!(
            mihomo.list("main").await.unwrap()[0].path,
            mihomo_snapshot.path
        );
        assert_eq!(
            sing_box.list("main").await.unwrap()[0].path,
            sing_box_snapshot.path
        );
        assert_eq!(
            mihomo.read("main", &mihomo_snapshot.path).await.unwrap(),
            "mode: rule\n"
        );
        assert_eq!(
            sing_box
                .read("main", &sing_box_snapshot.path)
                .await
                .unwrap(),
            "{\"log\": {\"level\": \"info\"}}"
        );
    }

    #[tokio::test]
    async fn current_snapshot_constructor_scopes_shared_roots_to_mihomo() {
        let root = tempfile::tempdir().unwrap();
        let store = FileSnapshotStore::new(root.path().to_path_buf());
        let snapshot = store.save("main", "mode: rule\n").await.unwrap();

        assert_eq!(store.engine_id(), mihomo_config::engine_id());
        assert!(
            snapshot
                .path
                .starts_with(root.path().join("engines/mihomo/snapshots/main"))
        );
    }
}
