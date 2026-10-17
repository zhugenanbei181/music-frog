//! Focused unit tests for the shared profile application and its
//! conditional subscription lifecycle.

use super::*;
use async_trait::async_trait;
use chrono::Timelike;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceRevision,
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_age_credential::{
    ProfileAgeContentDecryption, ProfileAgeCredentialObservation, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialReadRequest, ProfileAgeCredentialRevision, ProfileAgeCredentialState,
    ProfileAgeDecryptedContent, ProfileAgeEncryptedContent,
};
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest,
};
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
use infiltrator_test_support::profile_document_codec_test_support::TestProfileDocumentCodec;
use infiltrator_test_support::profile_workspace_test_support::{
    workspace_with_origin, workspace_with_raw_sidecar,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{self, Barrier, OwnedMutexGuard};

#[derive(Default)]
struct FakeStore {
    active_reference: Mutex<ActiveProfileReferenceState>,
    bound_owner: Mutex<Option<(CoreIdentity, CoreGeneration)>>,
    reference_read_barrier: Mutex<Option<Arc<Barrier>>>,
    reference_change_before_compare: Mutex<Option<ActiveProfileReferenceTarget>>,
    reference_write_count: AtomicUsize,
    profile_save_count: AtomicUsize,
    profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
    catalog_order: Mutex<Option<Vec<ProfileId>>>,
    display_names: Mutex<BTreeMap<String, ProfileDisplayNameObservation>>,
    age_decryption: Mutex<FakeAgeDecryption>,
    age_decryption_calls: AtomicUsize,
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
    workspace_lease_gate: Arc<sync::Mutex<()>>,
}

#[derive(Default)]
enum FakeAgeDecryption {
    #[default]
    Unsupported,
    Decrypted {
        content: String,
        revision: u64,
    },
    CredentialNotConfigured,
    InvalidCredentialOrContent,
}

fn profile_id(name: &str) -> ProfileId {
    ProfileId::new(name).expect("test profile identity")
}

struct FakeActiveWorkspaceWriteLease<'a> {
    store: &'a FakeStore,
    expected_reference: ActiveProfileReference,
    observed: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

struct FakeActiveWorkspacePublishedLease<'a> {
    store: &'a FakeStore,
    previous: ProfileWorkspace,
    published: ProfileWorkspace,
    published_state: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

struct FakeActiveWorkspaceRestoredLease<'a> {
    workspace: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
    marker: PhantomData<&'a FakeStore>,
}

fn profiles_with_mihomo_codec(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    ProfileApplication::new(store).with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()))
}

impl FakeStore {
    fn decrypt_age_to(&self, content: &str, revision: u64) {
        *self.age_decryption.lock().expect("Age decryption") = FakeAgeDecryption::Decrypted {
            content: content.to_owned(),
            revision,
        };
    }

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

    fn set_reference_read_barrier(&self, barrier: Arc<Barrier>) {
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
                marker: PhantomData,
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

#[path = "profile_application_tests/fake_store_profile_store.rs"]
mod fake_store_profile_store;

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
        match self.result.lock().expect("result lock").clone() {
            Some(ConditionalDocumentResult::Modified { document, .. }) => Ok(document),
            Some(ConditionalDocumentResult::NotModified { .. }) | None => Err(
                PortError::unsupported(Capability::Profiles, "no modified subscription content"),
            ),
        }
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
#[path = "profile_application_tests/catalog_order.rs"]
mod catalog_order;
#[path = "profile_application_tests/clear_active_selection.rs"]
mod clear_active_selection;
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

#[path = "profile_application_tests/age_credential.rs"]
mod age_credential;

#[path = "profile_application_tests/configuration_codec.rs"]
mod configuration_codec;
