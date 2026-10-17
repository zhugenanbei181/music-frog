use super::*;
use ActiveProfileOwnerLease;
use lurker_core_contract::core_process::CoreProcessStartContext;
use tokio::sync::{self, OwnedMutexGuard};

#[derive(Clone)]
pub(crate) enum MemoryOwnerSwitchRecoveryFixture {
    DurableJournal,
    CandidateTerminalEvidenceRequired {
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    },
}

pub(crate) struct MemoryReferenceLease {
    pub(crate) state: OwnedMutexGuard<ActiveProfileReferenceState>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
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
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
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

pub(crate) struct MemoryOwnerSwitchLease {
    pub(crate) state: ActiveProfileOwnerSwitchState,
    pub(crate) previous_reference: ActiveProfileReferenceState,
    pub(crate) identity: CoreIdentity,
    pub(crate) generation: CoreGeneration,
    pub(crate) bound_owner_generation: CoreGeneration,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
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

pub(crate) struct MemoryOwnerStoppedSwitchLease {
    pub(crate) state: ActiveProfileOwnerSwitchState,
    pub(crate) previous_reference: ActiveProfileReferenceState,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
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

pub(crate) struct MemoryReferenceChangedLease {
    pub(crate) state: ActiveProfileOwnerSwitchState,
    pub(crate) selection: ActiveProfileReferenceState,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
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

pub(crate) struct MemoryOwnerBindingLease {
    pub(crate) state: ActiveProfileOwnerSwitchState,
    pub(crate) engine_id: EngineId,
    pub(crate) selection: ActiveProfileReferenceState,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    pub(crate) root_state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
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
