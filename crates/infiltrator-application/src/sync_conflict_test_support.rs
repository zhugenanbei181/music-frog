//! Test fixtures for the WebDAV conflict-resolution use case.
//!
//! The fake sync owner owns one pending conflict and its remote copy; the fake
//! profile owner records every local write. Both are in-memory so the use
//! case can be exercised without a WebDAV server or a profile directory.

use async_trait::async_trait;
use infiltrator_contract::sync::{SyncConflict, SyncReport, SyncTransferReport};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::sync::{SyncConflictSnapshot, SyncPort, SyncRequest, SyncTransferRequest};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the fake owner does with the pending conflict when the remote copy is
/// discarded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiscardBehaviour {
    /// The pending conflict is cleared together with the copy.
    #[default]
    ClearsTheConflict,
    /// The pending conflict survives the discarded copy.
    KeepsTheConflict,
}

/// The fake sync owner's own state.
#[derive(Default)]
pub struct FakeSyncState {
    conflict: Option<SyncConflictSnapshot>,
    successor: Option<SyncConflictSnapshot>,
    reads: usize,
    remote_content: String,
    discarded: bool,
    discard_behaviour: DiscardBehaviour,
}

/// The fake sync owner: it owns one pending conflict and its remote copy.
#[derive(Default)]
pub struct FakeSyncPort {
    state: Mutex<FakeSyncState>,
}

impl FakeSyncPort {
    /// A fake owner that holds one conflict whose remote copy carries
    /// `remote_content`.
    pub fn with_conflict(remote_content: &str) -> Self {
        let owner = Self::default();
        {
            let mut state = owner.state.lock().expect("state lock");
            state.conflict = Some(SyncConflictSnapshot {
                conflict: SyncConflict {
                    profile: "main".to_owned(),
                    remote_path: "/fake/configs/main.remote-conflict.yaml".to_owned(),
                },
                revision: 1,
            });
            state.remote_content = remote_content.to_owned();
            state.discard_behaviour = DiscardBehaviour::ClearsTheConflict;
        }
        owner
    }

    /// A fake owner that holds no conflict at all.
    pub fn without_conflict() -> Self {
        Self::default()
    }

    /// The owner moves on to `successor` once it published its first read.
    pub fn publishing_successor(&self, successor: SyncConflict) {
        let mut state = self.state.lock().expect("state lock");
        state.successor = Some(SyncConflictSnapshot {
            conflict: successor,
            revision: 2,
        });
    }

    /// The owner keeps its pending conflict when the remote copy is discarded.
    pub fn keeping_the_conflict_after_discard(&self) {
        let mut state = self.state.lock().expect("state lock");
        state.discard_behaviour = DiscardBehaviour::KeepsTheConflict;
    }

    /// Whether the owner discarded the remote copy.
    pub fn discarded(&self) -> bool {
        self.state.lock().expect("state lock").discarded
    }
}

#[async_trait]
impl SyncPort for FakeSyncPort {
    async fn test(&self, _config: WebDavConfig) -> Result<usize, PortError> {
        Ok(0)
    }

    async fn sync(&self, _request: SyncRequest) -> Result<SyncReport, PortError> {
        Ok(SyncReport::default())
    }

    async fn upload(&self, _request: SyncTransferRequest) -> Result<SyncTransferReport, PortError> {
        Ok(SyncTransferReport::default())
    }

    async fn download(
        &self,
        _request: SyncTransferRequest,
    ) -> Result<SyncTransferReport, PortError> {
        Ok(SyncTransferReport::default())
    }

    async fn read_conflict(
        &self,
        _configs_dir: String,
        _remote_path: String,
    ) -> Result<String, PortError> {
        Ok(self
            .state
            .lock()
            .expect("state lock")
            .remote_content
            .clone())
    }

    async fn delete_conflict(
        &self,
        _configs_dir: String,
        _remote_path: String,
    ) -> Result<(), PortError> {
        let mut state = self.state.lock().expect("state lock");
        state.discarded = true;
        if state.discard_behaviour == DiscardBehaviour::ClearsTheConflict {
            state.conflict = None;
        }
        Ok(())
    }

    async fn current_conflict(&self) -> Result<Option<SyncConflictSnapshot>, PortError> {
        let mut state = self.state.lock().expect("state lock");
        state.reads += 1;
        if state.reads > 1
            && let Some(successor) = state.successor.clone()
        {
            state.conflict = Some(successor);
        }
        Ok(state.conflict.clone())
    }
}

/// The fake profile owner: it records every local write.
#[derive(Default)]
pub struct FakeProfileStore {
    content: Mutex<String>,
    saves: AtomicUsize,
}

impl FakeProfileStore {
    /// A fake owner whose profile holds `content`.
    pub fn with_content(content: &str) -> Self {
        let store = Self::default();
        *store.content.lock().expect("content lock") = content.to_owned();
        store
    }

    /// The local profile's current content.
    pub fn content(&self) -> String {
        self.content.lock().expect("content lock").clone()
    }

    /// How many times the local profile was written.
    pub fn saves(&self) -> usize {
        self.saves.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileStore for FakeProfileStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(Vec::new())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok("main".to_owned())
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::default(),
        })
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(self.content())
    }

    async fn save_inactive_profile(&self, _profile: &str, content: &str) -> Result<(), PortError> {
        *self.content.lock().expect("content lock") = content.to_owned();
        self.saves.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}
