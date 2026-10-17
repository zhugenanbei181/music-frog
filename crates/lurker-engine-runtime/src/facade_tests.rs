//! Facade behavior tests: builder fences, lifecycle, stream replay, switch
//! rollback/recovery and the single-control-operation fence.

use crate::facade::{
    LurkerRuntime, LurkerRuntimeBuilder, LurkerRuntimeState, RuntimeBuildError, RuntimeError,
    RuntimeFailurePhase,
};
use crate::switch::memory_journal::MemoryEngineSwitchJournal;
use async_trait::async_trait;
use futures_channel::oneshot;
use futures_executor::block_on;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchStage,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::core_process::{CoreProcess, CoreProcessStartAdmission, CoreReadiness};
use lurker_core_ports::engine_backend::{
    EngineBackendPorts, EngineBackendProvider, EngineOperationPorts,
};
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Waker};
use std::time::Duration;

fn instance(value: &str) -> CoreInstanceId {
    CoreInstanceId::new(value).expect("test instance ID is valid")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessCall {
    PrepareStart,
    Start,
    Stop,
    Status,
    Pid,
}

struct FakeProcess {
    identity: CoreIdentity,
    calls: Arc<Mutex<Vec<ProcessCall>>>,
    lifecycle: Arc<Mutex<CoreLifecycle>>,
    pid: Arc<Mutex<Option<u32>>>,
    admission: CoreProcessStartAdmission,
    prepare_gate: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    fail_start: bool,
    fail_stop: bool,
}

impl FakeProcess {
    fn new(engine_id: EngineId, instance_id: &str) -> Self {
        Self {
            identity: CoreIdentity::new(engine_id, instance(instance_id), None),
            calls: Arc::new(Mutex::new(Vec::new())),
            lifecycle: Arc::new(Mutex::new(CoreLifecycle::Stopped)),
            pid: Arc::new(Mutex::new(None)),
            admission: CoreProcessStartAdmission::LaunchNewProcess,
            prepare_gate: Arc::new(Mutex::new(None)),
            fail_start: false,
            fail_stop: false,
        }
    }

    fn with_prepare_gate(self, gate: oneshot::Receiver<()>) -> Self {
        *self
            .prepare_gate
            .lock()
            .expect("test prepare gate lock is not poisoned") = Some(gate);
        self
    }

    fn failing_start(mut self) -> Self {
        self.fail_start = true;
        self
    }

    fn record(&self, call: ProcessCall) {
        self.calls
            .lock()
            .expect("test process call log lock is not poisoned")
            .push(call);
    }

    fn current_lifecycle(&self) -> CoreLifecycle {
        self.lifecycle
            .lock()
            .expect("test process lifecycle lock is not poisoned")
            .clone()
    }

    fn current_pid(&self) -> Option<u32> {
        *self
            .pid
            .lock()
            .expect("test process pid lock is not poisoned")
    }

    fn calls(&self) -> Vec<ProcessCall> {
        self.calls
            .lock()
            .expect("test process call log lock is not poisoned")
            .clone()
    }
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn prepare_start(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartAdmission, PortError> {
        self.record(ProcessCall::PrepareStart);
        if !context.identity().same_instance(&self.identity) {
            return Err(PortError::Failed(
                "test start context targets another Core instance".to_owned(),
            ));
        }
        let gate = self
            .prepare_gate
            .lock()
            .expect("test prepare gate lock is not poisoned")
            .take();
        if let Some(gate) = gate {
            let _ = gate.await;
        }
        Ok(self.admission)
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.record(ProcessCall::Start);
        if self.fail_start {
            return Err(PortError::Failed("fake process start failed".to_owned()));
        }
        *self
            .lifecycle
            .lock()
            .expect("test process lifecycle lock is not poisoned") = CoreLifecycle::Running;
        *self
            .pid
            .lock()
            .expect("test process pid lock is not poisoned") = Some(4242);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.record(ProcessCall::Stop);
        if self.fail_stop {
            return Err(PortError::Failed("fake process stop failed".to_owned()));
        }
        *self
            .lifecycle
            .lock()
            .expect("test process lifecycle lock is not poisoned") = CoreLifecycle::Stopped;
        *self
            .pid
            .lock()
            .expect("test process pid lock is not poisoned") = None;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        self.record(ProcessCall::Status);
        Ok(self.current_lifecycle())
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }

    async fn pid(&self) -> Option<u32> {
        self.record(ProcessCall::Pid);
        self.current_pid()
    }
}

struct FakeReadiness {
    calls: Arc<AtomicUsize>,
    remaining_failures: AtomicUsize,
    version: Option<CoreVersion>,
}

impl FakeReadiness {
    fn ready() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            remaining_failures: AtomicUsize::new(0),
            version: Some(CoreVersion::new("1.2.3").expect("test version is valid")),
        }
    }

    fn never_ready() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            remaining_failures: AtomicUsize::new(usize::MAX),
            version: None,
        }
    }
}

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let remaining = self.remaining_failures.load(Ordering::SeqCst);
        if remaining > 0 {
            self.remaining_failures
                .store(remaining - 1, Ordering::SeqCst);
            return Err(PortError::Network(
                "fake readiness is not ready yet".to_owned(),
            ));
        }
        Ok("fake-ready".to_owned())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(self.version.clone())
    }
}

struct FakeCodec {
    scope: ProfileDocumentCodecScope,
}

impl ProfileDocumentCodecPort for FakeCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, _raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

struct FakeProvider {
    declared: EngineId,
    bundle_engine: EngineId,
    calls: Arc<AtomicUsize>,
    process: Arc<FakeProcess>,
    readiness: Arc<FakeReadiness>,
}

impl EngineBackendProvider<EngineBackendPorts> for FakeProvider {
    fn engine_id(&self) -> EngineId {
        self.declared.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let bundle_engine = self.bundle_engine.clone();
        let scope = ProfileDocumentCodecScope {
            engine_id: bundle_engine.clone(),
            format_id: ProfileFormatId::new("fake").expect("test format ID is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("test schema version is valid"),
        };
        let process: Arc<dyn CoreProcess> = self.process.clone();
        let readiness: Arc<dyn CoreReadiness> = self.readiness.clone();
        EngineBackendPorts::new(
            bundle_engine,
            process,
            readiness,
            Arc::new(FakeCodec { scope }),
            EngineOperationPorts::unsupported(
                "the facade test does not compose runtime operations",
            ),
        )
    }
}

struct EngineFixture {
    engine: EngineId,
    process: Arc<FakeProcess>,
    readiness: Arc<FakeReadiness>,
    provider_calls: Arc<AtomicUsize>,
}

impl EngineFixture {
    fn new(engine_id: EngineId, instance_id: &str) -> Self {
        let process = Arc::new(FakeProcess::new(engine_id.clone(), instance_id));
        Self::with_process(engine_id, process, Arc::new(FakeReadiness::ready()))
    }

    fn with_process(
        engine_id: EngineId,
        process: Arc<FakeProcess>,
        readiness: Arc<FakeReadiness>,
    ) -> Self {
        Self {
            engine: engine_id,
            process,
            readiness,
            provider_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn provider(&self) -> Arc<dyn EngineBackendProvider<EngineBackendPorts>> {
        self.provider_for(self.engine.clone(), self.engine.clone())
    }

    fn provider_for(
        &self,
        declared: EngineId,
        bundle_engine: EngineId,
    ) -> Arc<dyn EngineBackendProvider<EngineBackendPorts>> {
        Arc::new(FakeProvider {
            declared,
            bundle_engine,
            calls: Arc::clone(&self.provider_calls),
            process: Arc::clone(&self.process),
            readiness: Arc::clone(&self.readiness),
        })
    }
}

fn single_engine_runtime(
    engine_id: EngineId,
) -> (
    LurkerRuntime,
    Arc<MemoryEngineSwitchJournal>,
    Arc<FakeProcess>,
    Arc<FakeReadiness>,
    Arc<AtomicUsize>,
) {
    let fixture = EngineFixture::new(engine_id.clone(), "test-instance");
    let journal = Arc::new(MemoryEngineSwitchJournal::new(engine_id.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(fixture.provider())
        .expect("test provider registers")
        .select_engine(engine_id)
        .with_instance_id(instance("test-instance"))
        .with_switch_journal(Arc::clone(&journal))
        .with_readiness_timeout(Duration::from_millis(50))
        .build()
        .expect("test runtime builds");
    (
        runtime,
        journal,
        Arc::clone(&fixture.process),
        Arc::clone(&fixture.readiness),
        Arc::clone(&fixture.provider_calls),
    )
}

#[test]
fn builder_fails_closed_on_empty_registry_missing_selection_and_unregistered_engine() {
    assert_eq!(
        LurkerRuntimeBuilder::new()
            .select_engine(EngineId::mihomo())
            .build()
            .err(),
        Some(RuntimeBuildError::EmptyRegistry)
    );

    let fixture = EngineFixture::new(EngineId::mihomo(), "test-instance");
    assert_eq!(
        LurkerRuntimeBuilder::new()
            .register_engine(fixture.provider())
            .expect("test provider registers")
            .build()
            .err(),
        Some(RuntimeBuildError::MissingSelection)
    );
    assert_eq!(
        LurkerRuntimeBuilder::new()
            .register_engine(fixture.provider())
            .expect("test provider registers")
            .select_engine(EngineId::sing_box())
            .build()
            .err(),
        Some(RuntimeBuildError::EngineNotRegistered {
            engine_id: EngineId::sing_box()
        })
    );
    assert_eq!(
        LurkerRuntimeBuilder::new()
            .register_engine(fixture.provider())
            .expect("test provider registers")
            .register_engine(fixture.provider())
            .err(),
        Some(RuntimeBuildError::DuplicateProvider {
            engine_id: EngineId::mihomo()
        })
    );
}

#[test]
fn start_and_stop_publish_the_exact_lifecycle_and_terminal_evidence() {
    let engine_id = EngineId::mihomo();
    let (runtime, journal, process, readiness, provider_calls) =
        single_engine_runtime(engine_id.clone());
    let mut stream = runtime.state_stream();
    assert_eq!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Idle {
            selected: Some(engine_id.clone())
        })
    );

    block_on(runtime.start()).expect("start reaches readiness");
    assert_eq!(provider_calls.load(Ordering::SeqCst), 1);
    assert_eq!(readiness.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Starting {
            engine: engine_id.clone()
        })
    );
    let running = block_on(stream.next()).expect("running state is published");
    let scope = match &running {
        LurkerRuntimeState::Running {
            engine,
            scope,
            version,
        } => {
            assert_eq!(engine, &engine_id);
            assert_eq!(version.as_ref().map(CoreVersion::raw), Some("1.2.3"));
            scope.clone()
        }
        other => panic!("expected running, got {other:?}"),
    };
    assert_eq!(scope.identity.engine, engine_id);
    assert_eq!(scope.identity.instance, instance("test-instance"));
    assert_eq!(scope.generation, CoreGeneration::new(1));
    assert_eq!(runtime.scope(), Some(scope.clone()));
    assert!(matches!(
        runtime.outbound(),
        Err(PortError::Unsupported { .. })
    ));
    assert!(matches!(
        runtime.routing_intent(),
        Err(PortError::Unsupported { .. })
    ));
    assert!(matches!(
        block_on(journal.snapshot())
            .expect("journal snapshot reads")
            .owner,
        EngineOwnerState::Bound { .. }
    ));
    assert!(block_on(runtime.wait_ready(Duration::ZERO)).is_ok());
    assert_eq!(process.current_lifecycle(), CoreLifecycle::Running);
    assert_eq!(process.current_pid(), Some(4242));

    block_on(runtime.stop()).expect("stop reaches the exact terminal");
    assert_eq!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Stopping {
            engine: engine_id.clone(),
            scope: scope.clone()
        })
    );
    match block_on(stream.next()).expect("stopped state is published") {
        LurkerRuntimeState::Stopped { engine, terminal } => {
            assert_eq!(engine, Some(engine_id));
            let terminal = terminal.expect("terminal snapshot is present");
            assert_eq!(terminal.lifecycle, CoreLifecycle::Stopped);
            assert_eq!(terminal.identity, Some(scope.identity.clone()));
            assert_eq!(terminal.session_token, None);
        }
        other => panic!("expected stopped, got {other:?}"),
    }
    assert_eq!(runtime.scope(), None);
    assert_eq!(process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(process.current_pid(), None);
    assert!(process.calls().contains(&ProcessCall::Stop));
    assert!(matches!(
        block_on(runtime.wait_ready(Duration::ZERO)),
        Err(RuntimeError::NotReady { .. })
    ));
}

#[test]
fn unregistered_switch_is_rejected_before_any_provider_or_process_call() {
    let engine_id = EngineId::mihomo();
    let (runtime, journal, process, _readiness, provider_calls) = single_engine_runtime(engine_id);
    block_on(runtime.start()).expect("start succeeds");
    let provider_calls_before = provider_calls.load(Ordering::SeqCst);
    let process_calls_before = process.calls().len();
    let revision_before = block_on(journal.snapshot())
        .expect("journal snapshot reads")
        .revision;

    let error = block_on(runtime.switch_engine(EngineId::sing_box()))
        .expect_err("an unregistered target must fail");
    match error {
        RuntimeError::UnregisteredEngine { engine_id, .. } => {
            assert_eq!(engine_id, EngineId::sing_box());
        }
        other => panic!("expected unregistered engine, got {other:?}"),
    }
    assert_eq!(provider_calls.load(Ordering::SeqCst), provider_calls_before);
    assert_eq!(process.calls().len(), process_calls_before);
    assert_eq!(
        block_on(journal.snapshot())
            .expect("journal snapshot reads")
            .revision,
        revision_before
    );
    assert!(matches!(
        runtime.state(),
        LurkerRuntimeState::Running { .. }
    ));
}

#[test]
fn state_stream_replays_the_current_state_to_every_new_subscriber() {
    let engine_id = EngineId::mihomo();
    let (runtime, _journal, _process, _readiness, _calls) =
        single_engine_runtime(engine_id.clone());
    let mut first = runtime.state_stream();
    assert_eq!(
        block_on(first.next()),
        Some(LurkerRuntimeState::Idle {
            selected: Some(engine_id.clone())
        })
    );

    block_on(runtime.start()).expect("start succeeds");
    let mut late = runtime.state_stream();
    assert!(matches!(
        block_on(late.next()),
        Some(LurkerRuntimeState::Running { .. })
    ));
    assert!(matches!(
        block_on(first.next()),
        Some(LurkerRuntimeState::Starting { .. })
    ));
    assert!(matches!(
        block_on(first.next()),
        Some(LurkerRuntimeState::Running { .. })
    ));
}

#[test]
fn switch_engine_replaces_the_owner_and_publishes_the_new_running_state() {
    let mihomo = EngineId::mihomo();
    let singbox = EngineId::sing_box();
    let old = EngineFixture::new(mihomo.clone(), "old-instance");
    let candidate = EngineFixture::new(singbox.clone(), "candidate-instance");
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(old.provider())
        .expect("old provider registers")
        .register_engine(candidate.provider())
        .expect("candidate provider registers")
        .select_engine(mihomo.clone())
        .with_switch_journal(Arc::clone(&journal))
        .build()
        .expect("runtime builds");
    block_on(runtime.start()).expect("start succeeds");
    let old_scope = runtime.scope().expect("old owner scope is present");
    let mut stream = runtime.state_stream();
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Running { .. })
    ));

    block_on(runtime.switch_engine(singbox.clone())).expect("switch succeeds");
    let new_scope = runtime.scope().expect("new owner scope is present");
    assert_eq!(new_scope.identity.engine, singbox);
    assert_eq!(new_scope.identity.instance, instance("candidate-instance"));
    assert_eq!(new_scope.generation, CoreGeneration::new(1));
    assert_ne!(old_scope.identity.engine, new_scope.identity.engine);
    assert_eq!(old.process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(old.process.current_pid(), None);
    assert_eq!(
        candidate.process.current_lifecycle(),
        CoreLifecycle::Running
    );
    assert_eq!(candidate.process.current_pid(), Some(4242));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::ClosingOld { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::OldStopped { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::BindingNew { .. },
            ..
        })
    ));
    match block_on(stream.next()) {
        Some(LurkerRuntimeState::Running { engine, scope, .. }) => {
            assert_eq!(engine, singbox);
            assert_eq!(scope, new_scope);
        }
        other => panic!("expected the new running state, got {other:?}"),
    }
    let snapshot = block_on(journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(snapshot.selection, singbox);
    assert!(matches!(
        snapshot.owner,
        EngineOwnerState::Bound { identity, .. } if identity.engine == singbox
    ));
}

#[test]
fn a_switch_failing_before_the_reservation_rolls_back_to_the_live_owner() {
    let mihomo = EngineId::mihomo();
    let singbox = EngineId::sing_box();
    let old = EngineFixture::new(mihomo.clone(), "old-instance");
    let foreign_calls = Arc::new(AtomicUsize::new(0));
    let foreign: Arc<dyn EngineBackendProvider<EngineBackendPorts>> = Arc::new(FakeProvider {
        declared: singbox.clone(),
        bundle_engine: mihomo.clone(),
        calls: Arc::clone(&foreign_calls),
        process: Arc::clone(&old.process),
        readiness: Arc::clone(&old.readiness),
    });
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(old.provider())
        .expect("old provider registers")
        .register_engine(foreign)
        .expect("foreign provider registers")
        .select_engine(mihomo.clone())
        .with_switch_journal(Arc::clone(&journal))
        .build()
        .expect("runtime builds");
    block_on(runtime.start()).expect("start succeeds");
    let calls_before = old.process.calls();
    let scope_before = runtime.scope().expect("old owner scope is present");

    let error = block_on(runtime.switch_engine(singbox))
        .expect_err("a foreign bundle must fail the switch");
    match error {
        RuntimeError::Switch(switch) => {
            assert_eq!(switch.phase, EngineSwitchFailurePhase::BeforeReservation);
            assert!(!switch.is_recovery_required());
            assert_eq!(
                switch.disposition,
                EngineSwitchDisposition::RolledBackToPrevious {
                    selection: mihomo.clone()
                }
            );
        }
        other => panic!("expected switch failure, got {other:?}"),
    }
    assert_eq!(foreign_calls.load(Ordering::SeqCst), 1);
    assert_eq!(old.process.calls(), calls_before);
    assert_eq!(runtime.scope(), Some(scope_before));
    assert!(matches!(
        runtime.state(),
        LurkerRuntimeState::Running { engine, .. } if engine == mihomo
    ));
    assert!(matches!(
        block_on(journal.snapshot())
            .expect("journal snapshot reads")
            .owner,
        EngineOwnerState::Bound { .. }
    ));
}

#[test]
fn a_switch_failing_after_the_old_terminal_reports_recovery_required() {
    let mihomo = EngineId::mihomo();
    let singbox = EngineId::sing_box();
    let old = EngineFixture::new(mihomo.clone(), "old-instance");
    let candidate_process =
        Arc::new(FakeProcess::new(singbox.clone(), "candidate-instance").failing_start());
    let candidate = EngineFixture::with_process(
        singbox.clone(),
        candidate_process,
        Arc::new(FakeReadiness::ready()),
    );
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(old.provider())
        .expect("old provider registers")
        .register_engine(candidate.provider())
        .expect("candidate provider registers")
        .select_engine(mihomo.clone())
        .with_switch_journal(Arc::clone(&journal))
        .build()
        .expect("runtime builds");
    block_on(runtime.start()).expect("start succeeds");
    let mut stream = runtime.state_stream();
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Running { .. })
    ));

    let error = block_on(runtime.switch_engine(singbox.clone()))
        .expect_err("candidate start failure requires recovery");
    match error {
        RuntimeError::Switch(switch) => {
            assert_eq!(switch.phase, EngineSwitchFailurePhase::OldOwnerClosed);
            assert!(switch.is_recovery_required());
            assert_eq!(
                switch.disposition,
                EngineSwitchDisposition::RecoveryRequired
            );
        }
        other => panic!("expected switch failure, got {other:?}"),
    }
    match runtime.state() {
        LurkerRuntimeState::RecoveryRequired {
            selection, phase, ..
        } => {
            assert_eq!(selection, singbox);
            assert_eq!(phase, EngineSwitchFailurePhase::OldOwnerClosed);
        }
        other => panic!("expected recovery-required state, got {other:?}"),
    }
    assert_eq!(old.process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(old.process.current_pid(), None);
    assert_eq!(
        candidate.process.current_lifecycle(),
        CoreLifecycle::Stopped
    );
    let snapshot = block_on(journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(snapshot.selection, singbox);
    assert!(matches!(
        snapshot.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew { .. }
        }
    ));

    // The stream published the exact observable switch order: closing the old
    // owner, its exact terminal, the durable new-owner admission and recovery.
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::ClosingOld { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::OldStopped { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::BindingNew { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::RecoveryRequired { .. })
    ));
}

#[test]
fn a_second_control_operation_returns_control_busy_without_mutating_state() {
    let engine_id = EngineId::mihomo();
    let (gate, gate_receiver) = oneshot::channel();
    let process = Arc::new(
        FakeProcess::new(engine_id.clone(), "test-instance").with_prepare_gate(gate_receiver),
    );
    let fixture =
        EngineFixture::with_process(engine_id.clone(), process, Arc::new(FakeReadiness::ready()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(fixture.provider())
        .expect("test provider registers")
        .select_engine(engine_id.clone())
        .build()
        .expect("runtime builds");

    let mut first = Box::pin(runtime.start());
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(first.as_mut().poll(&mut context).is_pending());

    assert_eq!(block_on(runtime.start()), Err(RuntimeError::ControlBusy));
    assert_eq!(
        runtime.state(),
        LurkerRuntimeState::Starting {
            engine: engine_id.clone()
        }
    );
    assert_eq!(block_on(runtime.stop()), Err(RuntimeError::ControlBusy));
    assert_eq!(
        runtime.state(),
        LurkerRuntimeState::Starting { engine: engine_id }
    );

    drop(first);
    drop(gate);
}

#[test]
fn readiness_failure_publishes_failed_and_stops_the_started_process() {
    let engine_id = EngineId::mihomo();
    let process = Arc::new(FakeProcess::new(engine_id.clone(), "test-instance"));
    let fixture = EngineFixture::with_process(
        engine_id.clone(),
        Arc::clone(&process),
        Arc::new(FakeReadiness::never_ready()),
    );
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(fixture.provider())
        .expect("test provider registers")
        .select_engine(engine_id.clone())
        .with_readiness_timeout(Duration::from_millis(20))
        .build()
        .expect("runtime builds");

    let error = block_on(runtime.start()).expect_err("readiness never becomes ready");
    assert!(matches!(error, RuntimeError::Port(_)));
    match runtime.state() {
        LurkerRuntimeState::Failed { engine, phase, .. } => {
            assert_eq!(engine, Some(engine_id));
            assert_eq!(phase, RuntimeFailurePhase::Readiness);
        }
        other => panic!("expected failed state, got {other:?}"),
    }
    assert_eq!(process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(process.current_pid(), None);
    assert!(process.calls().contains(&ProcessCall::Stop));
}
