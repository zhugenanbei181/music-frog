//! Configuration snapshot use-cases over profile and snapshot ports.

use infiltrator_contract::error::Failure;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_ports::snapshot_store::SnapshotStore;
use std::path::Path;
use std::sync::Arc;

use crate::profile_application::ProfileApplication;

#[derive(Clone)]
pub struct SnapshotApplication {
    profiles: ProfileApplication,
    snapshots: Arc<dyn SnapshotStore>,
}

impl SnapshotApplication {
    pub fn new(profile_store: Arc<dyn ProfileStore>, snapshots: Arc<dyn SnapshotStore>) -> Self {
        Self {
            profiles: ProfileApplication::new(profile_store),
            snapshots,
        }
    }

    pub async fn create_current(&self) -> Result<SnapshotMeta, Failure> {
        let profile = self.profiles.current_profile().await?;
        self.create(&profile).await
    }

    pub async fn create(&self, profile: &str) -> Result<SnapshotMeta, Failure> {
        let detail = self.profiles.load_profile_detail(profile).await?;
        self.snapshots
            .save(&detail.name, &detail.content)
            .await
            .map_err(Failure::from)
    }

    pub async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, Failure> {
        self.snapshots.list(profile).await.map_err(Failure::from)
    }

    pub async fn read(&self, profile: &str, path: &Path) -> Result<String, Failure> {
        self.snapshots
            .read(profile, path)
            .await
            .map_err(Failure::from)
    }

    pub async fn restore<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: &str,
        path: &Path,
    ) -> Result<(), Failure> {
        let profile = self.profiles.load_profile_info(profile).await?.name;
        let content = self.read(&profile, path).await?;
        self.profiles
            .save_profile_content(runtime, profile, content, ApplyStrategy::PreferReload)
            .await
    }
}
