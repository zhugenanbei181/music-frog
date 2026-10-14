//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use crate::profile_workspace_test_support::{workspace_with_origin, workspace_with_raw_sidecar};
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceRevision,
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::profile_display_name::{
    ProfileDisplayNameMutation, ProfileDisplayNameObservation, ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease,
};
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::subscription_source::SubscriptionDocument;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeStore {
    active_reference: Mutex<ActiveProfileReferenceState>,
    bound_owner: Mutex<Option<(CoreIdentity, CoreGeneration)>>,
    reference_read_barrier: Mutex<Option<Arc<tokio::sync::Barrier>>>,
    reference_change_before_compare: Mutex<Option<ActiveProfileReferenceTarget>>,
    reference_write_count: AtomicUsize,
    profile_save_count: AtomicUsize,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    display_names: Mutex<BTreeMap<String, ProfileDisplayNameObservation>>,
    sidecars: Mutex<BTreeMap<String, Vec<u8>>>,
    options_read_error: Mutex<Option<PortError>>,
    workspace_write_error: Mutex<Option<PortError>>,
    list_read_error: Mutex<Option<PortError>>,
    metadata_read_error: Mutex<Option<PortError>>,
    deleted_options: Mutex<Vec<String>>,
    cleared_backups: Mutex<Vec<String>>,
    restorable_backups: Mutex<Vec<String>>,
    restored_backups: Mutex<Vec<String>>,
    workspace_reads: AtomicUsize,
    workspace_writes: AtomicUsize,
    foreign_workspace_receipt: AtomicBool,
    stale_document_revision_receipt: AtomicBool,
    stale_sidecar_revision_receipt: AtomicBool,
    stale_after_next_workspace_read: AtomicBool,
    workspace_lease_gate: Arc<tokio::sync::Mutex<()>>,
}

fn profile_id(name: &str) -> ProfileId {
    ProfileId::new(name).expect("test profile identity")
}

struct FakeActiveWorkspaceWriteLease<'a> {
    store: &'a FakeStore,
    expected_reference: ActiveProfileReference,
    observed: ProfileWorkspace,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

struct FakeActiveWorkspacePublishedLease<'a> {
    store: &'a FakeStore,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    published_state: ProfileWorkspace,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

struct FakeActiveWorkspaceRestoredLease<'a> {
    workspace: ProfileWorkspace,
    _guard: tokio::sync::OwnedMutexGuard<()>,
    marker: std::marker::PhantomData<&'a FakeStore>,
}

fn profiles_with_mihomo_codec(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    ProfileApplication::new(store).with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()))
}

impl FakeStore {
    fn with_profile(name: &str, content: &str, active: bool) -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            name.to_string(),
            (content.to_string(), ProfileMetadata::default()),
        );
        Self {
            active_reference: Mutex::new(if active {
                ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    EngineId::new("mihomo").expect("test engine ID"),
                    ProfileId::new(name).expect("test profile ID"),
                    ActiveProfileReferenceRevision::new(1),
                ))
            } else {
                ActiveProfileReferenceState::Unselected {
                    revision: ActiveProfileReferenceRevision::new(0),
                }
            }),
            profiles: Mutex::new(profiles),
            ..Self::default()
        }
    }

    fn set_reference_read_barrier(&self, barrier: Arc<tokio::sync::Barrier>) {
        *self
            .reference_read_barrier
            .lock()
            .expect("reference read barrier") = Some(barrier);
    }

    fn change_reference_before_next_compare(&self, target: ActiveProfileReferenceTarget) {
        *self
            .reference_change_before_compare
            .lock()
            .expect("reference compare hook") = Some(target);
    }

    fn active_reference(&self) -> ActiveProfileReferenceState {
        self.active_reference
            .lock()
            .expect("active reference")
            .clone()
    }

    fn bind_owner_for_test(&self, identity: CoreIdentity, generation: CoreGeneration) {
        *self.bound_owner.lock().expect("test active owner") = Some((identity, generation));
    }

    fn replace_document_for_test(&self, profile: &str, content: &str) {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get_mut(profile)
            .expect("test profile exists")
            .0 = content.to_owned();
    }

    fn set_external_current(&self, profile: &str) -> Result<(), PortError> {
        self.update_reference_target(ActiveProfileReferenceTarget::selected(
            self.engine_id(),
            ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?,
        ))?;
        Ok(())
    }

    fn update_reference_target(
        &self,
        target: ActiveProfileReferenceTarget,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        if let ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } = &target
        {
            if engine_id != &self.engine_id() {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "fake store cannot select a foreign engine profile",
                ));
            }
            if !self
                .profiles
                .lock()
                .expect("profiles")
                .contains_key(profile_id.as_str())
            {
                return Err(PortError::NotFound(profile_id.as_str().to_owned()));
            }
        }
        let mut state = self.active_reference.lock().expect("active reference");
        let revision = state
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active reference revision exhausted".to_owned()))?;
        let next = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        *state = next.clone();
        self.reference_write_count.fetch_add(1, Ordering::SeqCst);
        Ok(next)
    }

    fn set_options(&self, profile: &str, options: &ProfileOptions) {
        let bytes = serde_yaml_ng::to_string(options)
            .expect("serialize Mihomo options fixture")
            .into_bytes();
        self.sidecars
            .lock()
            .expect("sidecars lock")
            .insert(profile.to_owned(), bytes);
    }

    fn publish_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let profile = expected.document.profile_id.as_str();
        let mut profiles = self.profiles.lock().unwrap();
        let mut sidecars = self.sidecars.lock().unwrap();
        let (content, metadata) = profiles
            .get(profile)
            .ok_or_else(|| PortError::NotFound(profile.into()))?;
        if workspace_with_raw_sidecar(
            profile,
            content.as_bytes(),
            sidecars.get(profile).cloned(),
            metadata.subscription_url.as_deref(),
        )?
        .identity()
            != *expected
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "changed source",
                true,
            )));
        }
        let protection = ProfileWriteProtection::from_subscription_url(
            metadata.subscription_url.as_deref().unwrap_or_default(),
        );
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock
            }
        ) && protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "protected direct edit",
                false,
            )));
        }
        if let Some(error) = self.workspace_write_error.lock().unwrap().clone() {
            return Err(error);
        }
        let content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        let mut result = workspace_with_raw_sidecar(
            profile,
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            metadata.subscription_url.as_deref(),
        )?;
        result.write_protection = protection;
        profiles.get_mut(profile).unwrap().0.clone_from(&content);
        match update.sidecar_bytes.as_ref() {
            Some(sidecar) => {
                sidecars.insert(profile.to_owned(), sidecar.clone());
            }
            None => {
                sidecars.remove(profile);
            }
        }
        self.workspace_writes.fetch_add(1, Ordering::SeqCst);
        if self.foreign_workspace_receipt.load(Ordering::SeqCst) {
            result.document.identity.engine_id = EngineId::new("sing-box").expect("test engine ID");
        }
        if self.stale_document_revision_receipt.load(Ordering::SeqCst) {
            result.document.identity.revision = expected.document.revision.clone();
        }
        if self.stale_sidecar_revision_receipt.load(Ordering::SeqCst) {
            result.sidecar_revision = expected.sidecar_revision.clone();
        }
        Ok(result)
    }
}

#[async_trait]
impl ActiveProfileWorkspaceWriteLease for FakeActiveWorkspaceWriteLease<'_> {
    fn observed_workspace(&self) -> &ProfileWorkspace {
        &self.observed
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        if self.store.active_reference()
            != ActiveProfileReferenceState::Selected(self.expected_reference.clone())
        {
            return ActiveProfileWorkspacePublishOutcome::Unpublished(PortError::Rejected(
                Failure::new(
                    ErrorCode::NotReady,
                    "active reference changed before fake workspace publication",
                    true,
                ),
            ));
        }
        let expected = self.observed.identity();
        let reported = match self.store.publish_workspace_locked(&expected, &update) {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        let profile = expected.document.profile_id.clone();
        let actual = match ProfileStore::load_workspace(self.store, &profile).await {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(
            FakeActiveWorkspacePublishedLease {
                store: self.store,
                previous: self.observed.clone(),
                published: reported,
                published_state: actual,
                _guard: self._guard,
            },
        ))
    }
}

#[async_trait]
impl ActiveProfileWorkspacePublishedLease for FakeActiveWorkspacePublishedLease<'_> {
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
        let actual = ProfileStore::load_workspace(
            self.store,
            &self.published_state.document.identity.profile_id,
        )
        .await?;
        if actual.identity() != self.published_state.identity() {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: actual.identity(),
            });
        }
        if let Some(error) = self.store.workspace_write_error.lock().unwrap().clone() {
            return Err(error);
        }
        let profile = self.previous.document.identity.profile_id.as_str();
        let text = String::from_utf8(self.previous.document.raw_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        {
            let mut profiles = self.store.profiles.lock().unwrap();
            let (content, _) = profiles
                .get_mut(profile)
                .ok_or_else(|| PortError::NotFound(profile.to_owned()))?;
            *content = text;
        }
        {
            let mut sidecars = self.store.sidecars.lock().unwrap();
            match self.previous.sidecar_bytes.as_ref() {
                Some(bytes) => {
                    sidecars.insert(profile.to_owned(), bytes.clone());
                }
                None => {
                    sidecars.remove(profile);
                }
            }
        }
        self.store.workspace_writes.fetch_add(1, Ordering::SeqCst);
        let restored =
            ProfileStore::load_workspace(self.store, &self.previous.document.identity.profile_id)
                .await?;
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            FakeActiveWorkspaceRestoredLease {
                workspace: restored,
                _guard: self._guard,
                marker: std::marker::PhantomData,
            },
        )))
    }
}

#[async_trait]
impl ActiveProfileWorkspaceRestoredLease for FakeActiveWorkspaceRestoredLease<'_> {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}

async fn load_workspace(store: &FakeStore, profile: &str) -> ProfileWorkspace {
    let profile = ProfileId::new(profile).expect("valid profile ID");
    ProfileStore::load_workspace(store, &profile)
        .await
        .expect("profile workspace")
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        let profile = profile.as_str();
        if let Some(error) = self.options_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let result = {
            let profiles = self.profiles.lock().unwrap();
            let sidecars = self.sidecars.lock().unwrap();
            let (content, metadata) = profiles
                .get(profile)
                .ok_or_else(|| PortError::NotFound(profile.into()))?;
            let mut result = workspace_with_raw_sidecar(
                profile,
                content.as_bytes(),
                sidecars.get(profile).cloned(),
                metadata.subscription_url.as_deref(),
            )?;
            result.write_protection = ProfileWriteProtection::from_subscription_url(
                metadata.subscription_url.as_deref().unwrap_or_default(),
            );
            result
        };
        if self
            .stale_after_next_workspace_read
            .swap(false, Ordering::SeqCst)
        {
            let mut profiles = self.profiles.lock().unwrap();
            let (content, _) = profiles
                .get_mut(profile)
                .ok_or_else(|| PortError::NotFound(profile.into()))?;
            content.push_str("# concurrent edit\n");
        }
        Ok(result)
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                reference.profile_id.as_str().to_owned()
            }
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "fake store has no selected profile",
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
        self.publish_workspace_locked(expected, update)
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = Arc::clone(&self.workspace_lease_gate).lock_owned().await;
        let bound_owner = self.bound_owner.lock().expect("test active owner").clone();
        let authorized = match (&authorization, &bound_owner) {
            (ActiveProfileReferenceWriteAuthorization::Unowned, None) => true,
            (
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity,
                    generation,
                },
                Some((bound_identity, bound_generation)),
            ) => identity == bound_identity && generation == bound_generation,
            _ => false,
        };
        if !authorized {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace owner authorization is stale",
                true,
            )));
        }
        let reference_matches = {
            let reference_state = self.active_reference.lock().expect("active reference");
            *reference_state == ActiveProfileReferenceState::Selected(expected_reference.clone())
        };
        if !reference_matches
            || expected.document.engine_id != expected_reference.engine_id
            || expected.document.profile_id != expected_reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference changed",
                true,
            )));
        }
        let observed = ProfileStore::load_workspace(self, &expected.document.profile_id).await?;
        if observed.identity() != expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace identity is stale",
                true,
            )));
        }
        Ok(Box::new(FakeActiveWorkspaceWriteLease {
            store: self,
            expected_reference,
            observed,
            _guard: guard,
        }))
    }
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        if let Some(error) = self.list_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let current = match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        };
        Ok(self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(|(name, (_content, metadata))| ProfileInfo {
                name: name.clone(),
                active: current.as_deref() == Some(name.as_str()),
                path: format!("/fake/configs/{name}.yaml"),
                subscription_url: metadata.subscription_url.clone(),
                auto_update_enabled: metadata.auto_update_enabled,
                update_interval_hours: metadata.update_interval_hours,
                last_updated: metadata.last_updated,
                next_update: metadata.next_update,
                traffic_upload: metadata.traffic_upload,
                traffic_download: metadata.traffic_download,
                traffic_total: metadata.traffic_total,
                expire_at: metadata.expire_at,
                controller_url: None,
                controller_changed: None,
                user_agent: metadata.user_agent.clone(),
                etag: metadata.etag.clone(),
                last_modified: metadata.last_modified.clone(),
                cron_expression: metadata.cron_expression.clone(),
                insecure_skip_verify: metadata.insecure_skip_verify,
                auto_reload_core: metadata.auto_reload_core,
                has_backup: self
                    .restorable_backups
                    .lock()
                    .expect("backup lock")
                    .contains(name),
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                Capability::Profiles,
                "fake store has no selected profile",
            )),
        }
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let observed = self.active_reference();
        let barrier = {
            let guard = self
                .reference_read_barrier
                .lock()
                .expect("reference read barrier");
            guard.clone()
        };
        if let Some(barrier) = barrier {
            barrier.wait().await;
        }
        Ok(observed)
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let bound_owner = self.bound_owner.lock().expect("test active owner").clone();
        match (&authorization, &bound_owner) {
            (ActiveProfileReferenceWriteAuthorization::Unowned, None) => {}
            (
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity,
                    generation,
                },
                Some((bound_identity, bound_generation)),
            ) if identity == bound_identity && generation == bound_generation => {}
            (ActiveProfileReferenceWriteAuthorization::Unowned, Some(_)) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "offline selection is blocked while a Core owner is bound",
                    true,
                )));
            }
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "active Core owner authorization does not match the bound identity and generation",
                    true,
                )));
            }
        }
        let concurrent_target = self
            .reference_change_before_compare
            .lock()
            .expect("reference compare hook")
            .take();
        if let Some(concurrent_target) = concurrent_target {
            self.update_reference_target(concurrent_target)?;
        }
        let mut state = self.active_reference.lock().expect("active reference");
        let observed = state.clone();
        if observed.revision() != expected.revision() {
            return Ok(ActiveProfileReferenceCasOutcome::Stale { expected, observed });
        }
        if observed != expected {
            return Err(PortError::Failed(
                "active reference state changed without advancing its revision".to_owned(),
            ));
        }
        if let ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } = &target
        {
            if engine_id != &self.engine_id() {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "fake store cannot select a foreign engine profile",
                ));
            }
            if !self
                .profiles
                .lock()
                .expect("profiles")
                .contains_key(profile_id.as_str())
            {
                return Err(PortError::NotFound(profile_id.as_str().to_owned()));
            }
        }
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active reference revision exhausted".to_owned()))?;
        let current = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        *state = current.clone();
        self.reference_write_count.fetch_add(1, Ordering::SeqCst);
        Ok(ActiveProfileReferenceCasOutcome::Committed {
            previous: expected,
            current,
        })
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        if matches!(
            self.active_reference(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id.as_str() == profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake inactive save cannot write the selected profile",
                true,
            )));
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .entry(profile.to_string())
            .or_insert_with(|| (String::new(), ProfileMetadata::default()))
            .0 = content.to_string();
        self.profile_save_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .remove(profile)
            .map(|_| ())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))?;
        self.deleted_options
            .lock()
            .expect("options lock")
            .push(profile.to_owned());
        self.sidecars.lock().expect("sidecars lock").remove(profile);
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        if let Some(error) = self.metadata_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(_, metadata)| metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get_mut(profile)
            .map(|(_, current)| *current = metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn profile_display_name(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileDisplayNameObservation, PortError> {
        display_name::observe_fake_store(self, profile)
    }

    async fn set_profile_display_name(
        &self,
        profile: &ProfileId,
        target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, PortError> {
        display_name::set_fake_store(self, profile, target)
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        self.cleared_backups
            .lock()
            .expect("backup lock")
            .push(profile.to_string());
        Ok(())
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        let mut restorable = self.restorable_backups.lock().expect("backup lock");
        if let Some(index) = restorable.iter().position(|name| name == profile) {
            restorable.remove(index);
            self.restored_backups
                .lock()
                .expect("backup lock")
                .push(profile.to_string());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

struct FakeSource {
    result: Mutex<Option<ConditionalDocumentResult>>,
    calls: Mutex<Vec<ConditionalFetchHeaders>>,
}

impl FakeSource {
    fn modified(content: &str, etag: Option<&str>) -> Self {
        Self {
            result: Mutex::new(Some(ConditionalDocumentResult::Modified {
                document: SubscriptionDocument {
                    content: content.to_string(),
                    userinfo: Some(SubscriptionUserInfo {
                        upload: Some(90),
                        download: Some(910),
                        total: Some(1000),
                        expire: Some(2_000_000_000),
                    }),
                },
                etag: etag.map(str::to_string),
                last_modified: Some("Wed, 21 Oct 2026 07:28:00 GMT".to_string()),
            })),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn not_modified(etag: Option<&str>) -> Self {
        Self {
            result: Mutex::new(Some(ConditionalDocumentResult::NotModified {
                userinfo: Some(SubscriptionUserInfo {
                    upload: Some(90),
                    download: Some(910),
                    total: Some(1000),
                    expire: Some(2_000_000_000),
                }),
                etag: etag.map(str::to_string),
                last_modified: None,
            })),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl SubscriptionSource for FakeSource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "use fetch_conditional",
        ))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
        headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        self.calls.lock().expect("calls lock").push(headers.clone());
        Ok(self
            .result
            .lock()
            .expect("result lock")
            .clone()
            .expect("result set"))
    }
}

async fn subscription_store(
    etag: Option<&str>,
    last_modified: Option<&str>,
    user_agent: Option<&str>,
    insecure_skip_verify: bool,
) -> Arc<FakeStore> {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: n1\n    type: ss\n",
        true,
    ));
    {
        let mut profiles = store.profiles.lock().expect("profiles lock");
        let metadata = &mut profiles.get_mut("main").expect("profile").1;
        metadata.subscription_url = Some("https://example.com/sub.yaml".to_string());
        metadata.auto_update_enabled = true;
        metadata.update_interval_hours = Some(12);
        metadata.etag = etag.map(str::to_string);
        metadata.last_modified = last_modified.map(str::to_string);
        metadata.user_agent = user_agent.map(str::to_string);
        metadata.insecure_skip_verify = insecure_skip_verify;
    }
    store
}

// ---- DUAL-09-12: remote-subscription write protection -----------------------

/// Mark a fake profile as downloaded from a subscription URL.
fn mark_subscription(store: &FakeStore, profile: &str, url: &str) {
    let mut profiles = store.profiles.lock().expect("profiles lock");
    profiles
        .get_mut(profile)
        .expect("profile exists")
        .1
        .subscription_url = Some(url.to_string());
}

// ---- DUAL-09-14: one option-sidecar use-case for both editor panes --------

#[path = "profile_application_tests/activation.rs"]
mod activation;
#[path = "profile_application_tests/apply.rs"]
mod apply;
#[path = "profile_application_tests/auto.rs"]
mod auto;
#[path = "profile_application_tests/batch.rs"]
mod batch;
#[path = "profile_application_tests/conditional.rs"]
mod conditional;
#[path = "profile_application_tests/deletion.rs"]
mod deletion;
#[path = "profile_application_tests/display_name.rs"]
mod display_name;
#[path = "profile_application_tests/edited.rs"]
mod edited;
#[path = "profile_application_tests/fetch.rs"]
mod fetch;
#[path = "profile_application_tests/filter.rs"]
mod filter;
#[path = "profile_application_tests/import.rs"]
mod import;
#[path = "profile_application_tests/inactive.rs"]
mod inactive;
#[path = "profile_application_tests/invalid.rs"]
mod invalid;
#[path = "profile_application_tests/list.rs"]
mod list;
#[path = "profile_application_tests/local.rs"]
mod local;
#[path = "profile_application_tests/mixin.rs"]
mod mixin;
#[path = "profile_application_tests/options.rs"]
mod options;
#[path = "profile_application_tests/restore.rs"]
mod restore;
#[path = "profile_application_tests/schedule.rs"]
mod schedule;
#[path = "profile_application_tests/selection.rs"]
mod selection;
#[path = "profile_application_tests/successful.rs"]
mod successful;
#[path = "profile_application_tests/update.rs"]
mod update;
#[path = "profile_application_tests/write.rs"]
mod write;

#[path = "profile_application_tests/editor_observations.rs"]
mod editor_observations;

#[path = "profile_application_tests/source_bound_edits.rs"]
mod source_bound_edits;

#[path = "profile_application_tests/engine_scope.rs"]
mod engine_scope;
#[path = "profile_application_tests/namespace.rs"]
mod namespace;
#[path = "profile_application_tests/read_status.rs"]
mod read_status;

#[path = "profile_application_tests/configuration_codec.rs"]
mod configuration_codec;
