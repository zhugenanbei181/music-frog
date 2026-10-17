//! Isolated complete profile/options transactions for native tests and captures.
use async_trait::async_trait;
use futures_util::lock;
use infiltrator_domain::profile_source::hash_bytes;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId, ProfileWorkspaceIdentity,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const FILTER_PROFILE: &str = "filter-source";
pub const FILTER_DOCUMENT: &str = "mixed-port: 7890\nmode: rule\nproxies:\n  - {name: HK-public, type: ss, server: 1.1.1.1, port: 443, cipher: aes-128-gcm, password: fixture}\n  - {name: HK-private, type: ss, server: 192.168.1.1, port: 443, cipher: aes-128-gcm, password: fixture}\n  - {name: US-public, type: ss, server: 8.8.8.8, port: 80, cipher: aes-128-gcm, password: fixture}\n";
pub const FILTER_POLICY: &str = "{drop-private-ip: true, remove-emojis: true, sort-by: name-desc}";

pub struct FilterCaptureStore {
    workspace: Mutex<ProfileWorkspace>,
    active_reference: Mutex<ActiveProfileReferenceState>,
    workspace_write_gate: lock::Mutex<()>,
    pub deny_write: AtomicBool,
    pub deny_read: AtomicBool,
    pub writes: AtomicUsize,
}

struct FilterWorkspaceWriteLease<'a> {
    store: &'a FilterCaptureStore,
    expected_reference: ActiveProfileReference,
    observed: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}

struct FilterWorkspacePublishedLease<'a> {
    store: &'a FilterCaptureStore,
    expected_reference: ActiveProfileReference,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}

struct FilterWorkspaceRestoredLease<'a> {
    workspace: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}
fn workspace(
    content: String,
    sidecar_bytes: Option<Vec<u8>>,
) -> Result<ProfileWorkspace, PortError> {
    let mut result = workspace_from_raw_bytes(content.into_bytes(), sidecar_bytes)?;
    result.write_protection = ProfileWriteProtection::Editable;
    Ok(result)
}

fn workspace_from_raw_bytes(
    document_bytes: Vec<u8>,
    sidecar_bytes: Option<Vec<u8>>,
) -> Result<ProfileWorkspace, PortError> {
    let source_id =
        ProfileSourceId::new(format!("local-{}", hash_bytes(FILTER_PROFILE.as_bytes())))
            .map_err(|error| PortError::Io(error.to_string()))?;
    let document_revision = ProfileDocumentRevision::new(hash_bytes(&document_bytes))
        .map_err(|error| PortError::Io(error.to_string()))?;
    let sidecar_revision = match sidecar_bytes.as_deref() {
        Some(bytes) => ProfileSidecarRevision::Present(
            ProfileDocumentRevision::new(hash_bytes(bytes))
                .map_err(|error| PortError::Io(error.to_string()))?,
        ),
        None => ProfileSidecarRevision::Absent,
    };
    let identity = ProfileDocumentIdentity::new(
        ProfileId::new(FILTER_PROFILE).map_err(|error| PortError::Io(error.to_string()))?,
        EngineId::new("mihomo").map_err(|error| PortError::Io(error.to_string()))?,
        ProfileDocumentSourceIdentity::new(ProfileDocumentSourceKind::Local, source_id),
        document_revision,
        ProfileFormatId::new("mihomo-yaml").map_err(|error| PortError::Io(error.to_string()))?,
        ProfileSchemaVersion::new(1).map_err(|error| PortError::Io(error.to_string()))?,
    );
    Ok(ProfileWorkspace {
        document: ProfileDocument::new(identity, document_bytes),
        sidecar_revision,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes,
    })
}
impl Default for FilterCaptureStore {
    fn default() -> Self {
        Self {
            workspace: Mutex::new(
                workspace(
                    FILTER_DOCUMENT.into(),
                    Some(b"filter:\n  drop-private-ip: true\n".to_vec()),
                )
                .expect("explicit filter fixture"),
            ),
            active_reference: Mutex::new(ActiveProfileReferenceState::Selected(
                ActiveProfileReference::new(
                    EngineId::new("mihomo").expect("test engine ID"),
                    ProfileId::new(FILTER_PROFILE).expect("test profile ID"),
                    ActiveProfileReferenceRevision::default(),
                ),
            )),
            workspace_write_gate: lock::Mutex::new(()),
            deny_write: AtomicBool::new(false),
            deny_read: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
        }
    }
}
impl FilterCaptureStore {
    pub fn observed(&self) -> ProfileWorkspace {
        self.workspace
            .lock()
            .expect("isolated filter store")
            .clone()
    }
    fn check_profile(profile: &str) -> Result<(), PortError> {
        if profile == FILTER_PROFILE {
            Ok(())
        } else {
            Err(PortError::NotFound(profile.into()))
        }
    }

    fn commit_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut current = self.workspace.lock().expect("isolated filter store");
        if current.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile or options changed",
                true,
            )));
        }
        if self.deny_write.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile write access, then retry",
                true,
            )));
        }
        let mut committed =
            workspace_from_raw_bytes(update.document_bytes.clone(), update.sidecar_bytes.clone())?;
        committed.write_protection = ProfileWriteProtection::Editable;
        committed.document.identity.profile_id = expected.document.profile_id.clone();
        *current = committed.clone();
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(committed)
    }
}

#[async_trait]
impl ActiveProfileWorkspaceWriteLease for FilterWorkspaceWriteLease<'_> {
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        match self.store.read_active_profile_reference().await {
            Ok(ActiveProfileReferenceState::Selected(reference))
                if reference == self.expected_reference => {}
            Ok(_) => {
                return ActiveProfileWorkspacePublishOutcome::Unpublished(PortError::Rejected(
                    Failure::new(ErrorCode::NotReady, "active reference changed", true),
                ));
            }
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        }
        let published = match self
            .store
            .commit_workspace(&self.observed.identity(), &update)
        {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(FilterWorkspacePublishedLease {
            store: self.store,
            expected_reference: self.expected_reference.clone(),
            previous: self.observed.clone(),
            published,
            _guard: self._guard,
        }))
    }
}

#[async_trait]
impl ActiveProfileWorkspacePublishedLease for FilterWorkspacePublishedLease<'_> {
    fn previous_workspace(&self) -> &ProfileWorkspace {
        &self.previous
    }

    fn published_workspace(&self) -> &ProfileWorkspace {
        &self.published
    }

    fn commit(self: Box<Self>) -> ProfileWorkspace {
        self.published.clone()
    }

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError> {
        if self.store.read_active_profile_reference().await?
            != ActiveProfileReferenceState::Selected(self.expected_reference.clone())
        {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: self.store.observed().identity(),
            });
        }
        let actual = self.store.observed();
        if actual.identity() != self.published.identity() {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: actual.identity(),
            });
        }
        let restored = self.store.commit_workspace(
            &self.published.identity(),
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: self.previous.document.raw_bytes.clone(),
                sidecar_bytes: self.previous.sidecar_bytes.clone(),
            },
        )?;
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            FilterWorkspaceRestoredLease {
                workspace: restored,
                _guard: self._guard,
            },
        )))
    }
}

#[async_trait]
impl ActiveProfileWorkspaceRestoredLease for FilterWorkspaceRestoredLease<'_> {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}
fn outside() -> PortError {
    PortError::unsupported(
        Capability::Profiles,
        "operation is outside the isolated filter transaction",
    )
}
#[async_trait]
impl ProfileStore for FilterCaptureStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-filter")
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(vec![ProfileInfo {
            name: FILTER_PROFILE.into(),
            active: true,
            ..Default::default()
        }])
    }
    async fn get_current(&self) -> Result<String, PortError> {
        Ok(FILTER_PROFILE.into())
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        Self::check_profile(profile)?;
        self.observed()
            .document
            .utf8_text()
            .map(str::to_owned)
            .map_err(|error| PortError::Io(error.to_string()))
    }
    async fn save_inactive_profile(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_profile(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        Self::check_profile(profile)?;
        Ok(ProfileMetadata::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Self::check_profile(profile.as_str())?;
        if self.deny_read.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "isolated editor read denied".into(),
            ));
        }
        Ok(self.observed())
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Ok(self.observed())
    }
    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self
            .active_reference
            .lock()
            .expect("isolated active reference")
            .clone())
    }
    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = self.workspace_write_gate.lock().await;
        if authorization != ActiveProfileReferenceWriteAuthorization::Unowned {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "isolated filter fixture has no bound Core owner",
                true,
            )));
        }
        if self.read_active_profile_reference().await?
            != ActiveProfileReferenceState::Selected(expected_reference.clone())
            || expected_workspace.document.engine_id != expected_reference.engine_id
            || expected_workspace.document.profile_id != expected_reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active reference changed",
                true,
            )));
        }
        let observed = self.load_active_workspace().await?;
        if observed.identity() != expected_workspace {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed",
                true,
            )));
        }
        Ok(Box::new(FilterWorkspaceWriteLease {
            store: self,
            expected_reference,
            observed,
            _guard: guard,
        }))
    }
    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.commit_workspace(expected, update)
    }
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        Self::check_profile(profile)
    }
    async fn restore_inactive_profile_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(outside())
    }
}
