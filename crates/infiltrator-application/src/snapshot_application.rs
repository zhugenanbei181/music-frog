//! Configuration snapshot use-cases over profile and snapshot ports.

use crate::profile_application::ProfileApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot_history::{
    SNAPSHOT_DEFAULT_KEEP, SnapshotEntry, SnapshotHistorySnapshot, SnapshotPruneReport,
    SnapshotPruneSource,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::backup::prune_snapshots;
use infiltrator_domain::myers_diff;
use infiltrator_domain::snapshots::{SnapshotMeta, content_hash};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::snapshot_store::SnapshotStore;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[path = "snapshot_application_test.rs"]
mod snapshot_application_test;

mod restore;

#[derive(Clone)]
pub struct SnapshotApplication {
    profiles: ProfileApplication,
    snapshots: Arc<dyn SnapshotStore>,
    observations: Arc<Mutex<SnapshotObservations>>,
    restores: Arc<Mutex<restore::RestoreState>>,
    restore_owner: u64,
}

#[derive(Default)]
struct SnapshotObservations {
    histories: BTreeMap<String, SnapshotHistorySnapshot>,
    differences: BTreeMap<String, ComparedSnapshot>,
}

struct ComparedSnapshot {
    document_hash: String,
    difference: YamlAstDiffSnapshot,
}

impl SnapshotApplication {
    pub fn new(profile_store: Arc<dyn ProfileStore>, snapshots: Arc<dyn SnapshotStore>) -> Self {
        Self {
            profiles: ProfileApplication::new(profile_store),
            snapshots,
            observations: Arc::default(),
            restores: Arc::default(),
            restore_owner: restore::allocate_owner(),
        }
    }

    pub fn history_observation(&self, profile: &str) -> Option<SnapshotHistorySnapshot> {
        self.observations
            .lock()
            .expect("snapshot observations")
            .histories
            .get(profile)
            .cloned()
    }
    pub fn diff_observation(&self, profile: &str, document: &str) -> Option<YamlAstDiffSnapshot> {
        let hash = content_hash(document.as_bytes());
        self.observations
            .lock()
            .expect("snapshot observations")
            .differences
            .get(profile)
            .filter(|observation| observation.document_hash == hash)
            .map(|observation| observation.difference.clone())
    }
    fn clear_diff(&self, profile: &str) {
        self.observations
            .lock()
            .expect("snapshot observations")
            .differences
            .remove(profile);
    }
    fn publish_history(&self, history: SnapshotHistorySnapshot) {
        self.observations
            .lock()
            .expect("snapshot observations")
            .histories
            .insert(history.profile.clone(), history);
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
        self.clear_diff(&meta.profile);
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
        snapshots.sort_by_key(|meta| Reverse(meta.timestamp));
        let mut seen = HashSet::new();
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
        let pending_prune = prune_snapshots(&snapshots, keep).len();
        let duplicate_entries = entries.iter().filter(|entry| entry.is_duplicate).count();
        let history = SnapshotHistorySnapshot {
            profile: profile.to_string(),
            entries,
            keep_limit: keep,
            pending_prune,
            duplicate_entries,
            last_prune: self
                .history_observation(profile)
                .and_then(|cached| cached.last_prune),
        };
        self.publish_history(history.clone());
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
        snapshots.sort_by_key(|meta| Reverse(meta.timestamp));
        let doomed = prune_snapshots(&snapshots, keep);
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
            self.publish_history(history);
        }
        match first_error {
            Some(error) => Err(Failure::from(error)),
            None => Ok(report),
        }
    }

    pub async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, Failure> {
        let identity = self.profiles.load_profile_info(profile).await?.name;
        let entries = self
            .snapshots
            .list(&identity)
            .await
            .map_err(Failure::from)?;
        if entries.iter().any(|entry| entry.profile != identity) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Snapshot store returned another profile's history",
                false,
            ));
        }
        Ok(entries)
    }

    pub async fn read(&self, profile: &str, path: &Path) -> Result<String, Failure> {
        self.snapshots
            .read(profile, path)
            .await
            .map_err(Failure::from)
    }

    /// Compute visual Myers AST difference between a historical snapshot and current profile content.
    ///
    /// The instance records the actual profile/document identity and snapshot path.
    /// Readers reject observations for different profiles or changed documents.
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
        self.observations
            .lock()
            .expect("snapshot observations")
            .differences
            .insert(
                current_detail.name.clone(),
                ComparedSnapshot {
                    document_hash: content_hash(current_detail.content.as_bytes()),
                    difference: diff.clone(),
                },
            );
        Ok(diff)
    }

    /// DUAL-09-08: diff the newest snapshot (if any) against the current
    /// content. `Ok(None)` means the profile has no history yet; the surfaces
    /// must render an honest empty state instead of a fabricated diff.
    pub async fn diff_newest(&self, profile: &str) -> Result<Option<YamlAstDiffSnapshot>, Failure> {
        let mut snapshots = self.list(profile).await?;
        snapshots.sort_by_key(|snapshot| Reverse(snapshot.timestamp));
        let Some(newest) = snapshots.first() else {
            self.clear_diff(profile);
            return Ok(None);
        };
        self.diff_snapshot(profile, &newest.path).await.map(Some)
    }
}
