//! Isolated memory adapters for both peers' native restoration tests and pixel captures.
use async_trait::async_trait;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_domain::profile_source::{hash_bytes, hash_document_bytes};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::snapshots::{SnapshotMeta, content_hash};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::snapshot_store::SnapshotStore;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const BEFORE: &str = "# Current profile\nmode: global\nproxies: []\nrules: [MATCH,DIRECT]\n";
pub const AFTER: &str =
    "# Reviewed historical profile\nmode: rule\nproxies: []\nrules: [MATCH,DIRECT]\n";
const SNAPSHOT: &str = "/memory/snapshots/main/1-reviewed.yaml";
#[derive(Default)]
struct MemoryState {
    content: String,
    sidecar_bytes: Option<Vec<u8>>,
    writes: usize,
    denied: bool,
}
#[derive(Clone)]
pub struct SnapshotRestoreFixture {
    state: Arc<Mutex<MemoryState>>,
    profile_store: Arc<MemoryProfile>,
    pub application: SnapshotApplication,
    pub target: SnapshotRestoreTarget,
}
impl Default for SnapshotRestoreFixture {
    fn default() -> Self {
        Self::new()
    }
}
impl SnapshotRestoreFixture {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(MemoryState {
            content: BEFORE.into(),
            ..Default::default()
        }));
        let store = Arc::new(MemoryProfile(state.clone()));
        Self {
            application: SnapshotApplication::new(store.clone(), Arc::new(MemorySnapshot)),
            state,
            profile_store: store,
            target: SnapshotRestoreTarget {
                profile: "main".into(),
                snapshot_id: SNAPSHOT.into(),
            },
        }
    }
    pub fn writes(&self) -> usize {
        self.state.lock().unwrap().writes
    }
    pub fn profile_application(&self) -> ProfileApplication {
        ProfileApplication::new(self.profile_store.clone())
    }
    pub fn content(&self) -> String {
        self.state.lock().unwrap().content.clone()
    }
    pub fn deny(&self, denied: bool) {
        self.state.lock().unwrap().denied = denied;
    }
    pub fn replace_options(&self, document: Option<&str>) {
        self.state.lock().unwrap().sidecar_bytes = document.map(|text| text.as_bytes().to_vec());
    }
    pub fn replace_source(&self, content: &str) {
        self.state.lock().unwrap().content = content.into();
    }
}
struct MemoryProfile(Arc<Mutex<MemoryState>>);
fn scoped(profile: &str) -> Result<(), PortError> {
    if profile == "main" {
        Ok(())
    } else {
        Err(PortError::NotFound(profile.into()))
    }
}
fn unsupported() -> PortError {
    PortError::unsupported(
        Capability::Profiles,
        "This isolated restoration adapter implements only reviewed commits",
    )
}
fn workspace(content: &str, sidecar_bytes: Option<&[u8]>) -> ProfileWorkspace {
    let profile_id = ProfileId::new("main").expect("fixture profile ID");
    let engine_id = EngineId::new("mihomo").expect("fixture engine ID");
    let source_id = ProfileSourceId::new("local-snapshot-fixture").expect("fixture source ID");
    let identity = ProfileDocumentIdentity::new(
        profile_id,
        engine_id,
        ProfileDocumentSourceIdentity::new(ProfileDocumentSourceKind::Local, source_id),
        ProfileDocumentRevision::new(hash_document_bytes(content))
            .expect("fixture document revision"),
        ProfileFormatId::new("mihomo-yaml").expect("fixture format ID"),
        ProfileSchemaVersion::new(1).expect("fixture schema version"),
    );
    let sidecar_revision = sidecar_bytes
        .map(hash_bytes)
        .map(|revision| {
            ProfileDocumentRevision::new(revision)
                .map(ProfileSidecarRevision::Present)
                .expect("fixture sidecar revision")
        })
        .unwrap_or(ProfileSidecarRevision::Absent);
    ProfileWorkspace {
        document: ProfileDocument::new(identity, content.as_bytes().to_vec()),
        sidecar_revision,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes: sidecar_bytes.map(<[u8]>::to_vec),
    }
}
#[async_trait]
impl ProfileStore for MemoryProfile {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("fixture engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        "/memory/configs".into()
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(vec![ProfileInfo {
            name: "main".into(),
            active: true,
            path: "/memory/configs/main.yaml".into(),
            ..Default::default()
        }])
    }
    async fn get_current(&self) -> Result<String, PortError> {
        Ok("main".into())
    }
    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(0),
        })
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        scoped(profile)?;
        Ok(self.0.lock().unwrap().content.clone())
    }
    async fn save_inactive_profile(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn delete_profile(&self, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        scoped(profile)?;
        Ok(Default::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        scoped(profile.as_str())?;
        let state = self.0.lock().unwrap();
        Ok(workspace(&state.content, state.sidecar_bytes.as_deref()))
    }
    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        scoped(expected.document.profile_id.as_str())?;
        let mut state = self.0.lock().unwrap();
        if state.denied {
            return Err(PortError::PermissionDenied(
                "Memory fixture denied profile access".into(),
            ));
        }
        if workspace(&state.content, state.sidecar_bytes.as_deref()).identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The reviewed profile source changed",
                false,
            )));
        }
        state.content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        state.sidecar_bytes = update.sidecar_bytes.clone();
        state.writes += 1;
        Ok(workspace(&state.content, state.sidecar_bytes.as_deref()))
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn restore_inactive_profile_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(unsupported())
    }
}
struct MemorySnapshot;
#[async_trait]
impl SnapshotStore for MemorySnapshot {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("fixture engine ID")
    }

    async fn save(&self, _: &str, _: &str) -> Result<SnapshotMeta, PortError> {
        Err(unsupported())
    }
    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError> {
        scoped(profile)?;
        Ok(vec![SnapshotMeta {
            profile: profile.into(),
            path: SNAPSHOT.into(),
            timestamp: Default::default(),
            sha256: content_hash(AFTER.as_bytes()),
        }])
    }
    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError> {
        scoped(profile)?;
        if path == Path::new(SNAPSHOT) {
            Ok(AFTER.into())
        } else {
            Err(PortError::NotFound(path.to_string_lossy().into_owned()))
        }
    }
    async fn delete(&self, _: &str, _: &Path) -> Result<(), PortError> {
        Err(unsupported())
    }
}
