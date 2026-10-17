use super::*;
use ActiveProfileReferenceLease;
use tokio::sync;

pub(crate) struct MemoryReferenceStore {
    pub(crate) state: Arc<sync::Mutex<ActiveProfileReferenceState>>,
    pub(crate) owner: Arc<Mutex<FakeOwnerAdmission>>,
    pub(crate) transition: Arc<Mutex<Option<ActiveProfileOwnerSwitchState>>>,
    pub(crate) events: Option<Arc<Mutex<Vec<String>>>>,
    pub(crate) delete_target_before_reservation: Option<Arc<AtomicBool>>,
    pub(crate) stale_next_begin: AtomicBool,
    pub(crate) fail_next_reservation: Arc<AtomicBool>,
    pub(crate) recovery_fixture: Mutex<MemoryOwnerSwitchRecoveryFixture>,
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
            recovery_fixture: Mutex::new(MemoryOwnerSwitchRecoveryFixture::DurableJournal),
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

    pub(crate) fn record(&self, event: &str) {
        record(&self.events, event);
    }
}

pub(crate) fn record(events: &Option<Arc<Mutex<Vec<String>>>>, event: &str) {
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
