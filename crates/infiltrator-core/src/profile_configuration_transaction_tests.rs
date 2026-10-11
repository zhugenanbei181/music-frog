//! test-intent: behavior

use super::{MockController, MockReloader, MockStore, OLD, TestLifecycle};
use crate::apply::ConfigReloader;
use crate::profile_configuration_transaction::MihomoProfileConfigurationTransactionAdapter;
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

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
        expected: &infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity,
        _document_bytes: &[u8],
    ) -> Result<(), PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.scope.supports(expected) {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
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
                infiltrator_contract::capability::Capability::Profiles,
                "native stopped-state Mihomo validator is unavailable",
            )),
        }
    }
}

struct TransactionFixture {
    _directory: TempDir,
    config: Arc<ConfigManager<MockStore>>,
    controller: Arc<MockController>,
    lifecycle: Arc<TestLifecycle>,
    validator: Arc<FakeStoppedValidator>,
    reloader: Arc<MockReloader>,
    transaction: MihomoProfileConfigurationTransactionAdapter<MockStore>,
}

async fn transaction_fixture(validation: NativeValidationResult) -> TransactionFixture {
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
        .ensure_default_config()
        .await
        .expect("write initial profile fixture");
    config.save("main", OLD).await.expect("seed profile");
    config.set_current("main").await.expect("select profile");

    let controller = Arc::new(MockController {
        running: std::sync::atomic::AtomicBool::new(false),
        fail_starts_left: std::sync::atomic::AtomicU64::new(0),
    });
    let lifecycle = Arc::new(TestLifecycle::new(Arc::clone(&controller), 0));
    let reloader = Arc::new(MockReloader {
        fail: false,
        calls: std::sync::atomic::AtomicU64::new(0),
    });
    let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(MihomoProfileDocumentCodec);
    let scope = ProfileConfigurationTransactionScope::new(codec.scope());
    let validator = Arc::new(FakeStoppedValidator::new(scope, validation));
    let validator_port: Arc<dyn ProfileConfigurationCandidateValidatorPort> = validator.clone();
    let session: Arc<dyn CoreLifecyclePort> = lifecycle.clone();
    let reloader_port: Arc<dyn ConfigReloader> = reloader.clone();
    let transaction = MihomoProfileConfigurationTransactionAdapter::new(
        codec,
        validator_port,
        session,
        Arc::clone(&config),
        reloader_port,
        Arc::new(tokio::sync::Mutex::new(())),
    )
    .expect("compose matching Mihomo codec and validator scopes");

    TransactionFixture {
        _directory: directory,
        config,
        controller,
        lifecycle,
        validator,
        reloader,
        transaction,
    }
}

fn request(
    expected: infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity,
    document_bytes: &[u8],
) -> ProfileConfigurationTransactionRequest {
    ProfileConfigurationTransactionRequest {
        expected,
        update: ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: document_bytes.to_vec(),
            sidecar_bytes: None,
        },
        strategy: ApplyStrategy::PreferReload,
    }
}

#[tokio::test]
async fn stopped_native_invalid_candidate_is_rejected_before_profile_store_write() {
    let fixture = transaction_fixture(NativeValidationResult::Reject).await;
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Stopped);
    let before = fixture
        .config
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
    assert_eq!(
        fixture
            .config
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
        .load_active_workspace()
        .await
        .expect("initial workspace");
    let result = fixture
        .transaction
        .transact(request(before.identity(), b"port: 7891\n"))
        .await;

    assert!(matches!(result, Err(PortError::Unsupported { .. })));
    assert_eq!(fixture.validator.calls(), 1);
    assert_eq!(
        fixture
            .config
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
async fn stopped_native_valid_candidate_can_commit_with_typed_terminal_state() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    let before = fixture
        .config
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
    assert_eq!(workspace.document.raw_bytes, b"port: 7891\n");
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(
        fixture
            .config
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
async fn valid_failed_candidate_is_stored_without_start_stop_or_runtime_calls() {
    let fixture = transaction_fixture(NativeValidationResult::Accept).await;
    fixture.lifecycle.set_status(CoreLifecycle::Failed);
    let before = fixture
        .config
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
            .load_active_workspace()
            .await
            .expect("committed workspace readback"),
        workspace
    );
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(fixture.lifecycle.lifecycle(), CoreLifecycle::Failed);
    assert_eq!(fixture.lifecycle.start_calls(), 0);
    assert_eq!(fixture.lifecycle.stop_calls(), 0);
    assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!fixture.controller.running.load(Ordering::SeqCst));
}
