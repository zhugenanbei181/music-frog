use super::*;
use ActiveProfileOwnerLease;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_ports::engine_switch::{
    EngineOwnerBindingLease, EngineOwnerLease, EngineOwnerStoppedSwitchLease,
    EngineOwnerSwitchLease, EngineSwitchSelectionChangedLease,
};
use tokio::sync::{self, OwnedMutexGuard};

pub(crate) struct MemoryReferenceLease {
    pub(crate) state: OwnedMutexGuard<ActiveProfileReferenceState>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FakeOwnerAdmission {
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
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
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

pub(crate) struct MemoryOwnerLease {
    pub(crate) identity: CoreIdentity,
    pub(crate) generation: CoreGeneration,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
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
        _context: &CoreProcessStartContext,
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
            Some(FakeSwitch {
                stage:
                    EngineSwitchStage::ClosingOld {
                        owner_identity,
                        owner_generation,
                        ..
                    },
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

#[async_trait]
impl EngineOwnerLease for MemoryOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        <Self as ActiveProfileOwnerLease>::release_after_terminal(self, terminal).await
    }
}

pub(crate) struct MemoryOwnerSwitchLease {
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
    pub(crate) previous_reference: ActiveProfileReferenceState,
    pub(crate) identity: CoreIdentity,
    pub(crate) generation: CoreGeneration,
    pub(crate) bound_owner_generation: CoreGeneration,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
}

#[async_trait]
impl EngineOwnerSwitchLease<ActiveProfileReferenceState> for MemoryOwnerSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let root_state = self.root_state.lock().await;
        let expected_owner = FakeOwnerAdmission::Bound {
            identity: self.identity.clone(),
            generation: self.bound_owner_generation,
        };
        let transition_matches = matches!(
            self.transition.lock().expect("switch state").as_ref(),
            Some(FakeSwitch {
                stage: EngineSwitchStage::ClosingOld { .. },
                ..
            })
        );
        if *root_state != self.previous_reference
            || !transition_matches
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
        Ok(fake_snapshot(
            &root_state,
            &self.owner.lock().expect("owner state").clone(),
            &None,
        ))
    }

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError>
    {
        if terminal_generation < self.generation {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch receipt predates the bound generation",
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
        let stage = EngineSwitchStage::OldStopped {
            previous: self.previous_reference.clone(),
            owner_identity: self.identity.clone(),
            owner_generation: self.generation,
            terminal_generation,
        };
        let mut transition = self.transition.lock().expect("switch state");
        if transition.as_ref().map(|switch| &switch.stage) != Some(&self.stage) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake owner-switch phase changed before old-owner receipt",
                true,
            )));
        }
        *transition = Some(FakeSwitch {
            stage: stage.clone(),
            owner_generation: self.generation,
        });
        drop(transition);
        record(&self.events, "old-owner-confirmed");
        Ok(Box::new(MemoryOwnerStoppedSwitchLease {
            stage,
            owner_generation: self.generation,
            root_state: Arc::clone(&self.root_state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            events: self.events.clone(),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
        }))
    }
}

pub(crate) struct MemoryOwnerStoppedSwitchLease {
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
    pub(crate) owner_generation: CoreGeneration,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
}

impl MemoryOwnerStoppedSwitchLease {
    fn previous_reference(&self) -> &ActiveProfileReferenceState {
        match &self.stage {
            EngineSwitchStage::OldStopped { previous, .. } => previous,
            _ => unreachable!("stopped lease always carries an old-stopped stage"),
        }
    }

    fn terminal_generation(&self) -> CoreGeneration {
        match &self.stage {
            EngineSwitchStage::OldStopped {
                terminal_generation,
                ..
            } => *terminal_generation,
            _ => unreachable!("stopped lease always carries an old-stopped stage"),
        }
    }
}

#[async_trait]
impl EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState> for MemoryOwnerStoppedSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    ) -> Result<Box<dyn EngineSwitchSelectionChangedLease<ActiveProfileReferenceState>>, PortError>
    {
        if let EngineSwitchStage::SelectionChanged {
            selection: applied, ..
        } = &self.stage
        {
            if applied != &selection {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "fake durable owner switch target differs from the retry request",
                    false,
                )));
            }
            return Ok(Box::new(MemoryReferenceChangedLease {
                stage: self.stage.clone(),
                owner_generation: self.owner_generation,
                root_state: Arc::clone(&self.root_state),
                owner: Arc::clone(&self.owner),
                transition: Arc::clone(&self.transition),
                fail_next_reservation: Arc::clone(&self.fail_next_reservation),
                events: self.events.clone(),
            }));
        }
        let previous_reference = self.previous_reference().clone();
        let terminal_generation = self.terminal_generation();
        let owner_identity = match &self.stage {
            EngineSwitchStage::OldStopped { owner_identity, .. } => owner_identity.clone(),
            _ => unreachable!("stopped lease always carries an old-stopped stage"),
        };
        let selected_engine = match &selection {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "fake owner switch only accepts a selected profile target",
                    false,
                )));
            }
        };
        let Some(expected_revision) = previous_reference.revision().next() else {
            return Err(PortError::Failed(
                "fake active-profile revision exhausted".into(),
            ));
        };
        if selected_engine != engine_id || selection.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake owner switch selection must advance exactly one revision for its engine",
                false,
            )));
        }
        let transition_matches = self
            .transition
            .lock()
            .expect("switch state")
            .as_ref()
            .map(|switch| &switch.stage)
            == Some(&self.stage);
        if !transition_matches
            || *self.owner.lock().expect("owner state") != FakeOwnerAdmission::Unowned
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake switch state changed before target CAS",
                true,
            )));
        }
        let mut current = self.root_state.lock().await;
        if current.revision() != previous_reference.revision() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake active-profile revision is stale",
                true,
            )));
        }
        *current = selection.clone();
        let stage = EngineSwitchStage::SelectionChanged {
            previous: previous_reference.clone(),
            selection: selection.clone(),
            owner_identity,
            terminal_generation,
        };
        let owner_generation = self.owner_generation;
        *self.transition.lock().expect("switch state") = Some(FakeSwitch {
            stage: stage.clone(),
            owner_generation,
        });
        drop(current);
        record(&self.events, "reference-cas");
        Ok(Box::new(MemoryReferenceChangedLease {
            stage,
            owner_generation: self.owner_generation,
            root_state: Arc::clone(&self.root_state),
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            fail_next_reservation: Arc::clone(&self.fail_next_reservation),
            events: self.events.clone(),
        }))
    }
}

pub(crate) struct MemoryReferenceChangedLease {
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
    pub(crate) owner_generation: CoreGeneration,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
}

impl MemoryReferenceChangedLease {
    fn selection(&self) -> &ActiveProfileReferenceState {
        match &self.stage {
            EngineSwitchStage::SelectionChanged { selection, .. } => selection,
            _ => unreachable!("selection-changed lease always carries its stage"),
        }
    }
}

#[async_trait]
impl EngineSwitchSelectionChangedLease<ActiveProfileReferenceState>
    for MemoryReferenceChangedLease
{
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
        candidate_identity: CoreIdentity,
        candidate_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerBindingLease<ActiveProfileReferenceState>>, PortError> {
        if self.fail_next_reservation.swap(false, Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake new-owner admission reservation failed",
                true,
            )));
        }
        let selected_engine = match self.selection() {
            ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "fake owner reservation requires a selected reference",
                    false,
                )));
            }
        };
        if selected_engine != engine_id
            || candidate_identity.engine != engine_id
            || candidate_generation != CoreGeneration::new(0)
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake new-owner reservation must match the committed engine and an unstarted candidate",
                false,
            )));
        }
        let transition_matches = self
            .transition
            .lock()
            .expect("switch state")
            .as_ref()
            .map(|switch| &switch.stage)
            == Some(&self.stage);
        let owner_is_unowned =
            *self.owner.lock().expect("owner state") == FakeOwnerAdmission::Unowned;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_is_unowned || *root_state != self.selection().clone() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake switch state changed before owner reservation",
                true,
            )));
        }
        drop(root_state);
        let (previous_reference, owner_identity, terminal_generation) = match &self.stage {
            EngineSwitchStage::SelectionChanged {
                previous,
                owner_identity,
                terminal_generation,
                ..
            } => (
                previous.clone(),
                owner_identity.clone(),
                *terminal_generation,
            ),
            _ => unreachable!("selection-changed lease always carries its stage"),
        };
        let stage = EngineSwitchStage::BindingNew {
            selection: self.selection().clone(),
            engine_id: engine_id.clone(),
        };
        let owner_generation = self.owner_generation;
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Reserved {
            engine_id: engine_id.clone(),
        };
        *self.transition.lock().expect("switch state") = Some(FakeSwitch {
            stage: stage.clone(),
            owner_generation,
        });
        record(&self.events, "new-owner-reserved");
        Ok(Box::new(MemoryOwnerBindingLease {
            stage,
            engine_id,
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
            owner: Arc::clone(&self.owner),
            transition: Arc::clone(&self.transition),
            root_state: Arc::clone(&self.root_state),
            events: self.events.clone(),
        }))
    }
}

pub(crate) struct MemoryOwnerBindingLease {
    pub(crate) stage: EngineSwitchStage<ActiveProfileReferenceState>,
    pub(crate) engine_id: EngineId,
    pub(crate) previous_reference: ActiveProfileReferenceState,
    pub(crate) owner_identity: CoreIdentity,
    pub(crate) owner_generation: CoreGeneration,
    pub(crate) terminal_generation: CoreGeneration,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<FakeSwitch>>>,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
}

impl MemoryOwnerBindingLease {
    fn selection(&self) -> &ActiveProfileReferenceState {
        match &self.stage {
            EngineSwitchStage::BindingNew { selection, .. } => selection,
            _ => unreachable!("binding lease always carries its stage"),
        }
    }
}

#[async_trait]
impl EngineOwnerBindingLease<ActiveProfileReferenceState> for MemoryOwnerBindingLease {
    fn stage(&self) -> &EngineSwitchStage<ActiveProfileReferenceState> {
        &self.stage
    }

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerLease>, PortError> {
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
        let transition_matches = self
            .transition
            .lock()
            .expect("switch state")
            .as_ref()
            .map(|switch| &switch.stage)
            == Some(&self.stage);
        if !transition_matches || *self.owner.lock().expect("owner state") != expected_owner {
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
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        if terminal_generation != CoreGeneration::new(0) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate discard requires a never-started generation",
                true,
            )));
        }
        let expected_owner = FakeOwnerAdmission::Reserved {
            engine_id: self.engine_id.clone(),
        };
        let transition_matches = self
            .transition
            .lock()
            .expect("switch state")
            .as_ref()
            .map(|switch| &switch.stage)
            == Some(&self.stage);
        let owner_matches = *self.owner.lock().expect("owner state") == expected_owner;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_matches || *root_state != *self.selection() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate reservation changed before discard",
                true,
            )));
        }
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Unowned;
        // The live-lease discard rewinds to `SelectionChanged` so the caller
        // can retry the reservation with the committed selection and the old
        // owner's terminal proof. `abort_before_core_application` and
        // `recover_candidate_after_terminal` instead land at `Unowned`.
        let transition = FakeSwitch {
            stage: EngineSwitchStage::SelectionChanged {
                previous: self.previous_reference.clone(),
                selection: self.selection().clone(),
                owner_identity: self.owner_identity.clone(),
                terminal_generation: self.terminal_generation,
            },
            owner_generation: self.owner_generation,
        };
        *self.transition.lock().expect("switch state") = Some(transition.clone());
        record(&self.events, "candidate-discarded");
        Ok(fake_snapshot(
            &root_state,
            &self.owner.lock().expect("owner state").clone(),
            &Some(transition),
        ))
    }

    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let expected_owner = FakeOwnerAdmission::Reserved {
            engine_id: self.engine_id.clone(),
        };
        let transition_matches = self
            .transition
            .lock()
            .expect("switch state")
            .as_ref()
            .map(|switch| &switch.stage)
            == Some(&self.stage);
        let owner_matches = *self.owner.lock().expect("owner state") == expected_owner;
        let root_state = self.root_state.lock().await;
        if !transition_matches || !owner_matches || *root_state != *self.selection() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake candidate reservation changed before abort",
                true,
            )));
        }
        *self.owner.lock().expect("owner state") = FakeOwnerAdmission::Unowned;
        *self.transition.lock().expect("switch state") = None;
        record(&self.events, "candidate-aborted");
        Ok(fake_snapshot(
            &root_state,
            &self.owner.lock().expect("owner state").clone(),
            &None,
        ))
    }
}
