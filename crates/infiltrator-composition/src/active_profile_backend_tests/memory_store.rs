use super::*;
use ActiveProfileReferenceLease;
use lurker_core_ports::engine_switch::{EngineOwnerSwitchLease, EngineSwitchJournal};
use tokio::sync;

pub(crate) struct MemoryReferenceStore {
    pub(crate) state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) delete_target_before_reservation: Option<Arc<AtomicBool>>,
    pub(crate) stale_next_begin: AtomicBool,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
    pub(crate) candidate_still_live: AtomicBool,
}

/// The fake journal keeps only the neutral stage: the persisted wire format is
/// owned and tested by `infiltrator-profile-store`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FakeSwitch {
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
    /// Exact old-owner generation retained by the persisted switch wrapper;
    /// `SelectionChanged` no longer carries it in the neutral stage.
    pub(crate) owner_generation: CoreGeneration,
}

impl FakeSwitch {
    pub(crate) fn closing_old(
        previous_reference: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Self {
        Self {
            stage: EngineSwitchStage::ClosingOld {
                previous: previous_reference,
                owner_identity,
                owner_generation,
            },
            owner_generation,
        }
    }
}

impl MemoryReferenceStore {
    pub(crate) fn new(state: ActiveProfileReferenceState) -> Self {
        Self {
            state: Arc::new(sync::Mutex::new(state)),
            owner: Arc::new(Mutex::new(FakeOwnerAdmission::Unowned)),
            transition: Arc::new(Mutex::new(None)),
            events: None,
            delete_target_before_reservation: None,
            stale_next_begin: AtomicBool::new(false),
            fail_next_reservation: Arc::new(AtomicBool::new(false)),
            candidate_still_live: AtomicBool::new(false),
        }
    }

    pub(crate) fn with_events(
        state: ActiveProfileReferenceState,
        events: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        let mut store = Self::new(state);
        store.events = Some(events);
        store
    }

    pub(crate) fn with_target_deletion_on_begin(
        state: ActiveProfileReferenceState,
        events: Arc<Mutex<Vec<String>>>,
        deleted: Arc<AtomicBool>,
    ) -> Self {
        let mut store = Self::with_events(state, events);
        store.delete_target_before_reservation = Some(deleted);
        store
    }

    pub(crate) fn reject_next_begin_as_stale(&self) {
        self.stale_next_begin.store(true, Ordering::SeqCst);
    }

    pub(crate) fn reject_next_reservation(&self) {
        self.fail_next_reservation.store(true, Ordering::SeqCst);
    }

    pub(crate) fn reject_candidate_terminal(&self) {
        self.candidate_still_live.store(true, Ordering::SeqCst);
    }

    pub(crate) fn record(&self, event: &str) {
        record(&self.events, event);
    }

    pub(crate) fn journal_revision(state: &ActiveProfileReferenceState) -> EngineSwitchRevision {
        EngineSwitchRevision::new(state.revision().value())
    }
}

pub(crate) fn record(events: &Option<Arc<Mutex<Vec<String>>>>, event: &str) {
    if let Some(events) = events {
        events.lock().expect("event lock").push(event.into());
    }
}

pub(crate) fn fake_snapshot(
    state: &ActiveProfileReferenceState,
    owner: &FakeOwnerAdmission,
    transition: &Option<FakeSwitch>,
) -> EngineSwitchSnapshot<ActiveProfileReferenceState> {
    let owner_state = match transition {
        Some(switch) => EngineOwnerState::Switching {
            stage: switch.stage.clone(),
        },
        None => match owner {
            FakeOwnerAdmission::Bound {
                identity,
                generation,
            } => EngineOwnerState::Bound {
                identity: identity.clone(),
                generation: *generation,
            },
            FakeOwnerAdmission::Unowned | FakeOwnerAdmission::Reserved { .. } => {
                EngineOwnerState::Unowned
            }
        },
    };
    EngineSwitchSnapshot {
        revision: MemoryReferenceStore::journal_revision(state),
        selection: state.clone(),
        owner: owner_state,
    }
}

fn stale_revision_error() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        "fake owner-switch revision is stale",
        true,
    ))
}

#[async_trait]
impl ActiveProfileReferenceStore for MemoryReferenceStore {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(self.state.lock().await.clone())
    }

    async fn adopt_finalized_owner(
        &self,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        if *self.owner.lock().expect("owner state")
            != (FakeOwnerAdmission::Bound {
                identity: identity.clone(),
                generation,
            })
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake finalized owner binding is not the bound owner",
                true,
            )));
        }
        Ok(Box::new(MemoryOwnerLease {
            identity,
            generation,
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
        }))
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
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
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
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
}

#[async_trait]
impl EngineSwitchJournal for MemoryReferenceStore {
    type Selection = ActiveProfileReferenceState;

    async fn snapshot(
        &self,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let state = self.state.lock().await;
        let transition = self.transition.lock().expect("switch state").clone();
        let owner = self.owner.lock().expect("owner state").clone();
        Ok(fake_snapshot(&state, &owner, &transition))
    }

    async fn bind_owner(
        &self,
        expected_revision: EngineSwitchRevision,
        selection: ActiveProfileReferenceState,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let mut state = self.state.lock().await;
        if MemoryReferenceStore::journal_revision(&state) != expected_revision {
            return Err(stale_revision_error());
        }
        if self.transition.lock().expect("switch state").is_some() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner switch is in progress",
                true,
            )));
        }
        let selected_engine = match &selection {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "fake owner binding requires a selected reference",
                    false,
                )));
            }
        };
        if selected_engine != identity.engine
            || *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner binding requires the unowned selected engine",
                true,
            )));
        }
        *state = selection;
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Bound {
            identity: identity.clone(),
            generation,
        };
        Ok(fake_snapshot(
            &state,
            &self.owner.lock().expect("owner state").clone(),
            &None,
        ))
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        previous: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<ActiveProfileReferenceState>>, PortError> {
        if self.stale_next_begin.swap(false, Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch revision became stale during preflight",
                true,
            )));
        }
        let current_state = self.state.lock().await;
        if *current_state != previous
            || MemoryReferenceStore::journal_revision(&current_state) != expected_revision
        {
            return Err(stale_revision_error());
        }
        let current_reference = match &*current_state {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake owner switch requires a selected current reference",
                    true,
                )));
            }
        };
        if current_reference.engine_id != owner_identity.engine {
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
            } if identity.same_instance(&owner_identity) && owner_generation == generation => {
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
        if let Some(deleted) = &self.delete_target_before_reservation {
            deleted.store(true, Ordering::SeqCst);
            self.record("target-deleted-before-reservation");
        }
        let switch =
            FakeSwitch::closing_old(previous.clone(), owner_identity.clone(), owner_generation);
        *self.transition.lock().expect("switch state") = Some(switch.clone());
        drop(current_state);
        self.record("switch-reserved");
        Ok(Box::new(MemoryOwnerSwitchLease {
            stage: switch.stage,
            previous_reference: previous,
            identity: owner_identity,
            generation: owner_generation,
            bound_owner_generation,
            root_state: Arc::clone(&self.state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
        }))
    }

    async fn recover_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
    ) -> Result<EngineSwitchRecovery<ActiveProfileReferenceState>, PortError> {
        let state = self.state.lock().await;
        if MemoryReferenceStore::journal_revision(&state) != expected_revision {
            return Err(stale_revision_error());
        }
        match &*self.transition.lock().expect("switch state") {
            None => Ok(EngineSwitchRecovery::NoPendingSwitch),
            Some(switch) => Ok(EngineSwitchRecovery::from(&switch.stage)),
        }
    }

    async fn recover_old_owner_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError>
    {
        let state = self.state.lock().await;
        if MemoryReferenceStore::journal_revision(&state) != expected_revision {
            return Err(stale_revision_error());
        }
        let persisted = self
            .transition
            .lock()
            .expect("switch state")
            .clone()
            .ok_or_else(|| {
                PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake owner switch has no pending reservation",
                    true,
                ))
            })?;
        let wrapper_generation = persisted.owner_generation;
        let stage = match persisted.stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => {
                if terminal_generation < owner_generation {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "fake old-owner terminal generation predates its binding",
                        true,
                    )));
                }
                if *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "fake old owner is still bound after terminal recovery",
                        true,
                    )));
                }
                let stage = EngineSwitchStage::OldStopped {
                    previous,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                };
                *self.transition.lock().expect("switch state") = Some(FakeSwitch {
                    stage: stage.clone(),
                    owner_generation: wrapper_generation,
                });
                stage
            }
            EngineSwitchStage::OldStopped {
                previous,
                owner_identity,
                owner_generation,
                terminal_generation: recorded,
            } => {
                if recorded != terminal_generation {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "fake old-owner terminal generation does not match the journal",
                        true,
                    )));
                }
                let stage = EngineSwitchStage::OldStopped {
                    previous,
                    owner_identity,
                    owner_generation,
                    terminal_generation: recorded,
                };
                *self.transition.lock().expect("switch state") = Some(FakeSwitch {
                    stage: stage.clone(),
                    owner_generation: wrapper_generation,
                });
                stage
            }
            EngineSwitchStage::SelectionChanged {
                previous,
                selection,
                owner_identity,
                terminal_generation: recorded,
            } => {
                if recorded != terminal_generation {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "fake old-owner terminal generation does not match the journal",
                        true,
                    )));
                }
                // Already past the CAS: keep the durable stage so a repeated
                // target is idempotent instead of advancing the revision.
                EngineSwitchStage::SelectionChanged {
                    previous,
                    selection,
                    owner_identity,
                    terminal_generation: recorded,
                }
            }
            EngineSwitchStage::BindingNew { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake binding stage cannot resume an old owner",
                    true,
                )));
            }
        };
        self.stopped_lease(state, stage, wrapper_generation).await
    }

    async fn recover_candidate_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let state = self.state.lock().await;
        if MemoryReferenceStore::journal_revision(&state) != expected_revision {
            return Err(stale_revision_error());
        }
        if self.candidate_still_live.swap(false, Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake replacement Core candidate is still live",
                true,
            )));
        }
        if terminal_generation != CoreGeneration::new(0) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate terminal generation is not never-started",
                true,
            )));
        }
        match &*self.transition.lock().expect("switch state") {
            Some(FakeSwitch {
                stage: EngineSwitchStage::BindingNew { .. },
                ..
            }) => {}
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "fake candidate reservation is not pending",
                    true,
                )));
            }
        }
        *self.transition.lock().expect("switch state") = None;
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Unowned;
        self.record("candidate-terminal-cleared");
        Ok(fake_snapshot(
            &state,
            &self.owner.lock().expect("owner state").clone(),
            &None,
        ))
    }
}

impl MemoryReferenceStore {
    async fn stopped_lease(
        &self,
        state: sync::MutexGuard<'_, ActiveProfileReferenceState>,
        stage: EngineSwitchStage<ActiveProfileReferenceState>,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError>
    {
        self.record("old-owner-confirmed");
        drop(state);
        Ok(Box::new(MemoryOwnerStoppedSwitchLease {
            stage,
            owner_generation,
            root_state: Arc::clone(&self.state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
        }))
    }
}
