//! Configuration snapshot use-cases over profile and snapshot ports.

use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot_history::{
    SNAPSHOT_DEFAULT_KEEP, SnapshotEntry, SnapshotHistorySnapshot, SnapshotPruneReport,
    SnapshotPruneSource,
};
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

/// DUAL-09-06/07: process-wide cache of the snapshot history a surface just
/// loaded. The Bevy surface owns no storage port, so it renders exactly the
/// entries + prune view the shared application computed.
fn history_cache() -> &'static Mutex<Option<SnapshotHistorySnapshot>> {
    static HISTORY: OnceLock<Mutex<Option<SnapshotHistorySnapshot>>> = OnceLock::new();
    HISTORY.get_or_init(|| Mutex::new(None))
}

/// The last snapshot history computed in this process, if any.
pub fn last_snapshot_history() -> Option<SnapshotHistorySnapshot> {
    history_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Replace the process-wide snapshot history.
pub fn publish_snapshot_history(history: SnapshotHistorySnapshot) {
    if let Ok(mut cache) = history_cache().lock() {
        *cache = Some(history);
    }
}

/// Drop the cached history (profile switch, delete, restore).
pub fn clear_snapshot_history() {
    if let Ok(mut cache) = history_cache().lock() {
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
        // DUAL-09-06: refresh the shared history view as part of the create, so
        // both surfaces see the new entry (the create itself already succeeded —
        // a history refresh failure must not turn it into a failure).
        let _ = self.history(&meta.profile, SNAPSHOT_DEFAULT_KEEP).await;
        Ok(meta)
    }

    /// DUAL-09-06/07: build the shared history view of `profile` — newest first,
    /// with the shared prune decision (`pending_prune` / `duplicate_entries`) and
    /// the last prune report — and publish it for both surfaces.
    pub async fn history(
        &self,
        profile: &str,
        keep: usize,
    ) -> Result<SnapshotHistorySnapshot, Failure> {
        let keep = SnapshotHistorySnapshot::clamp_keep(keep);
        let mut snapshots = self.list(profile).await?;
        snapshots.sort_by_key(|meta| std::cmp::Reverse(meta.timestamp));
        let mut seen = std::collections::HashSet::new();
        let entries: Vec<SnapshotEntry> = snapshots
            .iter()
            .enumerate()
            .map(|(index, meta)| SnapshotEntry {
                id: meta.path.to_string_lossy().to_string(),
                file_name: meta
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                timestamp_millis: meta.timestamp.timestamp_millis(),
                sha256: meta.sha256.clone(),
                is_newest: index == 0,
                is_duplicate: !seen.insert(meta.sha256.clone()),
            })
            .collect();
        let pending_prune = infiltrator_domain::backup::prune_snapshots(&snapshots, keep).len();
        let duplicate_entries = entries.iter().filter(|entry| entry.is_duplicate).count();
        let history = SnapshotHistorySnapshot {
            profile: profile.to_string(),
            entries,
            keep_limit: keep,
            pending_prune,
            duplicate_entries,
            last_prune: last_snapshot_history()
                .filter(|cached| cached.profile == profile)
                .and_then(|cached| cached.last_prune),
        };
        publish_snapshot_history(history.clone());
        Ok(history)
    }

    /// DUAL-09-07: execute the shared dedupe+LRU prune now.
    ///
    /// Every deletion goes through the [`SnapshotStore`] identity check; a
    /// partial failure still publishes the honest post-prune history and then
    /// reports the first error.
    pub async fn prune(
        &self,
        profile: &str,
        keep: usize,
        source: SnapshotPruneSource,
    ) -> Result<SnapshotPruneReport, Failure> {
        let keep = SnapshotHistorySnapshot::clamp_keep(keep);
        let mut snapshots = self.list(profile).await?;
        snapshots.sort_by_key(|meta| std::cmp::Reverse(meta.timestamp));
        let doomed = infiltrator_domain::backup::prune_snapshots(&snapshots, keep);
        let mut removed = 0usize;
        let mut first_error = None;
        for path in doomed {
            match self.snapshots.delete(profile, Path::new(&path)).await {
                Ok(()) => removed += 1,
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        let report = SnapshotPruneReport {
            removed,
            keep_limit: keep,
            source,
        };
        if let Ok(mut history) = self.history(profile, keep).await {
            history.last_prune = Some(report);
            publish_snapshot_history(history);
        }
        match first_error {
            Some(error) => Err(Failure::from(error)),
            None => Ok(report),
        }
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
