//! test-intent: behavior

use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
    ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecScope;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const VALID: &[u8] = b"port: 7891\n";

#[derive(Clone, Copy)]
enum FakePlan {
    Commit,
    ApplyFailureAndRestore,
    ApplyFailureAndRollbackFailure,
    ReadbackMismatchAndRestore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FakeLifecycle {
    Running,
    Stopped,
    Failed,
}

#[derive(Clone, Copy)]
enum NativeValidationPlan {
    Accept,
    Reject,
    Unsupported,
}

struct FakeNativeValidator {
    scope: ProfileConfigurationTransactionScope,
    plan: NativeValidationPlan,
    calls: AtomicUsize,
}

impl FakeNativeValidator {
    fn new(scope: ProfileConfigurationTransactionScope, plan: NativeValidationPlan) -> Self {
        Self {
            scope,
            plan,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileConfigurationCandidateValidatorPort for FakeNativeValidator {
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
                infiltrator_contract::capability::Capability::Profiles,
                "native validator received a foreign engine, format, or schema",
            ));
        }
        match self.plan {
            NativeValidationPlan::Accept => Ok(()),
            NativeValidationPlan::Reject => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "fake native validator rejected an otherwise YAML-valid candidate",
                false,
            ))),
            NativeValidationPlan::Unsupported => Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "fake native validator is unavailable while the Core is stopped",
            )),
        }
    }
}

struct FakeTransaction {
    scope: ProfileConfigurationTransactionScope,
    current: Mutex<ProfileWorkspace>,
    writes: AtomicUsize,
    runtime_effects: AtomicUsize,
    plan: FakePlan,
    lifecycle: FakeLifecycle,
    native_validator: FakeNativeValidator,
}

impl FakeTransaction {
    fn new(plan: FakePlan) -> Self {
        let scope = scope("mihomo", "mihomo-yaml", 1);
        Self {
            current: Mutex::new(workspace(&scope, "rev-1", b"port: 7890\n")),
            native_validator: FakeNativeValidator::new(scope.clone(), NativeValidationPlan::Accept),
            scope,
            writes: AtomicUsize::new(0),
            runtime_effects: AtomicUsize::new(0),
            plan,
            lifecycle: FakeLifecycle::Running,
        }
    }

    fn stopped(plan: FakePlan, native_validation: NativeValidationPlan) -> Self {
        let mut transaction = Self::new(plan);
        transaction.lifecycle = FakeLifecycle::Stopped;
        transaction.native_validator =
            FakeNativeValidator::new(transaction.scope.clone(), native_validation);
        transaction
    }

    fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn runtime_effects(&self) -> usize {
        self.runtime_effects.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileConfigurationTransactionPort for FakeTransaction {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        if !self.scope.supports(&request.expected) {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "foreign engine, format, or schema",
            ));
        }
        let current_identity = self
            .current
            .lock()
            .expect("fake transaction state")
            .identity();
        if current_identity != request.expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "stale profile workspace revision",
                true,
            )));
        }
        match self.lifecycle {
            FakeLifecycle::Running => {
                if request.update.document_bytes != VALID {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::Configuration,
                        "candidate rejected by fake running native validator",
                        false,
                    )));
                }
            }
            FakeLifecycle::Stopped | FakeLifecycle::Failed => {
                self.native_validator
                    .validate_candidate(&request.expected, &request.update.document_bytes)
                    .await?;
            }
        }

        let mut current = self.current.lock().expect("fake transaction state");
        if current.identity() != request.expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "stale profile workspace revision after native validation",
                true,
            )));
        }
        let previous = current.clone();
        let committed = workspace_with_sidecar(
            &self.scope,
            "rev-2",
            &request.update.document_bytes,
            request.update.sidecar_bytes.as_deref(),
        );
        self.writes.fetch_add(1, Ordering::SeqCst);
        *current = committed.clone();
        let effect = match self.lifecycle {
            FakeLifecycle::Running => {
                self.runtime_effects.fetch_add(1, Ordering::SeqCst);
                ProfileConfigurationCommitEffect::AppliedToRunningCore
            }
            FakeLifecycle::Stopped => ProfileConfigurationCommitEffect::StoredWhileStopped,
            FakeLifecycle::Failed => ProfileConfigurationCommitEffect::StoredWhileFailed,
        };

        match self.plan {
            FakePlan::Commit => {
                let readback = current.clone();
                if readback.document.raw_bytes != request.update.document_bytes
                    || readback.sidecar_bytes != request.update.sidecar_bytes
                    || readback.identity() != committed.identity()
                {
                    *current = previous.clone();
                    return Ok(ProfileConfigurationTransactionOutcome::RolledBack {
                        restored: current.clone(),
                        cause: Failure::new(
                            ErrorCode::Storage,
                            "fake transaction readback did not match the committed bytes",
                            true,
                        ),
                    });
                }
                Ok(ProfileConfigurationTransactionOutcome::Committed {
                    workspace: readback,
                    effect,
                })
            }
            FakePlan::ApplyFailureAndRestore => {
                *current = previous.clone();
                Ok(ProfileConfigurationTransactionOutcome::RolledBack {
                    restored: current.clone(),
                    cause: Failure::new(
                        ErrorCode::Configuration,
                        "fake native apply failed after candidate persistence",
                        false,
                    ),
                })
            }
            FakePlan::ReadbackMismatchAndRestore => {
                let mut corrupted = committed;
                corrupted.document.raw_bytes = b"readback mismatch".to_vec();
                *current = corrupted;
                let readback = current.clone();
                if readback.document.raw_bytes == request.update.document_bytes
                    && readback.sidecar_bytes == request.update.sidecar_bytes
                {
                    return Ok(ProfileConfigurationTransactionOutcome::Committed {
                        workspace: readback,
                        effect,
                    });
                }
                *current = previous.clone();
                Ok(ProfileConfigurationTransactionOutcome::RolledBack {
                    restored: current.clone(),
                    cause: Failure::new(
                        ErrorCode::Storage,
                        "fake transaction readback did not match the committed bytes",
                        true,
                    ),
                })
            }
            FakePlan::ApplyFailureAndRollbackFailure => {
                Ok(ProfileConfigurationTransactionOutcome::RollbackFailed {
                    cause: Failure::new(
                        ErrorCode::Configuration,
                        "fake native apply failed after candidate persistence",
                        false,
                    ),
                    rollback: Failure::new(
                        ErrorCode::Storage,
                        "fake rollback could not restore the old workspace",
                        true,
                    ),
                })
            }
        }
    }
}

fn scope(engine: &str, format: &str, schema: u32) -> ProfileConfigurationTransactionScope {
    ProfileConfigurationTransactionScope::new(ProfileDocumentCodecScope {
        engine_id: infiltrator_contract::core_identity::EngineId::new(engine)
            .expect("valid test engine"),
        format_id: ProfileFormatId::new(format).expect("valid test format"),
        schema_version: ProfileSchemaVersion::new(schema).expect("valid test schema"),
    })
}

fn workspace(
    scope: &ProfileConfigurationTransactionScope,
    revision: &str,
    bytes: &[u8],
) -> ProfileWorkspace {
    workspace_with_sidecar(scope, revision, bytes, None)
}

fn workspace_with_sidecar(
    scope: &ProfileConfigurationTransactionScope,
    revision: &str,
    bytes: &[u8],
    sidecar_bytes: Option<&[u8]>,
) -> ProfileWorkspace {
    let identity = ProfileDocumentIdentity::new(
        ProfileId::new("main").expect("valid test profile"),
        scope.document.engine_id.clone(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-main").expect("valid test source"),
        ),
        ProfileDocumentRevision::new(revision).expect("valid test revision"),
        scope.document.format_id.clone(),
        scope.document.schema_version,
    );
    ProfileWorkspace {
        document: ProfileDocument::new(identity, bytes.to_vec()),
        sidecar_revision: match sidecar_bytes {
            Some(_) => ProfileSidecarRevision::Present(
                ProfileDocumentRevision::new(format!("sidecar-{revision}"))
                    .expect("valid sidecar revision"),
            ),
            None => ProfileSidecarRevision::Absent,
        },
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes: sidecar_bytes.map(<[u8]>::to_vec),
    }
}

fn request(
    _transaction: &FakeTransaction,
    expected: ProfileWorkspaceIdentity,
    document_bytes: &[u8],
) -> ProfileConfigurationTransactionRequest {
    let expected_reference = ActiveProfileReference::new(
        expected.document.engine_id.clone(),
        expected.document.profile_id.clone(),
        ActiveProfileReferenceRevision::default(),
    );
    ProfileConfigurationTransactionRequest {
        expected_reference,
        expected,
        update: ProfileWorkspaceUpdate {
            purpose: infiltrator_ports::profile_workspace::ProfileWorkspacePurpose::Derived,
            document_bytes: document_bytes.to_vec(),
            sidecar_bytes: None,
        },
        authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
        strategy: ApplyStrategy::PreferReload,
    }
}

#[tokio::test]
async fn foreign_engine_format_and_schema_are_rejected_before_any_write_effect() {
    for foreign_scope in [
        scope("sing-box", "mihomo-yaml", 1),
        scope("mihomo", "sing-box-json", 1),
        scope("mihomo", "mihomo-yaml", 2),
    ] {
        let transaction = FakeTransaction::new(FakePlan::Commit);
        let foreign = workspace(&foreign_scope, "rev-1", b"port: 7890\n");
        let result = transaction
            .transact(request(&transaction, foreign.identity(), VALID))
            .await;
        assert!(matches!(result, Err(PortError::Unsupported { .. })));
        assert_eq!(transaction.writes(), 0);
        assert_eq!(transaction.runtime_effects(), 0);
    }
}

#[tokio::test]
async fn stale_revision_and_invalid_candidate_are_rejected_before_any_write_effect() {
    let transaction = FakeTransaction::new(FakePlan::Commit);
    let stale = workspace(&transaction.scope, "rev-0", b"port: 7890\n");
    assert!(matches!(
        transaction
            .transact(request(&transaction, stale.identity(), VALID))
            .await,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::NotReady,
            ..
        }))
    ));
    assert_eq!(transaction.writes(), 0);
    assert_eq!(transaction.runtime_effects(), 0);

    let current = transaction
        .current
        .lock()
        .expect("fake transaction state")
        .clone();
    assert!(matches!(
        transaction
            .transact(request(
                &transaction,
                current.identity(),
                b"invalid YAML candidate",
            ))
            .await,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::Configuration,
            ..
        }))
    ));
    assert_eq!(transaction.writes(), 0);
    assert_eq!(transaction.runtime_effects(), 0);
}

#[tokio::test]
async fn stopped_native_invalid_candidate_is_rejected_before_any_write_effect() {
    let transaction = FakeTransaction::stopped(FakePlan::Commit, NativeValidationPlan::Reject);
    let current = transaction
        .current
        .lock()
        .expect("fake transaction state")
        .clone();
    let original = current.clone();
    let result = transaction
        .transact(request(&transaction, current.identity(), VALID))
        .await;

    assert!(matches!(
        result,
        Err(PortError::Rejected(Failure {
            code: ErrorCode::Configuration,
            ..
        }))
    ));
    assert_eq!(transaction.native_validator.calls(), 1);
    assert_eq!(transaction.writes(), 0);
    assert_eq!(transaction.runtime_effects(), 0);
    assert_eq!(
        *transaction.current.lock().expect("fake transaction state"),
        original
    );
}

#[tokio::test]
async fn stopped_without_native_validator_is_unsupported_before_any_write_effect() {
    for lifecycle in [FakeLifecycle::Stopped, FakeLifecycle::Failed] {
        let mut transaction =
            FakeTransaction::stopped(FakePlan::Commit, NativeValidationPlan::Unsupported);
        transaction.lifecycle = lifecycle;
        let current = transaction
            .current
            .lock()
            .expect("fake transaction state")
            .clone();
        let original = current.clone();
        let result = transaction
            .transact(request(&transaction, current.identity(), VALID))
            .await;

        assert!(matches!(result, Err(PortError::Unsupported { .. })));
        assert_eq!(transaction.native_validator.calls(), 1);
        assert_eq!(transaction.writes(), 0);
        assert_eq!(transaction.runtime_effects(), 0);
        assert_eq!(
            *transaction.current.lock().expect("fake transaction state"),
            original
        );
    }
}

#[tokio::test]
async fn commit_returns_exact_document_and_sidecar_readback_with_new_revision() {
    let transaction = FakeTransaction::new(FakePlan::Commit);
    let before = transaction
        .current
        .lock()
        .expect("fake transaction state")
        .clone();
    let mut request = request(&transaction, before.identity(), VALID);
    request.update.sidecar_bytes = Some(b"# exact sidecar bytes\n".to_vec());
    let result = transaction.transact(request.clone()).await.unwrap();
    let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
        panic!("successful transaction must return a committed terminal state");
    };
    assert_eq!(
        effect,
        ProfileConfigurationCommitEffect::AppliedToRunningCore
    );
    assert_eq!(workspace.document.raw_bytes, request.update.document_bytes);
    assert_eq!(workspace.sidecar_bytes, request.update.sidecar_bytes);
    assert_ne!(workspace.identity(), before.identity());
    assert_eq!(transaction.writes(), 1);
    assert_eq!(transaction.runtime_effects(), 1);
}

#[tokio::test]
async fn inactive_commits_report_effect_and_have_no_runtime_effect() {
    for (lifecycle, expected_effect) in [
        (
            FakeLifecycle::Stopped,
            ProfileConfigurationCommitEffect::StoredWhileStopped,
        ),
        (
            FakeLifecycle::Failed,
            ProfileConfigurationCommitEffect::StoredWhileFailed,
        ),
    ] {
        let mut transaction =
            FakeTransaction::stopped(FakePlan::Commit, NativeValidationPlan::Accept);
        transaction.lifecycle = lifecycle;
        let before = transaction
            .current
            .lock()
            .expect("fake transaction state")
            .clone();
        let request = request(&transaction, before.identity(), VALID);
        let result = transaction.transact(request.clone()).await.unwrap();
        let ProfileConfigurationTransactionOutcome::Committed { workspace, effect } = result else {
            panic!("valid inactive transaction must commit");
        };

        assert_eq!(effect, expected_effect);
        assert_eq!(workspace.document.raw_bytes, request.update.document_bytes);
        assert_eq!(workspace.sidecar_bytes, request.update.sidecar_bytes);
        assert_ne!(workspace.identity(), before.identity());
        assert_eq!(transaction.writes(), 1);
        assert_eq!(transaction.runtime_effects(), 0);
        assert_eq!(transaction.lifecycle, lifecycle);
    }
}

#[tokio::test]
async fn failures_after_persistence_end_in_typed_rollback_states() {
    for (plan, expect_rollback_failed) in [
        (FakePlan::ApplyFailureAndRestore, false),
        (FakePlan::ApplyFailureAndRollbackFailure, true),
        (FakePlan::ReadbackMismatchAndRestore, false),
    ] {
        let transaction = FakeTransaction::new(plan);
        let before = transaction
            .current
            .lock()
            .expect("fake transaction state")
            .clone();
        let result = transaction
            .transact(request(&transaction, before.identity(), VALID))
            .await
            .unwrap();
        match (result, expect_rollback_failed) {
            (ProfileConfigurationTransactionOutcome::RolledBack { restored, .. }, false) => {
                assert_eq!(restored, before);
                assert_eq!(
                    transaction
                        .current
                        .lock()
                        .expect("fake transaction state")
                        .clone(),
                    before
                );
            }
            (ProfileConfigurationTransactionOutcome::RollbackFailed { .. }, true) => {}
            (state, _) => panic!("unexpected transaction terminal state: {state:?}"),
        }
        assert_eq!(transaction.writes(), 1);
        assert_eq!(transaction.runtime_effects(), 1);
    }
}

#[tokio::test]
async fn factory_binding_seals_one_transaction_owner_for_its_exact_scope() {
    let transaction = Arc::new(FakeTransaction::new(FakePlan::Commit));
    let binding = ProfileConfigurationTransactionBinding::new(transaction.scope());
    binding
        .bind(transaction.clone())
        .expect("first matching owner binds");
    assert!(
        binding
            .bind(Arc::new(FakeTransaction::new(FakePlan::Commit)))
            .is_err()
    );

    let before = transaction
        .current
        .lock()
        .expect("fake transaction state")
        .clone();
    let outcome = binding
        .transact(request(&transaction, before.identity(), VALID))
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        ProfileConfigurationTransactionOutcome::Committed {
            effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
            ..
        }
    ));
}
