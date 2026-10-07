//! Isolated memory adapters for both peers' native restoration tests and pixel captures.
use async_trait::async_trait;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::identify_profile_source;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::snapshots::{SnapshotMeta, content_hash};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::snapshot_store::SnapshotStore;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const BEFORE: &str = "# Current profile\nmode: global\nproxies: []\nrules: [MATCH,DIRECT]\n";
pub const AFTER: &str =
    "# Reviewed historical profile\nmode: rule\nproxies: []\nrules: [MATCH,DIRECT]\n";
const SNAPSHOT: &str = "/memory/snapshots/main/1-reviewed.yaml";
#[derive(Default)]
struct MemoryState {
    content: String,
    options_document: Option<String>,
    writes: usize,
    denied: bool,
}
#[derive(Clone)]
pub struct SnapshotRestoreFixture {
    state: Arc<Mutex<MemoryState>>,
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
            application: SnapshotApplication::new(store, Arc::new(MemorySnapshot)),
            state,
            target: SnapshotRestoreTarget {
                profile: "main".into(),
                snapshot_id: SNAPSHOT.into(),
            },
        }
    }
    pub fn writes(&self) -> usize {
        self.state.lock().unwrap().writes
    }
    pub fn content(&self) -> String {
        self.state.lock().unwrap().content.clone()
    }
    pub fn deny(&self, denied: bool) {
        self.state.lock().unwrap().denied = denied;
    }
    pub fn replace_options(&self, document: Option<&str>) {
        self.state.lock().unwrap().options_document = document.map(str::to_owned);
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
fn workspace(content: &str, options: Option<&str>) -> ProfileWorkspace {
    ProfileWorkspace {
        write_protection: ProfileWriteProtection::Editable,
        source: identify_profile_source("main".into(), content, options),
        content: content.into(),
        options: Default::default(),
        options_document: options.map(str::to_owned),
    }
}
#[async_trait]
impl ProfileStore for MemoryProfile {
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
    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        scoped(profile)
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        scoped(profile)?;
        Ok(self.0.lock().unwrap().content.clone())
    }
    async fn save(&self, _: &str, _: &str) -> Result<(), PortError> {
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
    async fn load_options(&self, profile: &str) -> Result<ProfileOptions, PortError> {
        scoped(profile)?;
        Ok(Default::default())
    }
    async fn save_options(&self, _: &str, _: &ProfileOptions) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn delete_options(&self, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn load_workspace(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        scoped(profile)?;
        let state = self.0.lock().unwrap();
        Ok(workspace(&state.content, state.options_document.as_deref()))
    }
    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        scoped(&expected.profile)?;
        let mut state = self.0.lock().unwrap();
        if state.denied {
            return Err(PortError::PermissionDenied(
                "Memory fixture denied profile access".into(),
            ));
        }
        if workspace(&state.content, state.options_document.as_deref()).source != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The reviewed profile source changed",
                false,
            )));
        }
        if update.options != ProfileOptions::default() {
            return Err(unsupported());
        }
        state.content = update.content.clone();
        state.writes += 1;
        Ok(workspace(&state.content, state.options_document.as_deref()))
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Err(unsupported())
    }
    async fn restore_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(unsupported())
    }
}
struct MemorySnapshot;
#[async_trait]
impl SnapshotStore for MemorySnapshot {
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
