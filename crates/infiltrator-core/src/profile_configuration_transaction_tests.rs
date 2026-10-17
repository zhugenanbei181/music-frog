//! test-intent: behavior

use super::{
    MockController, MockReloader, MockStore, OLD, TestLifecycle, replace_profile_file_for_test,
};
use crate::apply::ConfigReloader;
use crate::profile_configuration_transaction::MihomoProfileConfigurationTransactionAdapter;
use async_trait::async_trait;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profile_options::options_path;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRestoreOutcome, ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use lurker_core_ports::error::PortError;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, write};
use tokio::sync;

#[derive(Clone, Copy)]
enum NativeValidationResult {
    Accept,
    Reject,
    Unsupported,
}

struct FakeStoppedValidator {
    scope: ProfileConfigurationTransactionScope,
    result: NativeValidationResult,
    calls: AtomicUsize,
}

impl FakeStoppedValidator {
    fn new(scope: ProfileConfigurationTransactionScope, result: NativeValidationResult) -> Self {
        Self {
            scope,
            result,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileConfigurationCandidateValidatorPort for FakeStoppedValidator {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn validate_candidate(
        &self,
        expected: &ProfileWorkspaceIdentity,
        _document_bytes: &[u8],
    ) -> Result<(), PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.scope.supports(expected) {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "fake stopped validator received a foreign document scope",
            ));
        }
        match self.result {
            NativeValidationResult::Accept => Ok(()),
            NativeValidationResult::Reject => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "native Mihomo validation rejected the YAML-valid candidate",
                false,
            ))),
            NativeValidationResult::Unsupported => Err(PortError::unsupported(
                Capability::Profiles,
                "native stopped-state Mihomo validator is unavailable",
            )),
        }
    }
}

#[path = "profile_configuration_transaction_tests/inactive_readback.rs"]
mod inactive_readback;
#[path = "profile_configuration_transaction_lifecycle_tests.rs"]
mod lifecycle_tests;
#[path = "profile_configuration_transaction_tests/port_contract.rs"]
mod port_contract;

struct TransactionFixture {
    _directory: TempDir,
    config: Arc<ConfigManager<MockStore>>,
    profile_store: Arc<FaultInjectingProfileStore>,
    controller: Arc<MockController>,
    lifecycle: Arc<TestLifecycle>,
    validator: Arc<FakeStoppedValidator>,
    reloader: Arc<MockReloader>,
    transaction: MihomoProfileConfigurationTransactionAdapter<MockStore>,
}

async fn transaction_fixture(validation: NativeValidationResult) -> TransactionFixture {
    transaction_fixture_with_store_behavior(validation, StoreBehavior::Exact).await
}

async fn transaction_fixture_with_store_behavior(
    validation: NativeValidationResult,
    store_behavior: StoreBehavior,
) -> TransactionFixture {
    let directory = tempfile::tempdir().expect("temporary Core config directory");
    let config = Arc::new(
        ConfigManager::with_home_and_store(
            directory.path().to_path_buf(),
            MockStore {
                entries: Mutex::new(HashMap::new()),
            },
        )
        .expect("Mihomo config manager"),
    );
    config
        .save_inactive_profile("main", OLD)
        .await
        .expect("seed profile");
    super::select_profile(&config, "main").await;

    let controller = Arc::new(MockController {
        running: AtomicBool::new(false),
        fail_starts_left: AtomicU64::new(0),
    });
    let lifecycle = Arc::new(TestLifecycle::new(Arc::clone(&controller), 0));
    let reloader = Arc::new(MockReloader {
        fail: false,
        calls: AtomicU64::new(0),
    });
    let codec: Arc<dyn ProfileConfigurationCodecPort> = Arc::new(MihomoProfileDocumentCodec);
    let scope = ProfileConfigurationTransactionScope::new(codec.scope());
    let validator = Arc::new(FakeStoppedValidator::new(scope, validation));
    let validator_port: Arc<dyn ProfileConfigurationCandidateValidatorPort> = validator.clone();
    let session: Arc<dyn CoreLifecyclePort> = lifecycle.clone();
    let reloader_port: Arc<dyn ConfigReloader> = reloader.clone();
    let profile_store = Arc::new(FaultInjectingProfileStore::new(
        Arc::clone(&config),
        store_behavior,
    ));
    let profile_store_port: Arc<dyn ProfileStore> = profile_store.clone();
    let transaction =
        MihomoProfileConfigurationTransactionAdapter::new_with_profile_store_for_test(
            codec,
            validator_port,
            session,
            Arc::clone(&config),
            profile_store_port,
            reloader_port,
            Arc::new(sync::Mutex::new(())),
        )
        .expect("compose matching Mihomo codec and validator scopes");

    TransactionFixture {
        _directory: directory,
        config,
        profile_store,
        controller,
        lifecycle,
        validator,
        reloader,
        transaction,
    }
}

#[derive(Clone, Copy)]
enum StoreBehavior {
    Exact,
    CorruptReadbackAndRestore,
    CorruptReadbackAndFailRestore,
}

struct FaultInjectingProfileStore {
    inner: Arc<ConfigManager<MockStore>>,
    behavior: StoreBehavior,
    writes: AtomicUsize,
    restores: AtomicUsize,
}

struct FaultInjectingWorkspaceWriteLease<'a> {
    store: &'a FaultInjectingProfileStore,
    inner: Box<dyn ActiveProfileWorkspaceWriteLease + 'a>,
}

struct FaultInjectingWorkspacePublishedLease<'a> {
    store: &'a FaultInjectingProfileStore,
    inner: Box<dyn ActiveProfileWorkspacePublishedLease + 'a>,
    reported_published: ProfileWorkspace,
}

impl FaultInjectingProfileStore {
    fn new(inner: Arc<ConfigManager<MockStore>>, behavior: StoreBehavior) -> Self {
        Self {
            inner,
            behavior,
            writes: AtomicUsize::new(0),
            restores: AtomicUsize::new(0),
        }
    }

    fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn restores(&self) -> usize {
        self.restores.load(Ordering::SeqCst)
    }

    fn unused_operation() -> PortError {
        PortError::Failed("transaction fixture does not expose this profile-store operation".into())
    }
}

#[async_trait]
impl ActiveProfileWorkspaceWriteLease for FaultInjectingWorkspaceWriteLease<'_> {
    fn observed_workspace(&self) -> &ProfileWorkspace {
        self.inner.observed_workspace()
    }

    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait> {
        let Self { store, inner } = *self;
        match inner.publish(update).await {
            ActiveProfileWorkspacePublishOutcome::Published(published) => {
                store.writes.fetch_add(1, Ordering::SeqCst);
                let mut reported_published = published.published_workspace().clone();
                match store.behavior {
                    StoreBehavior::Exact => {}
                    StoreBehavior::CorruptReadbackAndRestore
                    | StoreBehavior::CorruptReadbackAndFailRestore => {
                        reported_published.document.raw_bytes =
                            b"fake readback corruption".to_vec();
                    }
                }
                ActiveProfileWorkspacePublishOutcome::Published(Box::new(
                    FaultInjectingWorkspacePublishedLease {
                        store,
                        inner: published,
                        reported_published,
                    },
                ))
            }
            outcome => outcome,
        }
    }
}

#[async_trait]
impl ActiveProfileWorkspacePublishedLease for FaultInjectingWorkspacePublishedLease<'_> {
    fn previous_workspace(&self) -> &ProfileWorkspace {
        self.inner.previous_workspace()
    }

    fn published_workspace(&self) -> &ProfileWorkspace {
        &self.reported_published
    }

    fn commit(self: Box<Self>) -> ProfileWorkspace {
        self.reported_published.clone()
    }

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError> {
        self.store.restores.fetch_add(1, Ordering::SeqCst);
        match self.store.behavior {
            StoreBehavior::Exact | StoreBehavior::CorruptReadbackAndRestore => {
                self.inner.restore_if_current().await
            }
            StoreBehavior::CorruptReadbackAndFailRestore => {
                Err(PortError::Failed("injected exact restore failure".into()))
            }
        }
    }
}

#[async_trait]
impl ProfileStore for FaultInjectingProfileStore {
    fn engine_id(&self) -> EngineId {
        ProfileStore::engine_id(self.inner.engine_profile_store())
    }

    fn config_dir(&self) -> PathBuf {
        ProfileStore::config_dir(self.inner.engine_profile_store())
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Err(Self::unused_operation())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Err(Self::unused_operation())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Err(Self::unused_operation())
    }

    async fn save_inactive_profile(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(Self::unused_operation())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(Self::unused_operation())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Err(Self::unused_operation())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Err(Self::unused_operation())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Err(Self::unused_operation())
    }

    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Err(Self::unused_operation())
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        ProfileStore::load_active_workspace(self.inner.engine_profile_store()).await
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let inner = self
            .inner
            .engine_profile_store()
            .acquire_active_workspace_write_lease(expected_reference, expected, authorization)
            .await?;
        Ok(Box::new(FaultInjectingWorkspaceWriteLease {
            store: self,
            inner,
        }))
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Err(Self::unused_operation())
    }

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(Self::unused_operation())
    }
}

fn request(
    expected: ProfileWorkspaceIdentity,
    document_bytes: &[u8],
) -> ProfileConfigurationTransactionRequest {
    let expected_reference = ActiveProfileReference::new(
        expected.document.engine_id.clone(),
        expected.document.profile_id.clone(),
        ActiveProfileReferenceRevision::new(1),
    );
    ProfileConfigurationTransactionRequest {
        expected_reference,
        expected,
        update: ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: document_bytes.to_vec(),
            sidecar_bytes: None,
        },
        authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
        strategy: ApplyStrategy::PreferReload,
    }
}

#[tokio::test]
async fn stopped_native_invalid_candidate_is_rejected_before_profile_store_write() {
    let fixture = transaction_fixture(NativeValidationResult::Reject).await;
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7891\n"))
        .await;

    assert!(matches!(
        result,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::Configuration,
            ..
        }))
    ));
    assert_eq!(fixture.validator.calls(), 1);
    assert_eq!(fixture.profile_store.writes(), 0);
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("workspace remains unchanged"),
        before
    );
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.controller.running.load(Ordering::SeqCst));
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
}

#[tokio::test]
async fn stopped_without_native_validator_is_unsupported_before_profile_store_write() {
    let fixture = transaction_fixture(NativeValidationResult::Unsupported).await;
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7891\n"))
        .await;

    assert!(matches!(result, Err(PortError::Unsupported { .. })));
    assert_eq!(fixture.validator.calls(), 1);
    assert_eq!(fixture.profile_store.writes(), 0);
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("workspace remains unchanged"),
        before
    );
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.controller.running.load(Ordering::SeqCst));
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
}

#[tokio::test]
async fn foreign_scope_and_invalid_codec_candidate_have_zero_write_effects() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");

    let mut foreign_engine = before.identity();
    foreign_engine.document.engine_id = EngineId::new("sing-box").unwrap();
    assert!(matches!(
        fixture
            .transaction
            .transact(request(foreign_engine, b"port: 7891\n"))
            .await,
        Err(PortError::Unsupported { .. })
    ));

    let mut foreign_format = before.identity();
    foreign_format.document.format_id = ProfileFormatId::new("sing-box-json").unwrap();
    assert!(matches!(
        fixture
            .transaction
            .transact(request(foreign_format, b"port: 7891\n"))
            .await,
        Err(PortError::Unsupported { .. })
    ));

    let mut foreign_schema = before.identity();
    foreign_schema.document.schema_version = ProfileSchemaVersion::new(2).unwrap();
    assert!(matches!(
        fixture
            .transaction
            .transact(request(foreign_schema, b"port: 7891\n"))
            .await,
        Err(PortError::Unsupported { .. })
    ));

    assert!(matches!(
        fixture
            .transaction
            .transact(request(before.identity(), b"port: [unterminated\n"))
            .await,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::Configuration,
            ..
        }))
    ));

    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("workspace is unchanged after foreign and invalid proposals"),
        before
    );
    assert_eq!(fixture.validator.calls(), 0);
    assert_eq!(fixture.profile_store.writes(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
}

#[tokio::test]
async fn stale_workspace_revision_is_rejected_without_replacing_the_newer_bytes() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let stale = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    super::replace_profile_file_for_test(
        &fixture.config,
        "main",
        "# newer external edit\nport: 7900\n",
    )
    .await;
    let newer = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("newer workspace");

    let result = fixture
        .transaction
        .transact(request(stale.identity(), b"port: 7891\n"))
        .await;

    assert!(matches!(
        result,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::NotReady,
            ..
        }))
    ));
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("newer workspace remains authoritative"),
        newer
    );
    assert_eq!(fixture.validator.calls(), 0);
    assert_eq!(fixture.profile_store.writes(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
}

#[tokio::test]
async fn stopped_native_valid_candidate_can_commit_with_typed_terminal_state() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7891\n"))
        .await
        .expect("candidate passed codec and native validation");

    let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
        panic!("valid stopped candidate must commit as a typed terminal state");
    };
    assert_eq!(effect, ProfileConfigurationCommitEffect::StoredWhileStopped);
    assert_eq!(fixture.validator.calls(), 1);
    assert_eq!(fixture.profile_store.writes(), 1);
    assert_eq!(workspace.document.raw_bytes, b"port: 7891\n");
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("committed workspace readback"),
        workspace
    );
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn running_transaction_applies_through_the_live_reload_seam() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let generation = fixture.lifecycle.start().await.expect("start fixture core");
    fixture
        .lifecycle
        .wait_for_ready(generation, Duration::from_secs(1))
        .await
        .expect("fixture core becomes ready");
    fixture.lifecycle.set_status(CoreLifecycle::Running);
    let starts_before = fixture.lifecycle.start_calls();
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");

    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7891\n"))
        .await
        .expect("running Mihomo accepts a validated live reload");
    let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
        panic!("successful running transaction must commit");
    };

    assert_eq!(
        effect,
        ProfileConfigurationCommitEffect::AppliedToRunningCore
    );
    assert_eq!(workspace.document.raw_bytes, b"port: 7891\n");
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("exact live transaction readback"),
        workspace
    );
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Ready);
    assert_eq!(fixture.lifecycle.start_calls(), starts_before);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
    assert_eq!(fixture.validator.calls(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stopped_transaction_reads_back_the_exact_document_and_sidecar_bytes() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let document = b"# authored byte-for-byte\r\nport: 7891 # retain spacing\r\n";
    let sidecar = b"# opaque sidecar\r\n\xff\x00";
    let mut update = request(before.identity(), document).clone();
    update.update.sidecar_bytes = Some(sidecar.to_vec());

    let result = fixture
        .transaction
        .transact(update.clone())
        .await
        .expect("valid native candidate commits");
    let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
        panic!("valid stopped transaction must commit");
    };

    assert_eq!(effect, ProfileConfigurationCommitEffect::StoredWhileStopped);
    assert_eq!(workspace.document.raw_bytes, document);
    assert_eq!(workspace.sidecar_bytes.as_deref(), Some(sidecar.as_slice()));
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("exact committed readback"),
        workspace
    );
    assert_eq!(
        read(fixture.config.config_dir().join("main.yaml"))
            .await
            .expect("raw profile bytes"),
        document
    );
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(fixture.profile_store.writes(), 1);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn valid_failed_candidate_is_stored_without_start_stop_or_runtime_calls() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    fixture.lifecycle.set_status(CoreLifecycle::Failed);
    let before = fixture
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7892\n"))
        .await
        .expect("candidate passed codec and native validation");

    let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
        panic!("valid failed-state candidate must commit as a typed terminal state");
    };
    assert_eq!(effect, ProfileConfigurationCommitEffect::StoredWhileFailed);
    assert_eq!(workspace.document.raw_bytes, b"port: 7892\n");
    assert_eq!(
        fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("committed workspace readback"),
        workspace
    );
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Failed);
    assert_eq!(fixture.profile_store.writes(), 1);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.controller.running.load(Ordering::SeqCst));
}
