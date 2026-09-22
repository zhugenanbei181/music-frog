//! Configuration snapshot use-cases over profile and snapshot ports.

use infiltrator_contract::error::Failure;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::myers_diff;
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_ports::snapshot_store::SnapshotStore;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::profile_application::ProfileApplication;

#[cfg(test)]
#[path = "snapshot_application_test.rs"]
mod snapshot_application_test;

#[derive(Clone)]
pub struct SnapshotApplication {
    profiles: ProfileApplication,
    snapshots: Arc<dyn SnapshotStore>,
}

/// DUAL-09-08: process-wide cache of the most recent snapshot diff so the
/// Bevy surface (which owns no storage port) can project the same diff the
/// Iced surface computes on demand.
fn diff_cache() -> &'static Mutex<Option<YamlAstDiffSnapshot>> {
    static DIFF: OnceLock<Mutex<Option<YamlAstDiffSnapshot>>> = OnceLock::new();
    DIFF.get_or_init(|| Mutex::new(None))
}

/// The last snapshot diff computed in this process, if any.
pub fn last_snapshot_diff() -> Option<YamlAstDiffSnapshot> {
    diff_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Replace the process-wide snapshot diff.
pub fn publish_snapshot_diff(diff: YamlAstDiffSnapshot) {
    if let Ok(mut cache) = diff_cache().lock() {
        *cache = Some(diff);
    }
}

/// Drop the cached diff (a restore or a fresh snapshot makes it stale).
pub fn clear_snapshot_diff() {
    if let Ok(mut cache) = diff_cache().lock() {
        *cache = None;
    }
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
        let meta = self
            .snapshots
            .save(&detail.name, &detail.content)
            .await
            .map_err(Failure::from)?;
        // The new snapshot becomes the newest candidate; a cached diff no
        // longer describes "newest snapshot vs current".
        clear_snapshot_diff();
        Ok(meta)
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

    /// Compute visual Myers AST difference between a historical snapshot and current profile content.
    ///
    /// The result is published process-wide (with the snapshot identity
    /// attached) so both surfaces can render the same diff.
    pub async fn diff_snapshot(
        &self,
        profile: &str,
        snapshot_path: &Path,
    ) -> Result<YamlAstDiffSnapshot, Failure> {
        let current_detail = self.profiles.load_profile_detail(profile).await?;
        let snapshot_content = self.read(profile, snapshot_path).await?;
        let snap_label = snapshot_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("historical_snapshot");
        let diff = myers_diff::compute_diff(
            &snapshot_content,
            &current_detail.content,
            snap_label,
            &current_detail.name,
        )
        .with_source_path(snapshot_path.to_string_lossy().to_string());
        publish_snapshot_diff(diff.clone());
        Ok(diff)
    }

    /// DUAL-09-08: diff the newest snapshot (if any) against the current
    /// content. `Ok(None)` means the profile has no history yet; the surfaces
    /// must render an honest empty state instead of a fabricated diff.
    pub async fn diff_newest(&self, profile: &str) -> Result<Option<YamlAstDiffSnapshot>, Failure> {
        let mut snapshots = self.list(profile).await?;
        snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.timestamp));
        let Some(newest) = snapshots.first() else {
            clear_snapshot_diff();
            return Ok(None);
        };
        self.diff_snapshot(profile, &newest.path).await.map(Some)
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
            .await?;
        // The live content now equals the snapshot: any cached diff is stale.
        clear_snapshot_diff();
        Ok(())
    }
}
