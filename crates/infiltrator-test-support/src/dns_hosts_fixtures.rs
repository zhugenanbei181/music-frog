//! Explicit isolated profile ports for Hosts behavior and capture; no filesystem or network fallback.
use crate::profile_workspace_test_support::workspace_with_raw_sidecar;
use async_trait::async_trait;
use futures_util::lock;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceRevision,
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use std::collections::BTreeMap;
use std::mem::replace;
use std::path::PathBuf;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
pub const HOSTS_PROFILE: &str = "hosts-work";
pub const ORIGINAL_HOSTS: &str = "mixed-port: 7890\ndns:\n  enable: false\n  future-setting: preserve-me\n  hosts:\n    legacy.test: 1.1.1.1\nhosts:\n  live.test: 9.9.9.9\n  second.test: 8.8.8.8\n";
pub struct HostsCaptureStore {
    active_reference: Mutex<ActiveProfileReferenceState>,
    files: Mutex<BTreeMap<String, String>>,
    workspace_write_gate: lock::Mutex<()>,
    pub deny_save: AtomicBool,
    pub deny_read: AtomicBool,
    pub writes: AtomicUsize,
}

struct HostsWorkspaceWriteLease<'a> {
    store: &'a HostsCaptureStore,
    expected_reference: ActiveProfileReference,
    observed: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}

struct HostsWorkspacePublishedLease<'a> {
    store: &'a HostsCaptureStore,
    expected_reference: ActiveProfileReference,
    published: ProfileWorkspace,
    previous: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}

struct HostsWorkspaceRestoredLease<'a> {
    workspace: ProfileWorkspace,
    _guard: lock::MutexGuard<'a, ()>,
}
impl Default for HostsCaptureStore {
    fn default() -> Self {
        Self {
            active_reference: Mutex::new(ActiveProfileReferenceState::Selected(
                ActiveProfileReference::new(
                    EngineId::new("mihomo").expect("test engine ID"),
                    ProfileId::new(HOSTS_PROFILE).expect("test profile ID"),
                    ActiveProfileReferenceRevision::default(),
                ),
            )),
            files: Mutex::new(BTreeMap::from([
                (HOSTS_PROFILE.into(), ORIGINAL_HOSTS.into()),
                ("other".into(), "hosts:\n  other.test: 4.4.4.4\n".into()),
            ])),
            workspace_write_gate: lock::Mutex::new(()),
            deny_save: AtomicBool::new(false),
            deny_read: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ActiveProfileWorkspaceWriteLease for HostsWorkspaceWriteLease<'_> {
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
            .commit_workspace(&self.observed.identity(), &update, true)
        {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(HostsWorkspacePublishedLease {
            store: self.store,
            expected_reference: self.expected_reference.clone(),
            previous: self.observed.clone(),
            published,
            _guard: self._guard,
        }))
    }
}

#[async_trait]
impl ActiveProfileWorkspacePublishedLease for HostsWorkspacePublishedLease<'_> {
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
            let active = self.store.load_active_workspace().await?;
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: active.identity(),
            });
        }
        let actual = self.store.load_active_workspace().await?;
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
            true,
        )?;
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            HostsWorkspaceRestoredLease {
                workspace: restored,
                _guard: self._guard,
            },
        )))
    }
}

#[async_trait]
impl ActiveProfileWorkspaceRestoredLease for HostsWorkspaceRestoredLease<'_> {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}
impl HostsCaptureStore {
    pub fn content(&self) -> String {
        self.files.lock().expect("isolated file state")[HOSTS_PROFILE].clone()
    }
    pub fn replace(&self, content: &str) {
        self.files
            .lock()
            .expect("isolated file state")
            .insert(HOSTS_PROFILE.into(), content.into());
    }
}
#[async_trait]
impl ProfileStore for HostsCaptureStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-hosts")
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let current = match &*self
            .active_reference
            .lock()
            .expect("isolated active reference")
        {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        };
        Ok(self
            .files
            .lock()
            .expect("isolated files")
            .keys()
            .map(|name| ProfileInfo {
                name: name.clone(),
                active: current.as_deref() == Some(name.as_str()),
                ..Default::default()
            })
            .collect())
    }
    async fn get_current(&self) -> Result<String, PortError> {
        match &*self
            .active_reference
            .lock()
            .expect("isolated active reference")
        {
            ActiveProfileReferenceState::Selected(reference) => {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                Capability::Profiles,
                "test store has no selected active profile",
            )),
        }
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
    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        _: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let _workspace_guard = self.workspace_write_gate.lock().await;
        let mut active_reference = self
            .active_reference
            .lock()
            .expect("isolated active reference");
        if *active_reference != expected {
            return Ok(ActiveProfileReferenceCasOutcome::Stale {
                expected,
                observed: active_reference.clone(),
            });
        }
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Io("test active-profile revision overflow".into()))?;
        let current = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => {
                if engine_id != self.engine_id() {
                    return Err(PortError::unsupported(
                        Capability::Profiles,
                        "test store cannot select a foreign engine",
                    ));
                }
                if !self
                    .files
                    .lock()
                    .expect("isolated files")
                    .contains_key(profile_id.as_str())
                {
                    return Err(PortError::NotFound(profile_id.as_str().to_owned()));
                }
                ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    engine_id, profile_id, revision,
                ))
            }
        };
        let previous = replace(&mut *active_reference, current.clone());
        Ok(ActiveProfileReferenceCasOutcome::Committed { previous, current })
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        if self.deny_read.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile access, then retry the Hosts operation.",
                true,
            )));
        }
        self.files
            .lock()
            .expect("isolated files")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.into()))
    }
    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        if self.deny_read.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile access, then retry the Hosts operation.",
                true,
            )));
        }
        let content = self
            .files
            .lock()
            .expect("isolated files")
            .get(profile.as_str())
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.as_str().into()))?;
        workspace_with_raw_sidecar(profile.as_str(), content.as_bytes(), None, None)
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = match &*self
            .active_reference
            .lock()
            .expect("isolated active reference")
        {
            ActiveProfileReferenceState::Selected(reference) => {
                reference.profile_id.as_str().to_owned()
            }
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "test store has no selected active profile",
                ));
            }
        };
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.load_workspace(&profile).await
    }
    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.commit_workspace(expected, update, false)
    }
    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = self.workspace_write_gate.lock().await;
        match authorization {
            ActiveProfileReferenceWriteAuthorization::Unowned => {}
            ActiveProfileReferenceWriteAuthorization::BoundOwner { identity, .. }
                if identity.engine == expected_reference.engine_id => {}
            ActiveProfileReferenceWriteAuthorization::BoundOwner { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "isolated Hosts fixture owner belongs to another engine",
                    true,
                )));
            }
        }
        if self.read_active_profile_reference().await?
            != ActiveProfileReferenceState::Selected(expected_reference.clone())
            || expected.document.engine_id != expected_reference.engine_id
            || expected.document.profile_id != expected_reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active reference changed",
                true,
            )));
        }
        let observed = self.load_active_workspace().await?;
        if observed.identity() != expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed",
                true,
            )));
        }
        Ok(Box::new(HostsWorkspaceWriteLease {
            store: self,
            expected_reference,
            observed,
            _guard: guard,
        }))
    }
    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        if self.deny_save.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile writes, then retry the Hosts operation.",
                true,
            )));
        }
        self.files
            .lock()
            .expect("isolated files")
            .insert(profile.into(), content.into());
        Ok(())
    }
    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.files.lock().expect("isolated files").remove(profile);
        Ok(())
    }
    async fn get_profile_metadata(&self, _: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "metadata write is outside this isolated Hosts port".into(),
        ))
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "subscription credentials are outside this isolated Hosts port".into(),
        ))
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn restore_inactive_profile_backup(&self, _: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

impl HostsCaptureStore {
    fn commit_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        require_active: bool,
    ) -> Result<ProfileWorkspace, PortError> {
        if self.deny_save.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile writes, then retry the Hosts operation.",
                true,
            )));
        }
        let profile = expected.document.profile_id.as_str();
        let active_profile = match &*self
            .active_reference
            .lock()
            .expect("isolated active reference")
        {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        };
        if require_active && active_profile.as_deref() != Some(profile) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile changed",
                true,
            )));
        }
        let mut files = self.files.lock().expect("isolated files");
        let content = files
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        let actual = workspace_with_raw_sidecar(profile, content.as_bytes(), None, None)?;
        if actual.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed",
                true,
            )));
        }
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock
            }
        ) && actual.write_protection == ProfileWriteProtection::RemoteSubscription
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "protected direct edit",
                false,
            )));
        }
        let content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        files.insert(profile.to_owned(), content);
        self.writes.fetch_add(1, Ordering::SeqCst);
        workspace_with_raw_sidecar(
            profile,
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            None,
        )
    }
}
