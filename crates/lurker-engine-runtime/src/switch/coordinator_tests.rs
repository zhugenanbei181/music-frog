//! Deterministic owner-switch tests with fake host, providers and journal.

use super::coordinator::EngineSwitchCoordinator;
use super::host::EngineSwitchHost;
use super::memory_journal::MemoryEngineSwitchJournal;
use crate::registry::EngineBackendProviderRegistry;
use async_trait::async_trait;
use futures_executor::block_on;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchRecovery,
    EngineSwitchRevision, EngineSwitchStage, EngineSwitchTarget,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_ports::engine_backend::{EngineBackendBundle, EngineBackendProvider};
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn engine(value: &str) -> EngineId {
    EngineId::new(value).expect("test engine ID is valid")
}

/// Unwrap the failure side without requiring the success type to be `Debug`
/// (trait-object leases are deliberately not `Debug`).
trait ExpectSwitchError<T, E> {
    fn expect_switch_error(self, message: &str) -> E;
}

impl<T, E> ExpectSwitchError<T, E> for Result<T, E> {
    fn expect_switch_error(self, message: &str) -> E {
        match self {
            Ok(_) => panic!("{message}"),
            Err(error) => error,
        }
    }
}

fn mihomo() -> EngineId {
    engine("mihomo")
}

fn singbox() -> EngineId {
    engine("sing-box")
}

fn old_identity() -> CoreIdentity {
    CoreIdentity::new(
        mihomo(),
        CoreInstanceId::new("old-owner").expect("test instance ID is valid"),
        None,
    )
}

fn candidate_identity(engine_id: &EngineId) -> CoreIdentity {
    CoreIdentity::new(
        engine_id.clone(),
        CoreInstanceId::new("candidate").expect("test instance ID is valid"),
        None,
    )
}

fn old_generation() -> CoreGeneration {
    CoreGeneration::new(3)
}

fn selected(engine_id: EngineId) -> EngineSwitchTarget<EngineId> {
    EngineSwitchTarget::Selected {
        selection: engine_id.clone(),
        engine_id,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestBundle {
    engine_id: EngineId,
}

impl EngineBackendBundle for TestBundle {
    fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }
}

struct TestProvider {
    declared: EngineId,
    bundle_engine: EngineId,
    calls: Arc<AtomicUsize>,
}

impl EngineBackendProvider<TestBundle> for TestProvider {
    fn engine_id(&self) -> EngineId {
        self.declared.clone()
    }

    fn create_backend(&self) -> Result<TestBundle, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(TestBundle {
            engine_id: self.bundle_engine.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostCall {
    Validate,
    Prepare,
    Terminate,
    Apply,
    Close,
}

#[derive(Default)]
struct HostFaults {
    fail_validate: bool,
    fail_prepare: bool,
    fail_terminate: bool,
    fail_apply: bool,
    terminal_generation: Option<CoreGeneration>,
    applied_generation: Option<CoreGeneration>,
}

struct RecordingHost {
    log: Mutex<Vec<HostCall>>,
    faults: HostFaults,
}

impl RecordingHost {
    fn new(faults: HostFaults) -> Self {
        Self {
            log: Mutex::new(Vec::new()),
            faults,
        }
    }

    fn record(&self, call: HostCall) {
        self.log
            .lock()
            .expect("test host log lock is not poisoned")
            .push(call);
    }

    fn take_log(&self) -> Vec<HostCall> {
        mem::take(&mut *self.log.lock().expect("test host log lock is not poisoned"))
    }
}

#[async_trait]
impl EngineSwitchHost<TestBundle, EngineId> for RecordingHost {
    async fn validate_target(
        &self,
        _target: &EngineSwitchTarget<EngineId>,
    ) -> Result<(), PortError> {
        self.record(HostCall::Validate);
        if self.faults.fail_validate {
            return Err(PortError::Failed("target validation failed".to_owned()));
        }
        Ok(())
    }

    async fn prepare_candidate(
        &self,
        engine_id: &EngineId,
        _selection: &EngineId,
        _backend: &TestBundle,
    ) -> Result<CoreIdentity, PortError> {
        self.record(HostCall::Prepare);
        if self.faults.fail_prepare {
            return Err(PortError::Failed("candidate preparation failed".to_owned()));
        }
        Ok(candidate_identity(engine_id))
    }

    async fn terminate_old_owner(
        &self,
        _identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<CoreGeneration, PortError> {
        self.record(HostCall::Terminate);
        if self.faults.fail_terminate {
            return Err(PortError::Failed("old owner termination failed".to_owned()));
        }
        Ok(self.faults.terminal_generation.unwrap_or(generation))
    }

    async fn apply_candidate(
        &self,
        backend: TestBundle,
        _selection: &EngineId,
    ) -> Result<(CoreIdentity, CoreGeneration), PortError> {
        self.record(HostCall::Apply);
        if self.faults.fail_apply {
            return Err(PortError::Failed("candidate application failed".to_owned()));
        }
        Ok((
            candidate_identity(backend.engine_id()),
            self.faults
                .applied_generation
                .unwrap_or(CoreGeneration::new(1)),
        ))
    }

    async fn close_candidate(&self, _backend: TestBundle) -> Result<(), PortError> {
        self.record(HostCall::Close);
        Ok(())
    }
}

fn provider(
    declared: EngineId,
    bundle_engine: EngineId,
    calls: Arc<AtomicUsize>,
) -> Arc<dyn EngineBackendProvider<TestBundle>> {
    Arc::new(TestProvider {
        declared,
        bundle_engine,
        calls,
    })
}

fn registry(
    providers: Vec<Arc<dyn EngineBackendProvider<TestBundle>>>,
) -> Arc<EngineBackendProviderRegistry<TestBundle>> {
    Arc::new(
        EngineBackendProviderRegistry::new(providers).expect("test provider registry is valid"),
    )
}

fn bound_journal() -> Arc<MemoryEngineSwitchJournal> {
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo()));
    let (_snapshot, _lease) = block_on(journal.bind_owner(
        EngineSwitchRevision::new(0),
        mihomo(),
        old_identity(),
        old_generation(),
    ))
    .expect("initial owner binds");
    journal
}

struct Fixture {
    coordinator: EngineSwitchCoordinator<TestBundle, MemoryEngineSwitchJournal>,
    journal: Arc<MemoryEngineSwitchJournal>,
    host: Arc<RecordingHost>,
    provider_calls: Arc<AtomicUsize>,
}

fn fixture(faults: HostFaults) -> Fixture {
    let mihomo_calls = Arc::new(AtomicUsize::new(0));
    let singbox_calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(vec![
        provider(mihomo(), mihomo(), Arc::clone(&mihomo_calls)),
        provider(singbox(), singbox(), Arc::clone(&singbox_calls)),
    ]);
    fixture_with(registry, faults, singbox_calls)
}

fn fixture_with(
    registry: Arc<EngineBackendProviderRegistry<TestBundle>>,
    faults: HostFaults,
    provider_calls: Arc<AtomicUsize>,
) -> Fixture {
    let journal = bound_journal();
    let host = Arc::new(RecordingHost::new(faults));
    let coordinator = EngineSwitchCoordinator::new(
        registry,
        Arc::clone(&journal),
        Arc::clone(&host) as Arc<dyn EngineSwitchHost<TestBundle, EngineId>>,
    );
    Fixture {
        coordinator,
        journal,
        host,
        provider_calls,
    }
}

fn current_revision(journal: &MemoryEngineSwitchJournal) -> EngineSwitchRevision {
    block_on(journal.snapshot())
        .expect("journal snapshot reads")
        .revision
}

#[test]
fn switch_terminates_old_owner_before_starting_candidate() {
    let fixture = fixture(HostFaults::default());

    let snapshot = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect("switch succeeds");

    assert_eq!(
        fixture.host.take_log(),
        vec![
            HostCall::Validate,
            HostCall::Prepare,
            HostCall::Terminate,
            HostCall::Apply,
        ]
    );
    assert_eq!(fixture.provider_calls.load(Ordering::SeqCst), 1);
    assert_eq!(snapshot.selection, singbox());
    assert_eq!(
        snapshot.owner,
        EngineOwnerState::Bound {
            identity: candidate_identity(&singbox()),
            generation: CoreGeneration::new(1),
        }
    );
    assert_eq!(snapshot.revision, EngineSwitchRevision::new(6));
}

#[test]
fn failure_before_old_close_rolls_back_and_keeps_owner_bound() {
    let fixture = fixture(HostFaults {
        fail_prepare: true,
        ..HostFaults::default()
    });

    let error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect_switch_error("preparation failure fails the switch");

    assert_eq!(error.phase, EngineSwitchFailurePhase::BeforeReservation);
    assert_eq!(
        error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert!(!error.is_recovery_required());
    assert_eq!(
        fixture.host.take_log(),
        vec![HostCall::Validate, HostCall::Prepare]
    );
    let snapshot = block_on(fixture.journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(snapshot.revision, EngineSwitchRevision::new(1));
    assert_eq!(snapshot.selection, mihomo());
    assert_eq!(
        snapshot.owner,
        EngineOwnerState::Bound {
            identity: old_identity(),
            generation: old_generation(),
        }
    );
}

#[test]
fn reserved_switch_stops_at_binding_new_after_the_old_terminal() {
    let fixture = fixture(HostFaults::default());

    let reservation = block_on(
        fixture
            .coordinator
            .reserve_switch(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect("switch reserves to the new-owner admission");

    // The old owner reached its exact terminal before the host may apply the
    // candidate: `Apply` is not recorded until the host finishes the handoff.
    assert_eq!(
        fixture.host.take_log(),
        vec![HostCall::Validate, HostCall::Prepare, HostCall::Terminate]
    );
    assert_eq!(reservation.previous_selection(), &mihomo());
    assert_eq!(reservation.selection(), &singbox());
    assert_eq!(reservation.engine_id(), &singbox());
    assert_eq!(
        reservation.candidate_identity(),
        &candidate_identity(&singbox())
    );
    assert_eq!(
        reservation.previous_owner().0,
        &old_identity(),
        "the reservation keeps the exact terminated owner"
    );
    assert_eq!(reservation.terminal_generation(), old_generation());
    assert_eq!(reservation.backend().engine_id, singbox());
    assert_eq!(
        reservation.snapshot().owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew {
                selection: singbox(),
                engine_id: singbox(),
            },
        }
    );

    let snapshot = block_on(fixture.coordinator.finalize_reserved(
        reservation,
        candidate_identity(&singbox()),
        CoreGeneration::new(1),
    ))
    .expect("the host binds the exact applied candidate");
    assert_eq!(snapshot.selection, singbox());
    assert_eq!(
        snapshot.owner,
        EngineOwnerState::Bound {
            identity: candidate_identity(&singbox()),
            generation: CoreGeneration::new(1),
        }
    );
    assert!(
        fixture.host.take_log().is_empty(),
        "the deferred host applies the candidate itself; finalize must not re-apply it"
    );
}

#[test]
fn reserved_switch_is_cancelled_after_the_old_owner_closed() {
    let fixture = fixture(HostFaults::default());

    let reservation = block_on(
        fixture
            .coordinator
            .reserve_switch(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect("switch reserves");

    let snapshot =
        block_on(fixture.coordinator.cancel_reserved(reservation)).expect("reservation cancels");
    // The old owner cannot be restored: it already reached its terminal. The
    // committed selection stays and the journal becomes unowned.
    assert_eq!(snapshot.selection, singbox());
    assert_eq!(snapshot.owner, EngineOwnerState::Unowned);
    assert_eq!(
        fixture.host.take_log(),
        vec![HostCall::Validate, HostCall::Prepare, HostCall::Terminate]
    );
}

#[test]
fn reserved_switch_can_discard_a_proven_unstarted_candidate() {
    let fixture = fixture(HostFaults::default());

    let reservation = block_on(
        fixture
            .coordinator
            .reserve_switch(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect("switch reserves");

    let snapshot = block_on(
        fixture
            .coordinator
            .discard_reserved(reservation, CoreGeneration::new(0)),
    )
    .expect("a proven unstarted candidate is discarded");

    assert_eq!(snapshot.selection, singbox());
    assert!(matches!(
        snapshot.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::SelectionChanged { .. }
        }
    ));
}

#[test]
fn finalize_reserved_rejects_a_mismatching_candidate_and_stays_resumable() {
    let fixture = fixture(HostFaults::default());

    let reservation = block_on(
        fixture
            .coordinator
            .reserve_switch(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect("switch reserves");
    let pending = reservation.snapshot().clone();

    let error = block_on(fixture.coordinator.finalize_reserved(
        reservation,
        CoreIdentity::new(
            singbox(),
            CoreInstanceId::new("foreign-instance").expect("test instance ID is valid"),
            None,
        ),
        CoreGeneration::new(1),
    ))
    .expect_switch_error("a mismatching applied identity cannot bind");
    assert_eq!(error.phase, EngineSwitchFailurePhase::OldOwnerClosed);
    assert!(error.is_recovery_required());

    let after = block_on(fixture.journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(after, pending);
    assert!(matches!(
        after.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew { .. }
        }
    ));
}

#[test]
fn failure_after_old_close_requires_recovery_and_resumes() {
    let fixture = fixture(HostFaults {
        fail_apply: true,
        ..HostFaults::default()
    });

    let error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect_switch_error("application failure fails the switch");

    assert_eq!(error.phase, EngineSwitchFailurePhase::OldOwnerClosed);
    assert!(error.is_recovery_required());
    assert_eq!(error.disposition, EngineSwitchDisposition::RecoveryRequired);

    let pending = block_on(fixture.journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(pending.selection, singbox());
    assert!(matches!(
        pending.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew { .. }
        }
    ));
    assert_eq!(
        block_on(fixture.journal.recover_owner_switch(pending.revision))
            .expect("pending switch is visible"),
        EngineSwitchRecovery::ResumeBindingNew {
            selection: singbox(),
            engine_id: singbox(),
        }
    );

    let recovered = block_on(
        fixture
            .journal
            .recover_candidate_after_terminal(pending.revision, CoreGeneration::new(0)),
    )
    .expect("candidate terminal recovery succeeds");
    assert_eq!(recovered.selection, singbox());
    assert_eq!(recovered.owner, EngineOwnerState::Unowned);

    let (bound, _lease) = block_on(fixture.journal.bind_owner(
        recovered.revision,
        singbox(),
        candidate_identity(&singbox()),
        CoreGeneration::new(1),
    ))
    .expect("recovered journal binds a new owner");
    assert!(matches!(bound.owner, EngineOwnerState::Bound { .. }));
    assert_eq!(
        fixture.host.take_log(),
        vec![
            HostCall::Validate,
            HostCall::Prepare,
            HostCall::Terminate,
            HostCall::Apply,
        ]
    );
}

#[test]
fn rejected_old_terminal_evidence_requires_recovery_and_resumes() {
    let fixture = fixture(HostFaults {
        terminal_generation: Some(CoreGeneration::new(1)),
        ..HostFaults::default()
    });

    let error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect_switch_error("stale terminal generation fails the switch");
    assert_eq!(error.phase, EngineSwitchFailurePhase::OldOwnerClosed);
    assert!(error.is_recovery_required());

    let pending = block_on(fixture.journal.snapshot()).expect("journal snapshot reads");
    assert!(matches!(
        pending.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::ClosingOld { .. }
        }
    ));
    assert_eq!(
        block_on(fixture.journal.recover_owner_switch(pending.revision))
            .expect("pending switch is visible"),
        EngineSwitchRecovery::ResumeClosingOld {
            previous: mihomo(),
            owner_identity: old_identity(),
            owner_generation: old_generation(),
        }
    );

    let stale = block_on(
        fixture
            .journal
            .recover_old_owner_after_terminal(pending.revision, CoreGeneration::new(1)),
    )
    .expect_switch_error("terminal evidence older than the binding is rejected");
    assert!(matches!(stale, PortError::Rejected(_)));

    let stopped = block_on(
        fixture
            .journal
            .recover_old_owner_after_terminal(pending.revision, old_generation()),
    )
    .expect("exact terminal recovery succeeds");
    assert!(matches!(
        stopped.stage(),
        EngineSwitchStage::OldStopped { .. }
    ));

    let changed = block_on(stopped.compare_and_set_target(singbox(), singbox()))
        .expect("selection changes after terminal recovery");
    let binding = block_on(changed.reserve_new_owner(
        singbox(),
        candidate_identity(&singbox()),
        CoreGeneration::new(0),
    ))
    .expect("new owner reserves");
    let owner = block_on(binding.finalize(candidate_identity(&singbox()), CoreGeneration::new(1)))
        .expect("new owner finalizes");
    assert_eq!(owner.identity().engine, singbox());
    assert_eq!(owner.generation(), CoreGeneration::new(1));
}

#[test]
fn unregistered_target_is_rejected_before_any_process_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(vec![provider(mihomo(), mihomo(), Arc::clone(&calls))]);
    let fixture = fixture_with(registry, HostFaults::default(), Arc::clone(&calls));

    let error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(singbox())),
    )
    .expect_switch_error("unregistered engine fails the switch");

    assert_eq!(error.phase, EngineSwitchFailurePhase::BeforeReservation);
    assert_eq!(
        error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert_eq!(fixture.host.take_log(), vec![HostCall::Validate]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        current_revision(&fixture.journal),
        EngineSwitchRevision::new(1)
    );
}

#[test]
fn provider_bundle_engine_mismatch_is_rejected() {
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(vec![provider(mihomo(), singbox(), Arc::clone(&calls))]);
    let fixture = fixture_with(registry, HostFaults::default(), Arc::clone(&calls));

    let error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), selected(mihomo())),
    )
    .expect_switch_error("bundle engine mismatch fails the switch");

    assert_eq!(error.phase, EngineSwitchFailurePhase::BeforeReservation);
    assert_eq!(
        error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert_eq!(fixture.host.take_log(), vec![HostCall::Validate]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        current_revision(&fixture.journal),
        EngineSwitchRevision::new(1)
    );
}

#[test]
fn stale_revision_generation_and_lease_are_rejected() {
    let fixture = fixture(HostFaults::default());

    let error = block_on(
        fixture
            .coordinator
            .reserve_switch(EngineSwitchRevision::new(9), selected(singbox())),
    )
    .expect_switch_error("stale revision fails the switch");
    assert_eq!(error.phase, EngineSwitchFailurePhase::BeforeReservation);
    assert_eq!(
        error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert_eq!(fixture.host.take_log(), vec![HostCall::Validate]);

    let stale_generation = block_on(fixture.journal.begin_owner_switch(
        EngineSwitchRevision::new(1),
        mihomo(),
        old_identity(),
        CoreGeneration::new(2),
    ))
    .expect_switch_error("mismatched owner generation is rejected");
    assert!(matches!(stale_generation, PortError::Rejected(_)));

    let lease = block_on(fixture.journal.begin_owner_switch(
        EngineSwitchRevision::new(1),
        mihomo(),
        old_identity(),
        old_generation(),
    ))
    .expect("switch reserves");
    let pending = block_on(fixture.journal.snapshot()).expect("journal snapshot reads");
    let stopped = block_on(
        fixture
            .journal
            .recover_old_owner_after_terminal(pending.revision, old_generation()),
    )
    .expect("terminal recovery succeeds");
    let stale_lease = block_on(lease.cancel_before_old_owner_close())
        .expect_switch_error("a lease from an earlier revision is stale");
    assert!(matches!(stale_lease, PortError::Rejected(_)));
    drop(stopped);
}

#[test]
fn switch_requires_a_bound_owner_and_selected_target() {
    let unowned = Arc::new(MemoryEngineSwitchJournal::new(mihomo()));
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = registry(vec![provider(mihomo(), mihomo(), Arc::clone(&calls))]);
    let host = Arc::new(RecordingHost::new(HostFaults::default()));
    let coordinator = EngineSwitchCoordinator::new(
        registry,
        Arc::clone(&unowned),
        Arc::clone(&host) as Arc<dyn EngineSwitchHost<TestBundle, EngineId>>,
    );

    let unowned_error =
        block_on(coordinator.switch_and_finalize(EngineSwitchRevision::new(0), selected(mihomo())))
            .expect_switch_error("unowned switch requires a bound owner");
    assert_eq!(
        unowned_error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert_eq!(host.take_log(), vec![HostCall::Validate]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let fixture = fixture(HostFaults::default());
    let unselected_error = block_on(
        fixture
            .coordinator
            .switch_and_finalize(EngineSwitchRevision::new(1), EngineSwitchTarget::Unselected),
    )
    .expect_switch_error("unselected target is rejected");
    assert_eq!(
        unselected_error.disposition,
        EngineSwitchDisposition::RolledBackToPrevious {
            selection: mihomo(),
        }
    );
    assert!(fixture.host.take_log().is_empty());
    assert_eq!(fixture.provider_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn candidate_discard_and_abort_leave_explicit_states() {
    let journal = bound_journal();
    let lease = block_on(journal.begin_owner_switch(
        EngineSwitchRevision::new(1),
        mihomo(),
        old_identity(),
        old_generation(),
    ))
    .expect("switch reserves");
    let stopped = block_on(lease.confirm_old_owner_stopped(old_generation()))
        .expect("old owner confirms stopped");
    let changed =
        block_on(stopped.compare_and_set_target(singbox(), singbox())).expect("selection changes");
    let binding = block_on(changed.reserve_new_owner(
        singbox(),
        candidate_identity(&singbox()),
        CoreGeneration::new(0),
    ))
    .expect("candidate reserves");
    let discarded = block_on(binding.discard_unstarted_candidate(CoreGeneration::new(0)))
        .expect("unstarted candidate discards");
    assert!(matches!(
        discarded.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::SelectionChanged { .. }
        }
    ));
    assert_eq!(discarded.selection, singbox());

    let stopped =
        block_on(journal.recover_old_owner_after_terminal(discarded.revision, old_generation()))
            .expect("recovered stopped lease is available");
    let changed = block_on(stopped.compare_and_set_target(singbox(), singbox()))
        .expect("recovered target is idempotent");
    let binding = block_on(changed.reserve_new_owner(
        singbox(),
        candidate_identity(&singbox()),
        CoreGeneration::new(0),
    ))
    .expect("candidate reserves again");
    let aborted =
        block_on(binding.abort_before_core_application()).expect("pre-application abort succeeds");
    assert_eq!(aborted.owner, EngineOwnerState::Unowned);
    assert_eq!(aborted.selection, singbox());

    let (bound, _lease) = block_on(journal.bind_owner(
        aborted.revision,
        singbox(),
        candidate_identity(&singbox()),
        CoreGeneration::new(1),
    ))
    .expect("aborted journal binds a fresh owner");
    assert!(matches!(bound.owner, EngineOwnerState::Bound { .. }));
}
