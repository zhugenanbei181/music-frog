//! Integration and unit tests for config apply transaction and YAML fidelity.

#[path = "profile_configuration_transaction_tests.rs"]
mod configuration_transactions;
#[path = "apply_source_race_tests.rs"]
mod source_races;
#[path = "apply_workspace_tests.rs"]
mod workspace;

use super::*;
use crate::apply_workspace::apply_active_profile_workspace_with_receipt;
use crate::history::{list_snapshots, read_snapshot};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization, CoreApplyFailureKind,
};
use infiltrator_contract::apply_transaction::ApplyTransactionStage;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::{ErrorCode, FailureReason};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::mixin::{MixinConfig, RuleMixin};
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspaceRecoveryLease,
    ActiveProfileWorkspaceRecoveryResolution, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::{result, time};
use tokio::fs::{read_to_string, write};
use tokio::time::{Instant, sleep};

const OLD: &str = "port: 7890\n";
const NEW: &str = "port: 7891\n";

struct MockController {
    running: AtomicBool,
    fail_starts_left: AtomicU64,
}

#[async_trait]
impl CoreProcess for MockController {
    async fn start(&self, _context: CoreProcessStartContext) -> result::Result<(), PortError> {
        if self.fail_starts_left.load(Ordering::SeqCst) > 0 {
            self.fail_starts_left.fetch_sub(1, Ordering::SeqCst);
            return Err(PortError::Failed("start rejected".into()));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn status(&self) -> result::Result<CoreLifecycle, PortError> {
        Ok(if self.running.load(Ordering::SeqCst) {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct TestLifecycleState {
    lifecycle: CoreLifecycle,
    generation: u64,
    session_token: Option<SessionToken>,
}

struct TestLifecycle {
    controller: Arc<MockController>,
    readiness_failures: AtomicU64,
    start_calls: AtomicU64,
    stop_calls: AtomicU64,
    state: Mutex<TestLifecycleState>,
}

impl TestLifecycle {
    fn new(controller: Arc<MockController>, readiness_failures: u64) -> Self {
        Self {
            controller,
            readiness_failures: AtomicU64::new(readiness_failures),
            start_calls: AtomicU64::new(0),
            stop_calls: AtomicU64::new(0),
            state: Mutex::new(TestLifecycleState {
                lifecycle: CoreLifecycle::Stopped,
                generation: 0,
                session_token: None,
            }),
        }
    }

    fn status(&self) -> CoreLifecycle {
        self.state.lock().expect("lifecycle lock").lifecycle.clone()
    }

    fn generation(&self) -> u64 {
        self.state.lock().expect("lifecycle lock").generation
    }

    fn session_token(&self) -> Option<SessionToken> {
        self.state.lock().expect("lifecycle lock").session_token
    }

    fn set_status(&self, lifecycle: CoreLifecycle) {
        self.state.lock().expect("lifecycle lock").lifecycle = lifecycle;
    }

    fn start_calls(&self) -> u64 {
        self.start_calls.load(Ordering::SeqCst)
    }

    fn stop_calls(&self) -> u64 {
        self.stop_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CoreLifecyclePort for TestLifecycle {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid test engine"),
            CoreInstanceId::new("apply-test-core").expect("valid test instance"),
            None,
        ))
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.status()
    }

    fn generation(&self) -> u64 {
        self.generation()
    }

    fn session_token(&self) -> Option<SessionToken> {
        TestLifecycle::session_token(self)
    }

    async fn start(&self) -> Result<u64, PortError> {
        self.start_calls.fetch_add(1, Ordering::SeqCst);
        let (generation, session_token) = {
            let mut state = self.state.lock().expect("lifecycle lock");
            state.generation += 1;
            state.lifecycle = CoreLifecycle::Starting;
            state.session_token = Some(SessionToken::new(state.generation as u128));
            (
                state.generation,
                state
                    .session_token
                    .expect("start allocated a session token"),
            )
        };
        let identity = self
            .core_identity()
            .expect("test lifecycle has a stable core identity");
        CoreProcess::start(
            self.controller.as_ref(),
            CoreProcessStartContext::managed(
                identity,
                CoreGeneration::new(generation),
                session_token,
            ),
        )
        .await?;
        Ok(generation)
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.stop_calls.fetch_add(1, Ordering::SeqCst);
        self.set_status(CoreLifecycle::Stopping);
        if let Err(error) = CoreProcess::stop(self.controller.as_ref()).await {
            self.set_status(CoreLifecycle::Failed);
            return Err(error);
        }
        self.set_status(CoreLifecycle::Stopped);
        self.state.lock().expect("lifecycle lock").session_token = None;
        Ok(())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        if self.status() != CoreLifecycle::Stopped {
            self.stop().await?;
        }
        self.start().await
    }

    async fn wait_for_ready(
        &self,
        generation: u64,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.generation() != generation {
                return Err(PortError::Failed(
                    "stale test lifecycle generation".to_string(),
                ));
            }
            if !matches!(
                CoreProcess::status(self.controller.as_ref()).await?,
                CoreLifecycle::Running | CoreLifecycle::Ready | CoreLifecycle::Starting
            ) {
                self.set_status(CoreLifecycle::Failed);
                return Err(PortError::Failed(
                    "test core exited before readiness".to_string(),
                ));
            }
            if self.readiness_failures.load(Ordering::SeqCst) > 0 {
                self.readiness_failures.fetch_sub(1, Ordering::SeqCst);
            } else {
                self.set_status(CoreLifecycle::Ready);
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.set_status(CoreLifecycle::Failed);
                return Err(PortError::Network("test readiness timed out".to_string()));
            }
            sleep(time::Duration::from_millis(1)).await;
        }
    }
}

struct MockReloader {
    fail: bool,
    calls: AtomicU64,
}

#[async_trait]
impl PreparedConfigReload for MockReloader {
    async fn reload(&self, _path: &Path) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err("reload rejected".into())
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl ConfigReloader for MockReloader {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        Ok(Box::new(self))
    }
}

struct MockStore {
    entries: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl SecureStore for MockStore {
    async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
        Ok(self
            .entries
            .lock()
            .expect("store lock")
            .get(&format!("{service}/{key}"))
            .cloned())
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        self.entries
            .lock()
            .expect("store lock")
            .insert(format!("{service}/{key}"), value.to_string());
        Ok(())
    }

    async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
        self.entries
            .lock()
            .expect("store lock")
            .remove(&format!("{service}/{key}"));
        Ok(())
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    session: TestLifecycle,
    config: ConfigManager<MockStore>,
    controller: Arc<MockController>,
    reloader: MockReloader,
}

async fn fixture(probe_failures: u64, reload_fails: bool) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ConfigManager::with_home_and_store(
        dir.path().to_path_buf(),
        MockStore {
            entries: Mutex::new(HashMap::new()),
        },
    )
    .expect("config manager");
    config
        .save_inactive_profile("main", OLD)
        .await
        .expect("seed profile");
    select_profile(&config, "main").await;

    let controller = Arc::new(MockController {
        running: AtomicBool::new(false),
        fail_starts_left: AtomicU64::new(0),
    });
    let session = TestLifecycle::new(controller.clone(), probe_failures);
    Fixture {
        _dir: dir,
        session,
        config,
        controller,
        reloader: MockReloader {
            fail: reload_fails,
            calls: AtomicU64::new(0),
        },
    }
}

async fn select_profile<S: SecureStore>(config: &ConfigManager<S>, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(config.engine_profile_store())
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ConfigManager::engine_id(config),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        config.engine_profile_store(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned test selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("test selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
}

async fn selected_reference<S: SecureStore>(config: &ConfigManager<S>) -> ActiveProfileReference {
    let profile_store = config.engine_profile_store();
    match ActiveProfileReferenceStore::read(profile_store)
        .await
        .expect("read active profile reference")
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        ActiveProfileReferenceState::Unselected { .. } => {
            panic!("apply test fixture must have a selected profile")
        }
    }
}

async fn publish_active_workspace_for_test(
    config: &ConfigManager<MockStore>,
    expected_reference: &ActiveProfileReference,
    expected_workspace: &ProfileWorkspaceIdentity,
    update: ProfileWorkspaceUpdate,
) -> ProfileWorkspace {
    let lease = ProfileStore::acquire_active_workspace_write_lease(
        config.engine_profile_store(),
        expected_reference.clone(),
        expected_workspace.clone(),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("acquire active workspace test lease");
    assert_eq!(
        lease.observed_workspace().identity(),
        *expected_workspace,
        "active workspace lease observes the requested identity"
    );

    match lease.publish(update).await {
        ActiveProfileWorkspacePublishOutcome::Published(published) => published.commit(),
        ActiveProfileWorkspacePublishOutcome::Unpublished(error) => {
            panic!("test workspace publication failed: {error}")
        }
        ActiveProfileWorkspacePublishOutcome::Superseded { observed } => {
            panic!("test workspace was superseded before publication: {observed:?}")
        }
        ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
            commit_recovered_workspace_publication(recovery).await
        }
    }
}

async fn commit_recovered_workspace_publication<'a>(
    mut recovery: Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>,
) -> ProfileWorkspace {
    loop {
        let state = recovery.state();
        match recovery.resolve().await {
            ActiveProfileWorkspaceRecoveryResolution::Published(published) => {
                return published.commit();
            }
            ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                restored.finish();
                panic!("test workspace publication recovered as restored: {state:?}");
            }
            ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                panic!("test workspace publication was not completed: {error}");
            }
            ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                panic!("test workspace was superseded during recovery: {observed:?}");
            }
            ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                recovery = next;
            }
        }
    }
}

async fn apply_to_selected_reference(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    content: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let expected = selected_reference(config).await;
    apply_workspace_content_for_reference(session, config, reloader, &expected, content, params)
        .await
}

async fn apply_confirmed_to_selected_reference(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    content: &str,
    params: ApplyParams,
    expected_rules: &RuleSourceIdentity,
) -> ApplyResult<ApplyOutcome> {
    let expected_reference = selected_reference(config).await;
    let workspace = ProfileStore::load_workspace(
        config.engine_profile_store(),
        &expected_reference.profile_id,
    )
    .await
    .map_err(|error| ApplyError::Write(error.to_string()))?;
    let existing = workspace
        .document
        .utf8_text()
        .map_err(|error| ApplyError::Validation(error.to_string()))?;
    if identify_rules_document(expected_reference.profile_id.as_str().to_owned(), existing)
        != *expected_rules
    {
        return Err(ApplyError::SourceChanged);
    }
    apply_workspace_content_for_reference_with_workspace(
        session,
        config,
        reloader,
        &expected_reference,
        &workspace,
        content,
        params,
    )
    .await
}

async fn apply_workspace_content_for_reference(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    content: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let workspace =
        ProfileStore::load_workspace(config.engine_profile_store(), &expected.profile_id)
            .await
            .map_err(|error| ApplyError::Write(error.to_string()))?;
    apply_workspace_content_for_reference_with_workspace(
        session, config, reloader, expected, &workspace, content, params,
    )
    .await
}

async fn apply_workspace_content_for_reference_with_workspace(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    workspace: &ProfileWorkspace,
    content: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: content.as_bytes().to_vec(),
        sidecar_bytes: workspace.sidecar_bytes.clone(),
    };
    let receipt = apply_active_profile_workspace_with_receipt(
        session,
        config,
        reloader,
        expected,
        &workspace.identity(),
        &update,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        params,
    )
    .await?;
    Ok(receipt.apply)
}

async fn apply_active_profile_doc(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    document: &SourceDoc,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    apply_workspace_content_for_reference(
        session,
        config,
        reloader,
        expected,
        &document.render(),
        params,
    )
    .await
}

async fn apply_active_profile_set_scalar(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    key: &str,
    value: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    super::apply_active_profile_set_scalar(
        session,
        config,
        reloader,
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        key,
        value,
        params,
    )
    .await
}

async fn apply_active_profile_append_rule(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    super::apply_active_profile_append_rule(
        session,
        config,
        reloader,
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        rule,
        params,
    )
    .await
}

async fn apply_active_profile_remove_rule(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    rule: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    super::apply_active_profile_remove_rule(
        session,
        config,
        reloader,
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        rule,
        params,
    )
    .await
}

async fn apply_active_profile_rewrite_anchors(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    prefix: &str,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    super::apply_active_profile_rewrite_anchors(
        session,
        config,
        reloader,
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        prefix,
        params,
    )
    .await
}

async fn apply_active_profile_mixin_fidelity(
    session: &TestLifecycle,
    config: &ConfigManager<MockStore>,
    reloader: &dyn ConfigReloader,
    expected: &ActiveProfileReference,
    mixin: &MixinConfig,
    params: ApplyParams,
) -> ApplyResult<ApplyOutcome> {
    super::apply_active_profile_mixin_fidelity(
        session,
        config,
        reloader,
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        mixin,
        params,
    )
    .await
}

#[test]
fn apply_error_port_mapping_preserves_typed_failure_classes() {
    let cases = [
        (
            ApplyError::Validation("invalid yaml".into()),
            ErrorCode::Configuration,
            CoreApplyFailureKind::Validation,
        ),
        (
            ApplyError::Write("disk error".into()),
            ErrorCode::Storage,
            CoreApplyFailureKind::Write,
        ),
        (
            ApplyError::Permission("denied".into()),
            ErrorCode::Permission,
            CoreApplyFailureKind::Permission,
        ),
        (
            ApplyError::Busy {
                status: CoreLifecycle::Starting,
            },
            ErrorCode::InvalidState,
            CoreApplyFailureKind::Busy,
        ),
        (
            ApplyError::NotRunning {
                status: CoreLifecycle::Stopped,
            },
            ErrorCode::NotReady,
            CoreApplyFailureKind::InactiveCore,
        ),
        (
            ApplyError::RolledBack {
                cause: "restart rejected".into(),
            },
            ErrorCode::InvalidState,
            CoreApplyFailureKind::RolledBack,
        ),
        (
            ApplyError::RollbackFailed {
                cause: "restart rejected".into(),
                rollback: "native rollback failed".into(),
            },
            ErrorCode::InvalidState,
            CoreApplyFailureKind::RollbackFailed,
        ),
        (
            ApplyError::Lifecycle("owner rejected".into()),
            ErrorCode::InvalidState,
            CoreApplyFailureKind::Lifecycle,
        ),
        (
            ApplyError::ReloadAndRestartFailed {
                reload: "reload rejected".into(),
                restart: "start rejected".into(),
            },
            ErrorCode::InvalidState,
            CoreApplyFailureKind::Lifecycle,
        ),
    ];
    for (error, expected_code, expected_kind) in cases {
        let failure = failure_from_apply_error(error);
        assert_eq!(failure.code, expected_code);
        assert_eq!(
            failure.reason,
            Some(FailureReason::CoreApplyFailure(expected_kind))
        );
    }

    let stale = failure_from_apply_error(ApplyError::ActiveProfileReferenceChanged {
        expected_revision: 7,
        observed_revision: 8,
    });
    assert_eq!(stale.code, ErrorCode::NotReady);
    assert_eq!(
        stale.reason,
        Some(FailureReason::ActiveProfileReferenceStale {
            expected_revision: 7,
            observed_revision: 8,
        })
    );
}

fn params(strategy: ApplyStrategy) -> ApplyParams {
    ApplyParams {
        strategy,
        health_timeout: Duration::from_secs(5),
        restart_timeout: Duration::from_secs(5),
        snapshot_history: false,
    }
}

async fn start_ready(fixture: &Fixture) {
    let generation = fixture.session.start().await.expect("start core");
    fixture
        .session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("core ready");
}

async fn file_content(config: &ConfigManager<MockStore>) -> String {
    let _current = config.get_current().await.expect("current");
    let path = config.get_current_path().await.expect("path");
    read_to_string(path).await.expect("profile file")
}

async fn replace_profile_file_for_test(
    config: &ConfigManager<MockStore>,
    profile: &str,
    content: &str,
) {
    let reference = selected_reference(config).await;
    assert_eq!(reference.profile_id.as_str(), profile);
    let before = ProfileStore::load_workspace(config.engine_profile_store(), &reference.profile_id)
        .await
        .expect("read active workspace before fixture edit");
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: content.as_bytes().to_vec(),
        sidecar_bytes: before.sidecar_bytes.clone(),
    };
    let committed =
        publish_active_workspace_for_test(config, &reference, &before.identity(), update).await;
    assert_eq!(committed.document.utf8_text().unwrap(), content);
}

async fn write_profile_file_outside_store_for_test(
    config: &ConfigManager<MockStore>,
    profile: &str,
    content: &str,
) {
    let path = config
        .existing_profile_document_path(profile)
        .await
        .expect("test profile path");
    write(path, content)
        .await
        .expect("simulate an out-of-store document edit");
}


#[path = "apply_lifecycle_tests.rs"]
mod lifecycle_tests;
