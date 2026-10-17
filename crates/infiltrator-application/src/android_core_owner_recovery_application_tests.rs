use super::{AndroidCoreOwnerRecoveryApplication, AndroidCoreOwnerRecoveryOutcome};
use crate::vpn_application::VpnServiceApplication;
use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidChildProcessIdentity, AndroidCoreProcessExitEvidence, AndroidCoreProcessObservation,
    AndroidCoreProcessRecoveryReceipt, AndroidCoreProcessUnavailableReason,
    AndroidOwnedCoreProcess,
};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
    ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::android_core_process_recovery::AndroidCoreProcessRecoveryPort;
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
enum RecoveryStep {
    ObserveChild,
    StopVpn,
    RecoverOwner,
    ConsumeExitRecord,
}

#[derive(Clone, Default)]
struct EventLog(Arc<Mutex<Vec<RecoveryStep>>>);

impl EventLog {
    fn record(&self, step: RecoveryStep) {
        self.0.lock().expect("recovery event log").push(step);
    }

    fn steps(&self) -> Vec<RecoveryStep> {
        self.0.lock().expect("recovery event log").clone()
    }
}

struct FakeProcess {
    observation: AndroidCoreProcessObservation,
    consume_result: Result<AndroidCoreProcessRecoveryReceipt, PortError>,
    consumed_contexts: Mutex<Vec<CoreProcessStartContext>>,
    events: EventLog,
}

#[async_trait]
impl AndroidCoreProcessRecoveryPort for FakeProcess {
    async fn observe(&self) -> Result<AndroidCoreProcessObservation, PortError> {
        self.events.record(RecoveryStep::ObserveChild);
        Ok(self.observation.clone())
    }

    async fn consume_exited(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt, PortError> {
        self.events.record(RecoveryStep::ConsumeExitRecord);
        self.consumed_contexts
            .lock()
            .expect("consumed contexts")
            .push(context.clone());
        self.consume_result.clone()
    }
}

struct FakeOwnerStore {
    outcome: Result<ActiveProfileOwnerRecoveryOutcome, PortError>,
    evidence: Mutex<Option<ActiveProfileOwnerRecoveryEvidence>>,
    events: EventLog,
}

#[async_trait]
impl ActiveProfileReferenceStore for FakeOwnerStore {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(ActiveProfileReferenceState::default())
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        Err(unexpected_call("acquire_selection_lease"))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        Err(unexpected_call("acquire_provider_selection_lease"))
    }

    async fn recover_bound_owner_after_terminal(
        &self,
        evidence: ActiveProfileOwnerRecoveryEvidence,
    ) -> Result<ActiveProfileOwnerRecoveryOutcome, PortError> {
        self.events.record(RecoveryStep::RecoverOwner);
        *self.evidence.lock().expect("recovery evidence") = Some(evidence);
        self.outcome.clone()
    }

    async fn compare_and_set(
        &self,
        _expected_revision: ActiveProfileReferenceRevision,
        _target: ActiveProfileReferenceTarget,
        _authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Err(unexpected_call("compare_and_set"))
    }
}

struct FakeVpnPort {
    stop_result: Result<VpnSessionSnapshot, PortError>,
    events: EventLog,
}

#[async_trait]
impl VpnServicePort for FakeVpnPort {
    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
        Err(unexpected_call("request_start"))
    }

    async fn prepare(
        &self,
        _prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError> {
        Err(unexpected_call("prepare"))
    }

    async fn start(&self, _request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
        Err(unexpected_call("start"))
    }

    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
        self.events.record(RecoveryStep::StopVpn);
        self.stop_result.clone()
    }

    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
        Err(unexpected_call("revoke"))
    }

    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
        Err(unexpected_call("snapshot"))
    }
}

fn unexpected_call(operation: &str) -> PortError {
    PortError::Failed(format!("unexpected test port call: {operation}"))
}

fn expected_identity() -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new("mihomo").expect("engine ID"),
        CoreInstanceId::new("android-active").expect("instance ID"),
        None,
    )
}

fn selected_reference() -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        EngineId::new("mihomo").expect("engine ID"),
        ProfileId::new("primary").expect("profile ID"),
        ActiveProfileReferenceRevision::new(4),
    ))
}

fn stopped_vpn() -> VpnSessionSnapshot {
    VpnSessionSnapshot {
        state: VpnSessionState::Stopped,
        revision: 8,
        ..VpnSessionSnapshot::default()
    }
}

fn exited_observation(context: CoreProcessStartContext) -> AndroidCoreProcessObservation {
    AndroidCoreProcessObservation::Exited {
        owned: AndroidOwnedCoreProcess {
            context,
            child: AndroidChildProcessIdentity {
                pid: 4123,
                start_time_ticks: 9876,
            },
        },
        evidence: AndroidCoreProcessExitEvidence::ProcessHandleExited,
    }
}

fn managed_context() -> CoreProcessStartContext {
    CoreProcessStartContext::managed(
        expected_identity(),
        CoreGeneration::new(11),
        SessionToken::new(73),
    )
}

fn application(
    observation: AndroidCoreProcessObservation,
    owner_outcome: Result<ActiveProfileOwnerRecoveryOutcome, PortError>,
    stop_result: Result<VpnSessionSnapshot, PortError>,
    consume_result: Result<AndroidCoreProcessRecoveryReceipt, PortError>,
) -> (
    AndroidCoreOwnerRecoveryApplication,
    Arc<FakeProcess>,
    Arc<FakeOwnerStore>,
    EventLog,
) {
    let events = EventLog::default();
    let process = Arc::new(FakeProcess {
        observation,
        consume_result,
        consumed_contexts: Mutex::new(Vec::new()),
        events: events.clone(),
    });
    let store = Arc::new(FakeOwnerStore {
        outcome: owner_outcome,
        evidence: Mutex::new(None),
        events: events.clone(),
    });
    let vpn = VpnServiceApplication::new(Arc::new(FakeVpnPort {
        stop_result,
        events: events.clone(),
    }));
    let recovery = AndroidCoreOwnerRecoveryApplication::new(
        process.clone(),
        store.clone(),
        vpn,
        expected_identity(),
    );
    (recovery, process, store, events)
}

fn not_ready(result: Result<AndroidCoreOwnerRecoveryOutcome, Failure>) {
    assert!(matches!(
        result,
        Err(Failure {
            code: ErrorCode::NotReady,
            ..
        })
    ));
}

#[tokio::test]
async fn never_started_stops_vpn_before_reconciling_the_unowned_proof() {
    let (application, process, store, events) = application(
        AndroidCoreProcessObservation::NeverStarted,
        Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::AlreadyAbsent),
    );

    assert_eq!(
        application.recover().await.expect("recovery"),
        AndroidCoreOwnerRecoveryOutcome::NoPreviousOwner
    );
    assert_eq!(
        events.steps(),
        vec![
            RecoveryStep::ObserveChild,
            RecoveryStep::StopVpn,
            RecoveryStep::RecoverOwner,
        ]
    );
    assert!(
        process
            .consumed_contexts
            .lock()
            .expect("consumed contexts")
            .is_empty()
    );
    assert!(matches!(
        store.evidence.lock().expect("recovery evidence").as_ref(),
        Some(ActiveProfileOwnerRecoveryEvidence::NeverStarted { terminal })
            if terminal.identity.as_ref().is_some_and(|identity| identity.same_instance(&expected_identity()))
                && terminal.lifecycle == CoreLifecycle::Stopped
                && terminal.generation == 0
                && terminal.session_token.is_none()
    ));
}

#[tokio::test]
async fn exited_managed_child_recovers_exact_owner_then_consumes_its_record() {
    let context = managed_context();
    let selected = selected_reference();
    let (application, process, store, events) = application(
        exited_observation(context.clone()),
        Ok(ActiveProfileOwnerRecoveryOutcome::Recovered(
            selected.clone(),
        )),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
    );

    assert_eq!(
        application.recover().await.expect("recovery"),
        AndroidCoreOwnerRecoveryOutcome::Recovered(selected)
    );
    assert_eq!(
        events.steps(),
        vec![
            RecoveryStep::ObserveChild,
            RecoveryStep::StopVpn,
            RecoveryStep::RecoverOwner,
            RecoveryStep::ConsumeExitRecord,
        ]
    );
    assert_eq!(
        *process.consumed_contexts.lock().expect("consumed contexts"),
        vec![context.clone()]
    );
    assert!(matches!(
        store.evidence.lock().expect("recovery evidence").as_ref(),
        Some(ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
            context: observed,
            terminal,
        }) if observed == &context
            && terminal.identity.as_ref().is_some_and(|identity| identity.same_instance(context.identity()))
            && terminal.lifecycle == CoreLifecycle::Stopped
            && terminal.generation == 11
            && terminal.session_token.is_none()
    ));
}

#[tokio::test]
async fn live_or_unobservable_child_keeps_vpn_and_owner_proof_untouched() {
    let context = managed_context();
    for observation in [
        AndroidCoreProcessObservation::Running {
            owned: match exited_observation(context.clone()) {
                AndroidCoreProcessObservation::Exited { owned, .. } => owned,
                _ => unreachable!(),
            },
        },
        AndroidCoreProcessObservation::Unavailable {
            reason: AndroidCoreProcessUnavailableReason::ProcessInspectionUnavailable,
        },
    ] {
        let (application, _process, store, events) = application(
            observation,
            Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
            Ok(stopped_vpn()),
            Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
        );

        not_ready(application.recover().await);
        assert_eq!(events.steps(), vec![RecoveryStep::ObserveChild]);
        assert!(store.evidence.lock().expect("recovery evidence").is_none());
    }
}

#[tokio::test]
async fn exited_child_requires_a_matching_owner_recovery_before_record_consumption() {
    let (application, process, store, events) = application(
        exited_observation(managed_context()),
        Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
    );

    not_ready(application.recover().await);
    assert_eq!(
        events.steps(),
        vec![
            RecoveryStep::ObserveChild,
            RecoveryStep::StopVpn,
            RecoveryStep::RecoverOwner,
        ]
    );
    assert!(
        process
            .consumed_contexts
            .lock()
            .expect("consumed contexts")
            .is_empty()
    );
    assert!(store.evidence.lock().expect("recovery evidence").is_some());
}

#[tokio::test]
async fn vpn_terminal_failure_prevents_owner_recovery() {
    let (application, _process, store, events) = application(
        AndroidCoreProcessObservation::NeverStarted,
        Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
        Err(PortError::Failed("VPN owner unavailable".to_owned())),
        Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
    );

    assert!(application.recover().await.is_err());
    assert_eq!(
        events.steps(),
        vec![RecoveryStep::ObserveChild, RecoveryStep::StopVpn]
    );
    assert!(store.evidence.lock().expect("recovery evidence").is_none());
}

#[tokio::test]
async fn nonterminal_vpn_readback_prevents_owner_recovery() {
    let (application, _process, store, events) = application(
        AndroidCoreProcessObservation::NeverStarted,
        Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
        Ok(VpnSessionSnapshot {
            state: VpnSessionState::Stopping,
            ..VpnSessionSnapshot::default()
        }),
        Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
    );

    assert!(application.recover().await.is_err());
    assert_eq!(
        events.steps(),
        vec![RecoveryStep::ObserveChild, RecoveryStep::StopVpn]
    );
    assert!(store.evidence.lock().expect("recovery evidence").is_none());
}

#[tokio::test]
async fn retry_accepts_already_absent_exit_record_after_exact_owner_recovery() {
    let context = managed_context();
    let selected = selected_reference();
    let (application, _process, _store, events) = application(
        exited_observation(context),
        Ok(ActiveProfileOwnerRecoveryOutcome::Recovered(
            selected.clone(),
        )),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::AlreadyAbsent),
    );

    assert_eq!(
        application.recover().await.expect("idempotent recovery"),
        AndroidCoreOwnerRecoveryOutcome::Recovered(selected)
    );
    assert_eq!(
        events.steps(),
        vec![
            RecoveryStep::ObserveChild,
            RecoveryStep::StopVpn,
            RecoveryStep::RecoverOwner,
            RecoveryStep::ConsumeExitRecord,
        ]
    );
}

#[tokio::test]
async fn mismatched_exit_record_receipt_fails_closed_after_owner_recovery() {
    let (application, process, store, events) = application(
        exited_observation(managed_context()),
        Ok(ActiveProfileOwnerRecoveryOutcome::Recovered(
            selected_reference(),
        )),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::ContextMismatch),
    );

    not_ready(application.recover().await);
    assert_eq!(
        events.steps(),
        vec![
            RecoveryStep::ObserveChild,
            RecoveryStep::StopVpn,
            RecoveryStep::RecoverOwner,
            RecoveryStep::ConsumeExitRecord,
        ]
    );
    assert_eq!(
        process
            .consumed_contexts
            .lock()
            .expect("consumed contexts")
            .len(),
        1
    );
    assert!(store.evidence.lock().expect("recovery evidence").is_some());
}

#[tokio::test]
async fn standalone_exit_context_is_rejected_before_vpn_or_store_mutation() {
    let context = CoreProcessStartContext::standalone(expected_identity());
    let (application, _process, store, events) = application(
        exited_observation(context),
        Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired),
        Ok(stopped_vpn()),
        Ok(AndroidCoreProcessRecoveryReceipt::Consumed),
    );

    not_ready(application.recover().await);
    assert_eq!(events.steps(), vec![RecoveryStep::ObserveChild]);
    assert!(store.evidence.lock().expect("recovery evidence").is_none());
}
