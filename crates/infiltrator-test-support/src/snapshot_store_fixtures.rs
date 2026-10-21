//! Isolated snapshot-store port fixture.
//!
//! The store keeps its snapshots in memory and publishes them through the
//! typed [`SnapshotStore`] port. The saved name follows the shared
//! `<unix-millis>-<8 hex>.yaml` convention, the metadata carries the real
//! content digest, and a delete outside the profile's own history directory
//! is refused instead of removing an arbitrary path.

use async_trait::async_trait;
use chrono::DateTime;
use infiltrator_domain::snapshots::{SnapshotMeta, content_hash};
use infiltrator_ports::snapshot_store::SnapshotStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The engine the capture store owns.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The profile the capture store publishes history for.
pub const CAPTURE_PROFILE: &str = "capture-profile";
/// The root directory the capture store publishes.
pub const CAPTURE_ROOT: &str = "/isolated-snapshots";
/// The first timestamp the fixture hands out.
pub const CAPTURE_FIRST_TIMESTAMP_MILLIS: i64 = 1_760_000_000_000;

/// An isolated snapshot store that publishes real snapshot metadata.
/// The saved snapshots of one profile: the file path and its content.
type SavedSnapshots = BTreeMap<PathBuf, (SnapshotMeta, String)>;

pub struct SnapshotStoreCapture {
    engine_id: EngineId,
    snapshots: Mutex<BTreeMap<String, SavedSnapshots>>,
    next_timestamp: Mutex<i64>,
}

impl Default for SnapshotStoreCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStoreCapture {
    pub fn new() -> Self {
        Self {
            engine_id: EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID"),
            snapshots: Mutex::new(BTreeMap::new()),
            next_timestamp: Mutex::new(CAPTURE_FIRST_TIMESTAMP_MILLIS),
        }
    }

    /// The snapshot metadata the store currently publishes for `profile`.
    pub fn snapshot_list(&self, profile: &str) -> Vec<SnapshotMeta> {
        self.snapshots
            .lock()
            .expect("snapshot capture state")
            .get(profile)
            .map(|entries| {
                let mut metas: Vec<SnapshotMeta> =
                    entries.values().map(|(meta, _)| meta.clone()).collect();
                metas.sort_by_key(|meta| meta.timestamp);
                metas
            })
            .unwrap_or_default()
    }

    fn history_directory(&self, profile: &str) -> PathBuf {
        PathBuf::from(CAPTURE_ROOT).join(profile)
    }

    fn require_history_path(&self, profile: &str, path: &Path) -> Result<(), PortError> {
        let directory = self.history_directory(profile);
        if path.parent() == Some(directory.as_path()) {
            return Ok(());
        }
        Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidInput,
            format!("snapshot {path:?} is outside the history directory of profile {profile}"),
            false,
        )))
    }
}

#[async_trait]
impl SnapshotStore for SnapshotStoreCapture {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn save(&self, profile: &str, content: &str) -> Result<SnapshotMeta, PortError> {
        let millis = {
            let mut next = self
                .next_timestamp
                .lock()
                .expect("snapshot capture timestamp");
            let value = *next;
            *next += 1;
            value
        };
        let digest = content_hash(content.as_bytes());
        let name = format!("{millis}-{}.yaml", &digest[..8]);
        let path = self.history_directory(profile).join(&name);
        let timestamp = DateTime::from_timestamp_millis(millis)
            .ok_or_else(|| PortError::Io("snapshot timestamp is out of range".into()))?;
        let meta = SnapshotMeta {
            profile: profile.to_owned(),
            timestamp,
            sha256: digest,
            path: path.clone(),
        };
        self.snapshots
            .lock()
            .expect("snapshot capture state")
            .entry(profile.to_owned())
            .or_default()
            .insert(path, (meta.clone(), content.to_owned()));
        Ok(meta)
    }

    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError> {
        Ok(self.snapshot_list(profile))
    }

    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError> {
        self.require_history_path(profile, path)?;
        self.snapshots
            .lock()
            .expect("snapshot capture state")
            .get(profile)
            .and_then(|entries| entries.get(path))
            .map(|(_, content)| content.clone())
            .ok_or_else(|| PortError::NotFound(format!("no snapshot at {path:?}")))
    }

    async fn delete(&self, profile: &str, path: &Path) -> Result<(), PortError> {
        self.require_history_path(profile, path)?;
        let removed = self
            .snapshots
            .lock()
            .expect("snapshot capture state")
            .get_mut(profile)
            .and_then(|entries| entries.remove(path));
        if removed.is_none() {
            return Err(PortError::NotFound(format!("no snapshot at {path:?}")));
        }
        Ok(())
    }
}
