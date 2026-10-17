//! Headless tests for the shared rule-edit command path (DUAL-11-09/10/11/12)
//! and the DUAL-07-09/14 subscription settings commands.
//!
//! A minimal in-memory `ProfileStore` proves the application applies the same
//! list arithmetic as `infiltrator_domain::rules::edit` and persists the
//! result, without a real profile directory.

use super::*;
use crate::command_application::CommandApplication;
use crate::command_application::handler::ValidatedCoreCommandScope;
use crate::configuration_application::ConfigurationApplication;
use crate::core_capability_test_support::{core_scope, with_test_capabilities};
use async_trait::async_trait;
use infiltrator_contract::provider_cache::{ProviderCachePurge, RuleProviderCacheSnapshot};
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_contract::subscription_import::SubscriptionScheduleDraft;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::rules::edit::build_custom_rule;
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;
use infiltrator_domain::rules::{game_routing_presets, load_rules_from_yaml};
use infiltrator_domain::runtime::ConnectionSnapshot;
use infiltrator_ports::connection::{
    ConnectionControlReceipt, ConnectionControlRequest, ConnectionControlTarget,
    ConnectionObservationSnapshot, ConnectionPort,
};
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceRestoredLease,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::rule_provider_cache::{
    ProviderCacheEntry, ProviderFileFact, RuleProviderCachePort,
};
use infiltrator_test_support::profile_document_codec_test_support::TestProfileDocumentCodec;
use infiltrator_test_support::profile_workspace_test_support::{
    workspace_with_origin, workspace_with_raw_sidecar,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync;
use tokio::sync::OwnedMutexGuard;

#[derive(Default)]
struct FakeStore {
    profile: Mutex<String>,
    content: Mutex<String>,
    metadata: Mutex<ProfileMetadata>,
    workspace_reads: AtomicUsize,
    workspace_writes: AtomicUsize,
    workspace_lease_gate: Arc<sync::Mutex<()>>,
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

struct FakeActiveWorkspaceRestoredLease {
    workspace: ProfileWorkspace,
    _guard: OwnedMutexGuard<()>,
}

impl FakeStore {
    fn with_profile(content: &str) -> Self {
        Self {
            profile: Mutex::new("main".to_owned()),
            content: Mutex::new(content.to_owned()),
            metadata: Mutex::new(ProfileMetadata::default()),
            ..Self::default()
        }
    }

    fn content(&self) -> String {
        self.content.lock().expect("content lock").clone()
    }

    fn metadata(&self) -> ProfileMetadata {
        self.metadata.lock().expect("metadata lock").clone()
    }

    fn publish_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.workspace_writes.fetch_add(1, Ordering::SeqCst);
        let mut content = self.content.lock().unwrap();
        let metadata = self.metadata.lock().unwrap();
        let current = workspace_with_origin(
            expected.document.profile_id.as_str(),
            content.as_bytes(),
            None,
            None,
        )?;
        if current.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "source changed",
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
                "protected edit",
                false,
            )));
        }
        assert!(
            update.sidecar_bytes.is_none(),
            "this fixture has no sidecar"
        );
        *content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        let mut committed = workspace_with_raw_sidecar(
            expected.document.profile_id.as_str(),
            update.document_bytes.as_slice(),
            update.sidecar_bytes.clone(),
            None,
        )?;
        committed.write_protection = protection;
        Ok(committed)
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
        match self.store.read_active_profile_reference().await {
            Ok(ActiveProfileReferenceState::Selected(reference))
                if reference == self.expected_reference => {}
            Ok(_) => {
                return ActiveProfileWorkspacePublishOutcome::Unpublished(PortError::Rejected(
                    Failure::new(
                        ErrorCode::NotReady,
                        "active reference changed before fake workspace publication",
                        true,
                    ),
                ));
            }
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        }
        let expected = self.observed.identity();
        let published = match self.store.publish_workspace_locked(&expected, &update) {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        let profile = expected.document.profile_id.clone();
        let published_state = match ProfileStore::load_workspace(self.store, &profile).await {
            Ok(workspace) => workspace,
            Err(error) => return ActiveProfileWorkspacePublishOutcome::Unpublished(error),
        };
        ActiveProfileWorkspacePublishOutcome::Published(Box::new(
            FakeActiveWorkspacePublishedLease {
                store: self.store,
                previous: self.observed.clone(),
                published,
                published_state,
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
        let profile = self.published_state.document.identity.profile_id.clone();
        let current = ProfileStore::load_workspace(self.store, &profile).await?;
        if current.identity() != self.published_state.identity() {
            return Ok(ActiveProfileWorkspaceRestoreOutcome::Superseded {
                observed: current.identity(),
            });
        }
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: self.previous.document.raw_bytes.clone(),
            sidecar_bytes: self.previous.sidecar_bytes.clone(),
        };
        let workspace = self
            .store
            .publish_workspace_locked(&self.published_state.identity(), &update)?;
        Ok(ActiveProfileWorkspaceRestoreOutcome::Restored(Box::new(
            FakeActiveWorkspaceRestoredLease {
                workspace,
                _guard: self._guard,
            },
        )))
    }
}

#[async_trait]
impl ActiveProfileWorkspaceRestoredLease for FakeActiveWorkspaceRestoredLease {
    fn restored_workspace(&self) -> &ProfileWorkspace {
        &self.workspace
    }

    fn finish(self: Box<Self>) -> ProfileWorkspace {
        self.workspace.clone()
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let name = self.profile.lock().expect("profile lock").clone();
        Ok(vec![ProfileInfo {
            name,
            active: true,
            path: "/fake/configs/main.yaml".to_owned(),
            ..ProfileInfo::default()
        }])
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.profile.lock().expect("profile lock").clone())
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let profile = ProfileId::new(self.profile.lock().expect("profile lock").clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        Ok(ActiveProfileReferenceState::Selected(
            ActiveProfileReference::new(
                self.engine_id(),
                profile,
                ActiveProfileReferenceRevision::default(),
            ),
        ))
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = Arc::clone(&self.workspace_lease_gate).lock_owned().await;
        if authorization != ActiveProfileReferenceWriteAuthorization::Unowned {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "command fixture has no bound Core owner",
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
                "active profile reference changed",
                true,
            )));
        }
        let observed = ProfileStore::load_workspace(self, &expected_reference.profile_id).await?;
        if observed.identity() != expected_workspace {
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

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(self.content())
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        let content = self.content();
        let mut workspace =
            workspace_with_origin(profile.as_str(), content.as_bytes(), None, None)?;
        workspace.write_protection = ProfileWriteProtection::Editable;
        Ok(workspace)
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = self.profile.lock().expect("profile lock").clone();
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

    async fn save_inactive_profile(&self, _profile: &str, content: &str) -> Result<(), PortError> {
        *self.content.lock().expect("content lock") = content.to_owned();
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(self.metadata())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        *self.metadata.lock().expect("metadata lock") = metadata.clone();
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

const THREE_RULES: &str =
    "rules:\n  - DOMAIN,a.com,DIRECT\n  - DOMAIN,b.com,PROXY\n  - MATCH,DIRECT\n";

/// Connection fixture records the operations under test.
#[derive(Default)]
pub(super) struct RecordingConnectionPort {
    close_all_calls: Arc<AtomicUsize>,
    deny_close_all: Arc<AtomicBool>,
}

#[async_trait]
impl ConnectionPort for RecordingConnectionPort {
    async fn observe(&self, scope: CoreScope) -> Result<ConnectionObservationSnapshot, PortError> {
        ConnectionObservationSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            ConnectionSnapshot::default(),
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn close(
        &self,
        request: ConnectionControlRequest,
    ) -> Result<ConnectionControlReceipt, PortError> {
        if !matches!(request.target(), ConnectionControlTarget::All) {
            return Err(PortError::Failed(
                "close-all application sent a single-connection target".to_owned(),
            ));
        }
        self.close_all_calls.fetch_add(1, Ordering::SeqCst);
        if self.deny_close_all.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "grant connection control permission".into(),
            ));
        }
        ConnectionControlReceipt::new(
            request.scope().clone(),
            request.expected_revision(),
            Vec::new(),
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

fn application(store: &Arc<FakeStore>) -> CommandApplication {
    let store = Arc::clone(store) as Arc<dyn ProfileStore>;
    let profiles = ProfileApplication::new(Arc::clone(&store))
        .with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()));
    let configuration = ConfigurationApplication::from_profile_application(profiles.clone());
    with_test_capabilities(
        CommandApplication::new()
            .with_profile(profiles)
            .with_configuration(configuration),
    )
}

#[path = "command_application_tests/rule_edit_cases.rs"]
mod rule_edit_cases;

#[derive(Default)]
struct RecordingProviderCache {
    purges: Mutex<usize>,
}

impl RecordingProviderCache {
    fn purges(&self) -> usize {
        *self.purges.lock().expect("purge lock")
    }
}

#[async_trait]
impl RuleProviderCachePort for RecordingProviderCache {
    async fn read_provider(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError> {
        Ok(None)
    }

    async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
        *self.purges.lock().expect("purge lock") += 1;
        Ok(ProviderCachePurge {
            directory: Some("/fake/configs/rules".to_owned()),
            files_removed: 2,
            bytes_freed: 64,
        })
    }

    async fn fingerprint(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderFileFact>, PortError> {
        Ok(None)
    }

    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
        Ok(RuleProviderCacheSnapshot::ready(
            "/fake/configs/rules",
            0,
            0,
        ))
    }
}

#[path = "command_probe_tests.rs"]
mod probe_tests;

#[path = "command_connections_tests.rs"]
mod connections_tests;

#[path = "command_profile_document_tests.rs"]
mod profile_documents;
