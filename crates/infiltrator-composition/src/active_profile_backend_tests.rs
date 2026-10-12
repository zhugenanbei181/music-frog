use super::{
    ActiveProfileBackendSelector, ActiveProfileBackendSwitchError,
    ActiveProfileBackendSwitchFailurePhase, PreviousCoreDisposition, PreviousCoreOwner,
};
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, CoreCapabilityEntry, CoreCapabilityMatrix, CoreCapabilityScope,
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::surface::HostKind;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState,
    ActiveProfileReferenceChangedLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::engine_backend_factory::{
    EngineBackendPorts, EngineBackendProvider, EngineBackendProviderAvailability,
    EngineOperationPorts,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep as tokio_sleep;

struct MemoryReferenceStore {
    state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
    delete_target_before_reservation: Option<Arc<AtomicBool>>,
    stale_next_begin: AtomicBool,
    fail_next_reservation: Arc<AtomicBool>,
    recovery_fixture: Mutex<MemoryOwnerSwitchRecoveryFixture>,
}

#[derive(Clone)]
enum MemoryOwnerSwitchRecoveryFixture {
    DurableJournal,
    CandidateTerminalEvidenceRequired {
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    },
}

struct MemoryReferenceLease {
    state: tokio::sync::OwnedMutexGuard<ActiveProfileReferenceState>,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FakeOwnerAdmission {
    Unowned,
    Bound {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
    Reserved {
        engine_id: EngineId,
    },
}

#[async_trait]
impl ActiveProfileReferenceLease for MemoryReferenceLease {
    fn state(&self) -> &ActiveProfileReferenceState {
        &self.state
    }

    async fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease>,
        PortError,
    > {
        let owner_state = Arc::clone(&self.owner);
        let transition_state = Arc::clone(&self.transition);
        let events = self.events.clone();
        match &*self.state {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == identity.engine => {}
            _ => {
                return Err(PortError::Failed(
                    "active reference does not authorize fake owner binding".into(),
                ));
            }
        }
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch is in progress",
                true,
            )));
        }
        let mut owner = self.owner.lock().expect("owner state");
        if *owner != FakeOwnerAdmission::Unowned {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake Core owner remains active",
                true,
            )));
        }
        *owner = FakeOwnerAdmission::Bound {
            identity: identity.clone(),
            generation,
        };
        drop(owner);
        drop(self);
        Ok(Box::new(MemoryOwnerLease {
            identity,
            generation,
            owner: owner_state,
            transition: transition_state,
            events,
        }))
    }
}

struct MemoryOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
}

#[async_trait]
impl ActiveProfileOwnerLease for MemoryOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn record_start_context(
        &self,
        _context: &infiltrator_contract::core_process::CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "owner lease requires matching Stopped readback without a session",
                true,
            )));
        }
        let mut owner = self.owner.lock().expect("owner state");
        if *owner
            != (FakeOwnerAdmission::Bound {
                identity: self.identity.clone(),
                generation: self.generation,
            })
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "owner lease is stale",
                true,
            )));
        }
        match self.transition.lock().expect("switch state").as_ref() {
            None => *owner = FakeOwnerAdmission::Unowned,
            Some(ActiveProfileOwnerSwitchState::ClosingOld {
                owner_identity,
                owner_generation,
                ..
            }) if owner_identity.same_instance(&self.identity)
                && terminal.generation >= owner_generation.value() =>
            {
                *owner = FakeOwnerAdmission::Unowned;
                if let Some(events) = &self.events {
                    events
                        .lock()
                        .expect("event lock")
                        .push("old-owner-released".into());
                }
            }
            Some(_) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "owner lease observed an invalid durable switch phase",
                    true,
                )));
            }
        }
        Ok(())
    }
}

struct MemoryOwnerSwitchLease {
    state: ActiveProfileOwnerSwitchState,
    previous_reference: ActiveProfileReferenceState,
    identity: CoreIdentity,
    generation: CoreGeneration,
    bound_owner_generation: CoreGeneration,
    root_state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
    fail_next_reservation: Arc<AtomicBool>,
}

#[async_trait]
impl ActiveProfileOwnerSwitchLease for MemoryOwnerSwitchLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let root_state = self.root_state.lock().await;
        let expected_owner = FakeOwnerAdmission::Bound {
            identity: self.identity.clone(),
            generation: self.bound_owner_generation,
        };
        if *root_state != self.previous_reference
            || *self.transition.lock().expect("switch state") != Some(self.state.clone())
            || *self.owner.lock().expect("owner state") != expected_owner
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch cannot cancel after the source owner or reference changed",
                true,
            )));
        }
        *self.transition.lock().expect("switch state") = None;
        record(&self.events, "switch-cancelled");
        Ok(root_state.clone())
    }

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch receipt is not the exact Stopped/no-session owner",
                true,
            )));
        }
        if *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake old owner binding was not released after terminal readback",
                true,
            )));
        }
        let state = ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference: self.previous_reference.clone(),
            owner_identity: self.identity.clone(),
            owner_generation: self.generation,
            terminal_generation: CoreGeneration::new(terminal.generation),
        };
        let mut transition = self.transition.lock().expect("switch state");
        if transition.as_ref() != Some(&self.state) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch phase changed before old-owner receipt",
                true,
            )));
        }
        *transition = Some(state.clone());
        drop(transition);
        record(&self.events, "old-owner-confirmed");
        Ok(Box::new(MemoryOwnerStoppedSwitchLease {
            state,
            previous_reference: self.previous_reference.clone(),
            root_state: Arc::clone(&self.root_state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
        }))
    }
}

struct MemoryOwnerStoppedSwitchLease {
    state: ActiveProfileOwnerSwitchState,
    previous_reference: ActiveProfileReferenceState,
    root_state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
    fail_next_reservation: Arc<AtomicBool>,
}

#[async_trait]
impl ActiveProfileOwnerStoppedSwitchLease for MemoryOwnerStoppedSwitchLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        target: ActiveProfileReferenceTarget,
    ) -> Result<Box<dyn ActiveProfileReferenceChangedLease>, PortError> {
        let (identity, terminal_generation) = match &self.state {
            ActiveProfileOwnerSwitchState::OldStopped {
                owner_identity,
                terminal_generation,
                ..
            } => (owner_identity.clone(), *terminal_generation),
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake owner-switch CAS requires OldStopped state",
                    true,
                )));
            }
        };
        if matches!(&target, ActiveProfileReferenceTarget::Unselected) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake owner switch only accepts a selected profile target",
                false,
            )));
        }
        if *self.transition.lock().expect("switch state") != Some(self.state.clone())
            || *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake switch state changed before target CAS",
                true,
            )));
        }
        let mut current = self.root_state.lock().await;
        if current.revision() != self.previous_reference.revision() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake active-profile revision is stale",
                true,
            )));
        }
        let revision = current
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("fake active-profile revision exhausted".into()))?;
        let selection = match target {
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
            ActiveProfileReferenceTarget::Unselected => unreachable!(),
        };
        *current = selection.clone();
        let state = ActiveProfileOwnerSwitchState::ReferenceChanged {
            previous_reference: self.previous_reference.clone(),
            selection: selection.clone(),
            owner_identity: identity,
            terminal_generation,
        };
        *self.transition.lock().expect("switch state") = Some(state.clone());
        drop(current);
        record(&self.events, "reference-cas");
        Ok(Box::new(MemoryReferenceChangedLease {
            state,
            selection,
            root_state: Arc::clone(&self.root_state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
            events: self.events.clone(),
        }))
    }
}

struct MemoryReferenceChangedLease {
    state: ActiveProfileOwnerSwitchState,
    selection: ActiveProfileReferenceState,
    root_state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    fail_next_reservation: Arc<AtomicBool>,
    events: Option<Arc<Mutex<Vec<String>>>>,
}

#[async_trait]
impl ActiveProfileReferenceChangedLease for MemoryReferenceChangedLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
    ) -> Result<Box<dyn ActiveProfileOwnerBindingLease>, PortError> {
        if self.fail_next_reservation.swap(false, Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake new-owner admission reservation failed",
                true,
            )));
        }
        let selected_engine = match &self.selection {
            ActiveProfileReferenceState::Selected(reference) => &reference.engine_id,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "fake owner reservation requires a selected reference",
                    false,
                )));
            }
        };
        if selected_engine != &engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake new-owner reservation engine differs from the committed selection",
                false,
            )));
        }
        let transition_matches =
            *self.transition.lock().expect("switch state") == Some(self.state.clone());
        let owner_is_unowned =
            *self.owner.lock().expect("owner state") == FakeOwnerAdmission::Unowned;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_is_unowned || *root_state != self.selection {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake switch state changed before owner reservation",
                true,
            )));
        }
        drop(root_state);
        let state = ActiveProfileOwnerSwitchState::BindingNew {
            selection: self.selection.clone(),
            engine_id: engine_id.clone(),
        };
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Reserved {
            engine_id: engine_id.clone(),
        };
        *self.transition.lock().expect("switch state") = Some(state.clone());
        record(&self.events, "new-owner-reserved");
        Ok(Box::new(MemoryOwnerBindingLease {
            state,
            engine_id,
            selection: self.selection.clone(),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            root_state: Arc::clone(&self.root_state),
            events: self.events.clone(),
        }))
    }
}

struct MemoryOwnerBindingLease {
    state: ActiveProfileOwnerSwitchState,
    engine_id: EngineId,
    selection: ActiveProfileReferenceState,
    owner: Arc<Mutex<FakeOwnerAdmission>>,
    transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    root_state: Arc<tokio::sync::Mutex<ActiveProfileReferenceState>>,
    events: Option<Arc<Mutex<Vec<String>>>>,
}

#[async_trait]
impl ActiveProfileOwnerBindingLease for MemoryOwnerBindingLease {
    fn state(&self) -> &ActiveProfileOwnerSwitchState {
        &self.state
    }

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        if identity.engine != self.engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake new Core identity engine differs from its reserved engine",
                false,
            )));
        }
        let expected_owner = FakeOwnerAdmission::Reserved {
            engine_id: self.engine_id.clone(),
        };
        if *self.transition.lock().expect("switch state") != Some(self.state.clone())
            || *self.owner.lock().expect("owner state") != expected_owner
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake new-owner reservation changed before finalization",
                true,
            )));
        }
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Bound {
            identity: identity.clone(),
            generation,
        };
        *self.transition.lock().expect("switch state") = None;
        record(&self.events, "new-owner-bound");
        Ok(Box::new(MemoryOwnerLease {
            identity,
            generation,
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
        }))
    }

    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        if terminal
            .identity
            .as_ref()
            .is_none_or(|identity| identity.engine != self.engine_id)
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate discard requires its Stopped/no-session target engine",
                true,
            )));
        }
        let expected_owner = FakeOwnerAdmission::Reserved {
            engine_id: self.engine_id.clone(),
        };
        let transition_matches =
            *self.transition.lock().expect("switch state") == Some(self.state.clone());
        let owner_matches = *self.owner.lock().expect("owner state") == expected_owner;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_matches || *root_state != self.selection {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate reservation changed before discard",
                true,
            )));
        }
        drop(root_state);
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Unowned;
        *self.transition.lock().expect("switch state") = None;
        record(&self.events, "candidate-discarded");
        Ok(self.selection.clone())
    }

    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let expected_owner = FakeOwnerAdmission::Reserved {
            engine_id: self.engine_id.clone(),
        };
        let transition_matches =
            *self.transition.lock().expect("switch state") == Some(self.state.clone());
        let owner_matches = *self.owner.lock().expect("owner state") == expected_owner;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_matches || *root_state != self.selection {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate reservation changed before abort",
                true,
            )));
        }
        drop(root_state);
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Unowned;
        *self.transition.lock().expect("switch state") = None;
        record(&self.events, "candidate-aborted");
        Ok(self.selection.clone())
    }
}

impl MemoryReferenceStore {
    fn new(state: ActiveProfileReferenceState) -> Self {
        Self {
            state: Arc::new(tokio::sync::Mutex::new(state)),
            owner: Arc::new(Mutex::new(FakeOwnerAdmission::Unowned)),
            transition: Arc::new(Mutex::new(None)),
            events: None,
            delete_target_before_reservation: None,
            stale_next_begin: AtomicBool::new(false),
            fail_next_reservation: Arc::new(AtomicBool::new(false)),
            recovery_fixture: Mutex::new(MemoryOwnerSwitchRecoveryFixture::DurableJournal),
        }
    }

    fn with_events(state: ActiveProfileReferenceState, events: Arc<Mutex<Vec<String>>>) -> Self {
        let mut store = Self::new(state);
        store.events = Some(events);
        store
    }

    fn with_target_deletion_on_begin(
        state: ActiveProfileReferenceState,
        events: Arc<Mutex<Vec<String>>>,
        deleted: Arc<AtomicBool>,
    ) -> Self {
        let mut store = Self::with_events(state, events);
        store.delete_target_before_reservation = Some(deleted);
        store
    }

    fn reject_next_begin_as_stale(&self) {
        self.stale_next_begin.store(true, Ordering::SeqCst);
    }

    fn reject_next_reservation(&self) {
        self.fail_next_reservation.store(true, Ordering::SeqCst);
    }

    fn record(&self, event: &str) {
        record(&self.events, event);
    }
}

fn record(events: &Option<Arc<Mutex<Vec<String>>>>, event: &str) {
    if let Some(events) = events {
        events.lock().expect("event lock").push(event.into());
    }
}

#[async_trait]
impl ActiveProfileReferenceStore for MemoryReferenceStore {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self.state.lock().await.clone())
    }

    async fn recover_owner_switch(&self) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        let fixture = self
            .recovery_fixture
            .lock()
            .expect("recovery fixture")
            .clone();
        if let MemoryOwnerSwitchRecoveryFixture::CandidateTerminalEvidenceRequired {
            selection,
            engine_id,
        } = fixture
        {
            return Ok(
                ActiveProfileOwnerSwitchRecovery::CandidateTerminalEvidenceRequired {
                    selection,
                    engine_id,
                },
            );
        }
        let state = self.transition.lock().expect("switch state").clone();
        match state {
            None => Ok(ActiveProfileOwnerSwitchRecovery::NoPendingSwitch),
            Some(state) => match &state {
                ActiveProfileOwnerSwitchState::ClosingOld {
                    previous_reference,
                    owner_identity,
                    owner_generation,
                } => Ok(ActiveProfileOwnerSwitchRecovery::ResumeClosingOld(
                    Box::new(MemoryOwnerSwitchLease {
                        state: state.clone(),
                        previous_reference: previous_reference.clone(),
                        identity: owner_identity.clone(),
                        generation: *owner_generation,
                        bound_owner_generation: *owner_generation,
                        root_state: Arc::clone(&self.state),
                        owner: Arc::clone(&self.owner),
                        transition: Arc::clone(&self.transition),
                        events: self.events.clone(),
                        fail_next_reservation: Arc::clone(&self.fail_next_reservation),
                    }),
                )),
                ActiveProfileOwnerSwitchState::OldStopped {
                    previous_reference, ..
                } => Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                    Box::new(MemoryOwnerStoppedSwitchLease {
                        state: state.clone(),
                        previous_reference: previous_reference.clone(),
                        root_state: Arc::clone(&self.state),
                        owner: Arc::clone(&self.owner),
                        transition: Arc::clone(&self.transition),
                        events: self.events.clone(),
                        fail_next_reservation: Arc::clone(&self.fail_next_reservation),
                    }),
                )),
                ActiveProfileOwnerSwitchState::ReferenceChanged { selection, .. } => {
                    Ok(ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(
                        Box::new(MemoryReferenceChangedLease {
                            state: state.clone(),
                            selection: selection.clone(),
                            root_state: Arc::clone(&self.state),
                            owner: Arc::clone(&self.owner),
                            transition: Arc::clone(&self.transition),
                            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
                            events: self.events.clone(),
                        }),
                    ))
                }
                ActiveProfileOwnerSwitchState::BindingNew {
                    selection,
                    engine_id,
                } => Ok(ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(
                    Box::new(MemoryOwnerBindingLease {
                        state: state.clone(),
                        engine_id: engine_id.clone(),
                        selection: selection.clone(),
                        owner: Arc::clone(&self.owner),
                        transition: Arc::clone(&self.transition),
                        root_state: Arc::clone(&self.state),
                        events: self.events.clone(),
                    }),
                )),
            },
        }
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease>,
        PortError,
    > {
        let state = Arc::clone(&self.state).lock_owned().await;
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch blocks selection leases",
                true,
            )));
        }
        Ok(Box::new(MemoryReferenceLease {
            state,
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
        }))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<
        Box<dyn infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease>,
        PortError,
    > {
        let state = Arc::clone(&self.state).lock_owned().await;
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch blocks provider selection leases",
                true,
            )));
        }
        if *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake active Core owner must close before another provider is composed",
                true,
            )));
        }
        Ok(Box::new(MemoryReferenceLease {
            state,
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
        }))
    }

    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let mut state = self.state.lock().await;
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch blocks ordinary active-reference CAS",
                true,
            )));
        }
        if state.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        let owner_state = self.owner.lock().expect("owner state").clone();
        match (&owner_state, authorization) {
            (FakeOwnerAdmission::Unowned, ActiveProfileReferenceWriteAuthorization::Unowned) => {}
            (
                FakeOwnerAdmission::Bound {
                    identity,
                    generation,
                },
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: authorized_identity,
                    generation: authorized_generation,
                },
            ) if identity.same_instance(&authorized_identity)
                && generation == &authorized_generation => {}
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake active-reference write authorization does not match its owner",
                    true,
                )));
            }
        }
        match owner_state {
            FakeOwnerAdmission::Unowned => {}
            FakeOwnerAdmission::Bound {
                identity,
                generation,
            } => match &target {
                ActiveProfileReferenceTarget::Selected { engine_id, .. }
                    if engine_id == &identity.engine => {}
                _ => {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        format!(
                            "fake Core owner {} at generation {} must terminate before engine selection changes",
                            identity.instance.as_str(),
                            generation.value()
                        ),
                        true,
                    )));
                }
            },
            FakeOwnerAdmission::Reserved { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake new Core owner admission is reserved",
                    true,
                )));
            }
        }
        let revision = expected_revision
            .next()
            .ok_or_else(|| PortError::Failed("active profile revision exhausted".into()))?;
        *state = match target {
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
        Ok(state.clone())
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        if self.stale_next_begin.swap(false, Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch revision became stale during preflight",
                true,
            )));
        }
        let current_state = self.state.lock().await;
        let previous_reference = current_state.clone();
        if previous_reference.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch revision is stale",
                true,
            )));
        }
        let current_reference = match &previous_reference {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake owner switch requires a selected current reference",
                    true,
                )));
            }
        };
        if current_reference.engine_id != expected_owner.engine {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner identity differs from current selection",
                true,
            )));
        }
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch is already active",
                true,
            )));
        }
        let bound_owner_generation = match self.owner.lock().expect("owner state").clone() {
            FakeOwnerAdmission::Bound {
                identity,
                generation,
            } if identity.same_instance(&expected_owner) && expected_generation == generation => {
                generation
            }
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake expected owner identity/generation is stale",
                    true,
                )));
            }
        };
        let state = ActiveProfileOwnerSwitchState::ClosingOld {
            previous_reference: previous_reference.clone(),
            owner_identity: expected_owner.clone(),
            owner_generation: expected_generation,
        };
        if let Some(deleted) = &self.delete_target_before_reservation {
            deleted.store(true, Ordering::SeqCst);
            self.record("target-deleted-before-reservation");
        }
        *self.transition.lock().expect("switch state") = Some(state.clone());
        drop(current_state);
        self.record("switch-reserved");
        Ok(Box::new(MemoryOwnerSwitchLease {
            state,
            previous_reference,
            identity: expected_owner,
            generation: expected_generation,
            bound_owner_generation,
            root_state: Arc::clone(&self.state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
        }))
    }
}

struct FakeProfileStore {
    engine_id: EngineId,
    document: ProfileDocument,
    current_reads: AtomicUsize,
    workspace_reads: AtomicUsize,
    deleted: Option<Arc<AtomicBool>>,
}

impl FakeProfileStore {
    fn new(engine_id: &str, document_engine: &str, document_profile: &str) -> Self {
        Self {
            engine_id: engine_id_value(engine_id),
            document: profile_document(document_engine, document_profile),
            current_reads: AtomicUsize::new(0),
            workspace_reads: AtomicUsize::new(0),
            deleted: None,
        }
    }

    fn with_deletion_flag(mut self, deleted: Arc<AtomicBool>) -> Self {
        self.deleted = Some(deleted);
        self
    }
}

#[async_trait]
impl ProfileStore for FakeProfileStore {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/memory/profiles")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(Vec::new())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        self.current_reads.fetch_add(1, Ordering::SeqCst);
        Ok("namespace-current-must-not-select-provider".into())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(String::from_utf8_lossy(&self.document.raw_bytes).into_owned())
    }

    async fn save_inactive_profile(&self, _profile: &str, _content: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        _metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        if matches!(&self.deleted, Some(deleted) if deleted.load(Ordering::SeqCst)) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake target profile was deleted before the switch reservation",
                true,
            )));
        }
        Ok(ProfileWorkspace {
            document: self.document.clone(),
            sidecar_revision: ProfileSidecarRevision::Absent,
            write_protection: ProfileWriteProtection::Editable,
            sidecar_bytes: None,
        })
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Err(unsupported_profiles())
    }

    async fn restore_inactive_profile_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Err(unsupported_profiles())
    }
}

fn unsupported_profiles() -> PortError {
    PortError::unsupported(Capability::Profiles, "unused fake profile operation")
}

#[derive(Clone, Copy)]
enum CapabilityFixtureState {
    Available,
    TargetUnavailable,
    StartUnsupported,
}

struct FakeCapabilities {
    state: CapabilityFixtureState,
}

impl CoreCapabilityProvider for FakeCapabilities {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        if matches!(self.state, CapabilityFixtureState::TargetUnavailable) {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "this target has no sing-box capability evidence".into(),
            });
        }
        let target = EngineBinaryTarget::current();
        let host = target.host_kind().unwrap_or(HostKind::Desktop);
        let engine = if matches!(self.state, CapabilityFixtureState::StartUnsupported) {
            EngineCapabilityDecision::Unsupported {
                reason: "Core start is not supported by the selected adapter".into(),
            }
        } else {
            EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::CoreLifecycle,
            }
        };
        CoreCapabilityMatrix::new(
            CoreCapabilityScope::new_for_target(
                engine_id.clone(),
                core_version.cloned(),
                host,
                target,
                infiltrator_contract::core_capabilities::AdapterRevision::new(1),
            ),
            vec![CoreCapabilityEntry::new(
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                engine,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )],
        )
        .map_err(|error| CoreCapabilityProviderError::MatrixUnavailable {
            reason: error.to_string(),
        })
    }
}

#[derive(Default)]
struct ProviderCounts {
    created: AtomicUsize,
    starts: AtomicUsize,
}

struct FakeProvider {
    engine_id: EngineId,
    instance_id: CoreInstanceId,
    counts: Arc<ProviderCounts>,
    events: Arc<Mutex<Vec<String>>>,
    identity_engine_override: Option<EngineId>,
    fail_creation: bool,
}

impl FakeProvider {
    fn new(
        engine: &str,
        instance: &str,
        events: Arc<Mutex<Vec<String>>>,
    ) -> (Self, Arc<ProviderCounts>) {
        let counts = Arc::new(ProviderCounts::default());
        (
            Self {
                engine_id: engine_id_value(engine),
                instance_id: CoreInstanceId::new(instance).unwrap(),
                counts: Arc::clone(&counts),
                events,
                identity_engine_override: None,
                fail_creation: false,
            },
            counts,
        )
    }

    fn with_identity_engine(mut self, engine: &str) -> Self {
        self.identity_engine_override = Some(engine_id_value(engine));
        self
    }

    fn failing_creation(mut self) -> Self {
        self.fail_creation = true;
        self
    }
}

impl EngineBackendProvider for FakeProvider {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        self.counts.created.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push(format!("provider:{}", self.engine_id.as_str()));
        if self.fail_creation {
            return Err(PortError::Failed("fake provider dispatch failed".into()));
        }
        let identity = CoreIdentity::new(
            self.identity_engine_override
                .clone()
                .unwrap_or_else(|| self.engine_id.clone()),
            self.instance_id.clone(),
            Some(CoreVersion::new("test-version").unwrap()),
        );
        let process: Arc<dyn CoreProcess> = Arc::new(FakeProcess {
            identity,
            starts: Arc::clone(&self.counts),
        });
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: self.engine_id.clone(),
            format_id: format_id(self.engine_id.as_str()),
            schema_version: ProfileSchemaVersion::new(1).unwrap(),
        };
        let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(FakeCodec {
            scope: codec_scope.clone(),
        });
        let transaction = Arc::new(ProfileConfigurationTransactionBinding::new(
            ProfileConfigurationTransactionScope::new(codec_scope),
        ));
        EngineBackendPorts::new(
            self.engine_id.clone(),
            process,
            Arc::new(FakeReadiness),
            codec,
            transaction,
            EngineOperationPorts::unsupported("selection test has no runtime operations"),
        )
    }
}

struct FakeProcess {
    identity: CoreIdentity,
    starts: Arc<ProviderCounts>,
}

#[async_trait]
impl CoreProcess for FakeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(
        &self,
        _context: infiltrator_ports::core_process::CoreProcessStartContext,
    ) -> Result<(), PortError> {
        self.starts.starts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct FakeReadiness;

#[async_trait]
impl CoreReadiness for FakeReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("ready".into())
    }
}

struct FakeCodec {
    scope: ProfileDocumentCodecScope,
}

impl ProfileDocumentCodecPort for FakeCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

struct FakeOwner {
    before: CoreLifecycleSnapshot,
    terminal: Result<CoreLifecycleSnapshot, PortError>,
    terminations: AtomicUsize,
    events: Arc<Mutex<Vec<String>>>,
    owner_lease: Option<Box<dyn ActiveProfileOwnerLease>>,
    references: Option<Arc<MemoryReferenceStore>>,
}

impl FakeOwner {
    fn with_binding(
        before: CoreLifecycleSnapshot,
        terminal: Result<CoreLifecycleSnapshot, PortError>,
        events: Arc<Mutex<Vec<String>>>,
        owner_lease: Box<dyn ActiveProfileOwnerLease>,
        references: Arc<MemoryReferenceStore>,
    ) -> Self {
        Self {
            before,
            terminal,
            terminations: AtomicUsize::new(0),
            events,
            owner_lease: Some(owner_lease),
            references: Some(references),
        }
    }
}

#[async_trait]
impl CoreLifecyclePort for FakeOwner {
    fn core_identity(&self) -> Option<CoreIdentity> {
        self.before.identity.clone()
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.before.lifecycle.clone()
    }

    fn generation(&self) -> u64 {
        self.before.generation
    }

    fn session_token(&self) -> Option<SessionToken> {
        self.before.session_token
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        self.before.clone()
    }

    async fn start(&self) -> Result<u64, PortError> {
        Err(unsupported_lifecycle())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(unsupported_lifecycle())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Err(unsupported_lifecycle())
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Err(unsupported_lifecycle())
    }
}

#[async_trait]
impl CoreOwnerTerminationPort for FakeOwner {
    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.owner_lease
            .as_ref()
            .map(
                |lease| ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: lease.identity().clone(),
                    generation: lease.generation(),
                },
            )
            .ok_or_else(unsupported_lifecycle)
    }

    async fn terminate_owner(&self) -> Result<CoreLifecycleSnapshot, PortError> {
        self.terminations.fetch_add(1, Ordering::SeqCst);
        self.events
            .lock()
            .expect("event lock")
            .push("old-owner-close".into());
        let terminal = self.terminal.clone()?;
        if let Some(references) = &self.references {
            let active = tokio::time::timeout(Duration::from_secs(2), references.read())
                .await
                .map_err(|_| PortError::Failed("fake close blocked on the root lock".into()))??;
            self.events
                .lock()
                .expect("event lock")
                .push("close-root-read".into());
            let revision = active.revision();
            let current_engine = match &active {
                ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
                ActiveProfileReferenceState::Unselected { .. } => {
                    return Err(PortError::Failed(
                        "fake close observed an unselected active reference".into(),
                    ));
                }
            };
            let ordinary_cas = references
                .compare_and_set(
                    revision,
                    ActiveProfileReferenceTarget::selected(
                        current_engine,
                        ProfileId::new("ordinary-cas-during-switch").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await;
            if !matches!(ordinary_cas, Err(PortError::Rejected(_))) {
                return Err(PortError::Failed(
                    "ordinary active-reference CAS was admitted during switch".into(),
                ));
            }
            self.events
                .lock()
                .expect("event lock")
                .push("ordinary-cas-blocked".into());
            if !matches!(
                references.acquire_provider_selection_lease().await,
                Err(PortError::Rejected(_))
            ) {
                return Err(PortError::Failed(
                    "provider selection was admitted during switch".into(),
                ));
            }
            self.events
                .lock()
                .expect("event lock")
                .push("ordinary-provider-blocked".into());
        }
        if let Some(owner_lease) = &self.owner_lease {
            owner_lease.release_after_terminal(&terminal).await?;
        }
        Ok(terminal)
    }
}

fn unsupported_lifecycle() -> PortError {
    PortError::unsupported(Capability::CoreLifecycle, "unused fake lifecycle operation")
}

struct SelectorTestRuntime;

impl ApplicationRuntime for SelectorTestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("selector test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio_sleep(duration))
    }
}

fn engine_id_value(value: &str) -> EngineId {
    EngineId::new(value).unwrap()
}

fn profile_document(engine: &str, profile: &str) -> ProfileDocument {
    ProfileDocument::new(
        ProfileDocumentIdentity::new(
            ProfileId::new(profile).unwrap(),
            engine_id_value(engine),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new("local-test").unwrap(),
            ),
            ProfileDocumentRevision::new("doc-r1").unwrap(),
            format_id(engine),
            ProfileSchemaVersion::new(1).unwrap(),
        ),
        b"test-profile".to_vec(),
    )
}

fn format_id(engine: &str) -> ProfileFormatId {
    ProfileFormatId::new(match engine {
        "mihomo" => "mihomo-yaml",
        "sing-box" => "sing-box-json",
        other => other,
    })
    .unwrap()
}

fn reference(engine: &str, profile: &str, revision: u64) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        engine_id_value(engine),
        ProfileId::new(profile).unwrap(),
        ActiveProfileReferenceRevision::new(revision),
    ))
}

fn selector(
    reference_store: Arc<MemoryReferenceStore>,
    stores: Vec<Arc<dyn ProfileStore>>,
    providers: Vec<Arc<dyn EngineBackendProvider>>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
) -> ActiveProfileBackendSelector {
    let factory = Arc::new(
        crate::engine_backend_factory::EngineBackendFactoryRegistry::new(providers).unwrap(),
    );
    let reference_store: Arc<dyn ActiveProfileReferenceStore> = reference_store;
    let factory: Arc<dyn infiltrator_ports::engine_backend_factory::EngineBackendFactory> = factory;
    ActiveProfileBackendSelector::new(reference_store, stores, factory, capabilities)
}

async fn load_prepared_profile_document(store: &dyn ProfileStore) -> ProfileDocument {
    store
        .load_workspace(&ProfileId::new("primary").unwrap())
        .await
        .unwrap()
        .document
}

fn switch_failure(
    result: Result<super::SelectedActiveProfileBackend, ActiveProfileBackendSwitchError>,
    expected_phase: ActiveProfileBackendSwitchFailurePhase,
) -> PortError {
    match result {
        Err(error) => {
            assert_eq!(error.phase(), expected_phase);
            error.into_failure()
        }
        Ok(_) => panic!("owner switch unexpectedly produced a candidate"),
    }
}

fn supported_capabilities() -> Arc<dyn CoreCapabilityProvider> {
    Arc::new(FakeCapabilities {
        state: CapabilityFixtureState::Available,
    })
}

fn terminal_owner_snapshot(before: &CoreLifecycleSnapshot) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: before.identity.clone(),
        lifecycle: CoreLifecycle::Stopped,
        generation: before.generation,
        session_token: None,
        revision: before.revision + 1,
        failure: None,
    }
}

#[tokio::test]
async fn retry_after_owner_close_selects_the_typed_target_when_no_switch_journal_remains() {
    let refs = Arc::new(MemoryReferenceStore::new(reference("mihomo", "main", 4)));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "recovered-instance", events);
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let previous = running_owner_snapshot("mihomo", "closed-old-instance", 5);
    let terminal = terminal_owner_snapshot(&previous);

    let selected = selector
        .resume_owner_switch_for_profile(target.clone(), &prepared, &terminal)
        .await
        .expect("closed prior owner retries through unowned target selection");

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(5),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::NoPriorOwner
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    drop(selected);

    let selected = selector
        .resume_owner_switch_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared,
            &terminal,
        )
        .await
        .expect("retry a durable BindingNew reservation");
    assert_eq!(counts.created.load(Ordering::SeqCst), 2);
    let committed = selected
        .abort_before_core_application()
        .await
        .expect("release recovered startup candidate");
    assert!(matches!(
        committed,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == engine_id_value("sing-box")
                && reference.profile_id == ProfileId::new("primary").unwrap()
    ));
}

#[tokio::test]
async fn retry_resumes_closing_old_and_binding_new_owner_journal_stages() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 4),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "recovered-journal-instance",
        Arc::clone(&events),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "old-journal-owner", 5);
    let expected_terminal = terminal_owner_snapshot(&before);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(expected_terminal),
        Arc::clone(&events),
    )
    .await;
    let identity = before.identity.clone().expect("old owner identity");
    let switch_lease = refs
        .begin_owner_switch(
            ActiveProfileReferenceRevision::new(4),
            identity,
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    let terminal = owner.terminate_owner().await.unwrap();
    drop(switch_lease);

    let selected = selector
        .resume_owner_switch_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared,
            &terminal,
        )
        .await
        .expect("recover the persisted ClosingOld phase and resume");

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(5),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(terminal.generation),
        }
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    selected
        .abort_before_core_application()
        .await
        .expect("release recovered candidate before Core actor construction");
    assert_eq!(*refs.transition.lock().expect("switch transition"), None);
}

#[tokio::test]
async fn retry_resumes_a_reference_changed_owner_journal_with_the_committed_selection() {
    let previous = reference("mihomo", "main", 4);
    let selection = reference("sing-box", "primary", 5);
    let old_identity = CoreIdentity::new(
        engine_id_value("mihomo"),
        CoreInstanceId::new("old-reference-changed-owner").unwrap(),
        None,
    );
    let refs = Arc::new(MemoryReferenceStore::new(selection.clone()));
    *refs.transition.lock().expect("switch transition") =
        Some(ActiveProfileOwnerSwitchState::ReferenceChanged {
            previous_reference: previous,
            selection: selection.clone(),
            owner_identity: old_identity.clone(),
            terminal_generation: CoreGeneration::new(5),
        });
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "reference-changed-retry-instance",
        Arc::new(Mutex::new(Vec::new())),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let terminal = CoreLifecycleSnapshot {
        identity: Some(old_identity.clone()),
        lifecycle: CoreLifecycle::Stopped,
        generation: 5,
        session_token: None,
        revision: 6,
        failure: None,
    };

    let selected = selector
        .resume_owner_switch_for_profile(target, &prepared, &terminal)
        .await
        .expect("resume the committed target and reserve its owner");

    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: old_identity,
            generation: CoreGeneration::new(5),
        }
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    selected
        .abort_before_core_application()
        .await
        .expect("release recovered candidate before actor construction");
}

#[tokio::test]
async fn retry_rejects_a_binding_new_candidate_without_exact_terminal_evidence() {
    let selection = reference("sing-box", "primary", 5);
    let refs = Arc::new(MemoryReferenceStore::new(selection.clone()));
    *refs.recovery_fixture.lock().expect("recovery fixture") =
        MemoryOwnerSwitchRecoveryFixture::CandidateTerminalEvidenceRequired {
            selection: selection.clone(),
            engine_id: engine_id_value("sing-box"),
        };
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "candidate-evidence-instance",
        Arc::new(Mutex::new(Vec::new())),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let terminal = CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            engine_id_value("mihomo"),
            CoreInstanceId::new("old-candidate-owner").unwrap(),
            None,
        )),
        lifecycle: CoreLifecycle::Stopped,
        generation: 5,
        session_token: None,
        revision: 6,
        failure: None,
    };

    let failure = match selector
        .resume_owner_switch_for_profile(target, &prepared, &terminal)
        .await
    {
        Err(failure) => failure,
        Ok(_) => panic!("candidate recovery must require its own terminal evidence"),
    };

    assert!(matches!(failure, PortError::Rejected(_)));
    assert_eq!(refs.read().await.unwrap(), selection);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

fn running_owner_snapshot(engine: &str, instance: &str, generation: u64) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            engine_id_value(engine),
            CoreInstanceId::new(instance).unwrap(),
            Some(CoreVersion::new("old-version").unwrap()),
        )),
        lifecycle: CoreLifecycle::Running,
        generation,
        session_token: Some(SessionToken::new(99)),
        revision: 4,
        failure: None,
    }
}

#[tokio::test]
async fn deleted_target_after_preflight_is_revalidated_and_cancelled_before_old_owner_close() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let source_reference = reference("mihomo", "main", 7);
    let target_deleted = Arc::new(AtomicBool::new(false));
    let refs = Arc::new(MemoryReferenceStore::with_target_deletion_on_begin(
        source_reference.clone(),
        Arc::clone(&events),
        Arc::clone(&target_deleted),
    ));
    let profile_store = Arc::new(
        FakeProfileStore::new("sing-box", "sing-box", "primary")
            .with_deletion_flag(Arc::clone(&target_deleted)),
    );
    let store: Arc<dyn ProfileStore> = profile_store.clone();
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(profile_store.workspace_reads.load(Ordering::SeqCst), 3);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), source_reference);
    assert_eq!(
        *refs.owner.lock().expect("owner state"),
        FakeOwnerAdmission::Bound {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(0),
        }
    );
    assert_eq!(*refs.transition.lock().expect("switch state"), None);
    assert_eq!(
        *events.lock().expect("event lock"),
        vec![
            "target-deleted-before-reservation",
            "switch-reserved",
            "switch-cancelled",
        ]
    );
}

async fn bound_fake_owner(
    references: Arc<MemoryReferenceStore>,
    before: CoreLifecycleSnapshot,
    terminal: Result<CoreLifecycleSnapshot, PortError>,
    events: Arc<Mutex<Vec<String>>>,
) -> Arc<FakeOwner> {
    let identity = before.identity.clone().expect("test owner identity");
    let lease = references
        .acquire_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(identity, CoreGeneration::new(0))
        .await
        .unwrap();
    Arc::new(FakeOwner::with_binding(
        before, terminal, events, lease, references,
    ))
}

#[tokio::test]
async fn active_reference_compare_and_set_rejects_a_stale_revision_without_write() {
    let store = MemoryReferenceStore::new(reference("mihomo", "main", 4));
    let before = store.read().await.unwrap();

    let error = store
        .compare_and_set(
            ActiveProfileReferenceRevision::new(3),
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(store.read().await.unwrap(), before);
}

#[tokio::test]
async fn selected_candidate_holds_revision_lease_against_concurrent_switches() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 2,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (sing_box, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();
    let write_store = Arc::clone(&refs);
    let write = tokio::spawn(async move {
        write_store
            .compare_and_set(
                ActiveProfileReferenceRevision::new(2),
                ActiveProfileReferenceTarget::selected(
                    engine_id_value("mihomo"),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(10), async {
            let _ = &write;
            tokio::task::yield_now().await;
            assert!(!write.is_finished());
        })
        .await
        .is_ok()
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    drop(selected);
    let updated = write.await.unwrap().unwrap();
    assert_eq!(updated.revision(), ActiveProfileReferenceRevision::new(3));
}

#[tokio::test]
async fn profile_document_engine_and_id_must_match_the_global_reference() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "mihomo", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );

    let result = selector.select_backend(&PreviousCoreOwner::Unowned).await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn reference_selects_exactly_one_registered_provider_and_never_reads_namespace_current() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 7,
    )));
    let store = Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let store_as_port: Arc<dyn ProfileStore> = store.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mihomo, mihomo_counts) =
        FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let (sing_box, sing_box_counts) =
        FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store_as_port],
        vec![Arc::new(mihomo), Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();

    assert_eq!(selected.reference.engine_id, engine_id_value("sing-box"));
    assert_eq!(
        selected.reference.profile_id,
        ProfileId::new("primary").unwrap()
    );
    assert_eq!(
        selected.reference.revision,
        ActiveProfileReferenceRevision::new(7)
    );
    assert_eq!(
        selected.document.identity.engine_id,
        engine_id_value("sing-box")
    );
    assert_eq!(selected.core_identity.engine, engine_id_value("sing-box"));
    assert_eq!(selected.core_identity.instance.as_str(), "singbox-instance");
    assert_eq!(selected.initial_generation, CoreGeneration::new(0));
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::NoPriorOwner
    );
    assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(sing_box_counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(store.current_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cross_engine_switch_uses_stable_binding_generation_after_lifecycle_advances() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 7),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    assert_eq!(owner.generation(), 12);
    assert_eq!(
        owner
            .active_profile_reference_write_authorization()
            .unwrap(),
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: before.identity.clone().unwrap(),
            generation: CoreGeneration::new(0),
        }
    );
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(8),
        )
    );
    assert_eq!(selected.initial_generation, CoreGeneration::new(0));
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(12),
        }
    );
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    selected
        .install_owner_binding_for_application(&application)
        .await
        .unwrap();
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 8)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "switch-reserved",
            "old-owner-close",
            "close-root-read",
            "ordinary-cas-blocked",
            "ordinary-provider-blocked",
            "old-owner-released",
            "old-owner-confirmed",
            "reference-cas",
            "new-owner-reserved",
            "provider:sing-box",
            "new-owner-bound",
        ]
    );
    application.close().await.unwrap();
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn same_engine_profile_replacement_uses_the_same_durable_owner_transaction() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("sing-box", "previous", 7),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) =
        FakeProvider::new("sing-box", "singbox-next-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("sing-box", "singbox-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(8),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(12),
        }
    );
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    selected
        .install_owner_binding_for_application(&application)
        .await
        .unwrap();
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 8)
    );
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "switch-reserved",
            "old-owner-close",
            "close-root-read",
            "ordinary-cas-blocked",
            "ordinary-provider-blocked",
            "old-owner-released",
            "old-owner-confirmed",
            "reference-cas",
            "new-owner-reserved",
            "provider:sing-box",
            "new-owner-bound",
        ]
    );
    application.close().await.unwrap();
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn provider_prepared_from_another_document_fails_before_old_owner_close() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let mut prepared_document = load_prepared_profile_document(store.as_ref()).await;
    prepared_document.raw_bytes.push(b' ');
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mismatching_constructed_application_is_closed_then_explicitly_discarded() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();
    let wrong_identity = CoreIdentity::new(
        engine_id_value("sing-box"),
        CoreInstanceId::new("wrong-instance").unwrap(),
        Some(CoreVersion::new("test-version").unwrap()),
    );
    let application = CoreApplication::new(
        Arc::new(FakeProcess {
            identity: wrong_identity.clone(),
            starts: Arc::clone(&counts),
        }),
        Arc::new(FakeReadiness),
        Arc::new(SelectorTestRuntime),
    );

    let result = selected
        .into_owner_binding_for_application(&application)
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    let terminal = application.snapshot().lifecycle_snapshot();
    assert_eq!(terminal.identity, Some(wrong_identity));
    assert_eq!(terminal.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(terminal.session_token, None);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-discarded")
    );
}

#[tokio::test]
async fn failed_owner_install_releases_only_after_exact_application_terminal() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();
    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    application.close().await.unwrap();

    let result = selected
        .install_owner_binding_for_application(&application)
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    let terminal = application.snapshot().lifecycle_snapshot();
    assert_eq!(
        terminal.identity.as_ref().unwrap().engine,
        engine_id_value("sing-box")
    );
    assert_eq!(terminal.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(terminal.session_token, None);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn legacy_selector_cannot_close_an_owner_without_the_switch_transaction() {
    let refs = Arc::new(MemoryReferenceStore::new(reference("mihomo", "main", 1)));
    let store: Arc<dyn ProfileStore> = Arc::new(FakeProfileStore::new("mihomo", "mihomo", "main"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        events,
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .select_backend(&PreviousCoreOwner::Owned(owner_as_port))
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn foreign_target_document_fails_before_close_or_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "mihomo", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_provider_or_target_capability_fails_before_close_or_dispatch() {
    for (capability_state, target_provider_registered) in [
        (CapabilityFixtureState::Available, false),
        (CapabilityFixtureState::TargetUnavailable, true),
        (CapabilityFixtureState::StartUnsupported, true),
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let refs = Arc::new(MemoryReferenceStore::with_events(
            reference("mihomo", "main", 1),
            Arc::clone(&events),
        ));
        let store: Arc<dyn ProfileStore> =
            Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
        let (mihomo, mihomo_counts) =
            FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
        let mut providers: Vec<Arc<dyn EngineBackendProvider>> = vec![Arc::new(mihomo)];
        let mut sing_box_counts = None;
        if target_provider_registered {
            let (sing_box, counts) =
                FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
            sing_box_counts = Some(counts);
            providers.push(Arc::new(sing_box));
        }
        let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
        let selector = selector(
            Arc::clone(&refs),
            vec![store],
            providers,
            Arc::new(FakeCapabilities {
                state: capability_state,
            }),
        );
        let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
        let owner = bound_fake_owner(
            Arc::clone(&refs),
            before.clone(),
            Ok(terminal_owner_snapshot(&before)),
            Arc::clone(&events),
        )
        .await;
        let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

        let result = selector
            .replace_owner_for_profile(
                ActiveProfileReferenceTarget::selected(
                    engine_id_value("sing-box"),
                    ProfileId::new("primary").unwrap(),
                ),
                &prepared_target_document,
                &PreviousCoreOwner::Owned(owner_as_port),
            )
            .await;

        assert!(matches!(
            switch_failure(
                result,
                ActiveProfileBackendSwitchFailurePhase::BeforeReservation
            ),
            PortError::Unsupported { .. }
        ));
        assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
        assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
        if let Some(counts) = sing_box_counts {
            assert_eq!(counts.created.load(Ordering::SeqCst), 0);
        }
        assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
        assert_eq!(*refs.transition.lock().unwrap(), None);
        assert!(events.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn stale_revision_at_reservation_fails_before_close_or_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    refs.reject_next_begin_as_stale();
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*events.lock().unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn terminal_receipt_must_match_exact_preclose_generation_before_cas() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let mut wrong_terminal = terminal_owner_snapshot(&before);
    wrong_terminal.generation += 1;
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before,
        Ok(wrong_terminal),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ClosingOld { .. })
    ));
}

#[tokio::test]
async fn close_failure_keeps_closing_reservation_and_never_dispatches_target() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before,
        Err(PortError::Failed("fake old owner failed to close".into())),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ClosingOld { .. })
    ));
    assert!(matches!(
        *refs.owner.lock().unwrap(),
        FakeOwnerAdmission::Bound { .. }
    ));
}

#[tokio::test]
async fn failed_new_owner_reservation_has_zero_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    refs.reject_next_reservation();
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ReferenceChanged { .. })
    ));
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn provider_dispatch_failure_explicitly_aborts_only_pre_application_reservation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider.failing_creation())],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-aborted")
    );
}

#[tokio::test]
async fn provider_identity_preflight_failure_aborts_before_core_application() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider.with_identity_engine("mihomo"))],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-aborted")
    );
}

#[test]
fn registry_registration_probe_does_not_dispatch_a_provider() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("mihomo", "mihomo-instance", events);
    let provider: Arc<dyn EngineBackendProvider> = Arc::new(provider);
    let registry =
        crate::engine_backend_factory::EngineBackendFactoryRegistry::new([provider]).unwrap();
    let factory: &dyn infiltrator_ports::engine_backend_factory::EngineBackendFactory = &registry;

    assert_eq!(
        factory.provider_availability(&engine_id_value("mihomo")),
        EngineBackendProviderAvailability::Registered
    );
    assert!(matches!(
        factory.provider_availability(&engine_id_value("sing-box")),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[path = "active_profile_backend_tests/abort_candidate.rs"]
mod abort_candidate;
