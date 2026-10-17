//! In-memory owner-switch journal used by the facade and by tests.
//!
//! The journal keeps the whole protocol state behind one `std::sync::Mutex`
//! and fences every transition by revision, exact owner generation and the
//! stage the lease was issued for. There is no randomness and no async runtime
//! dependency, so a test can prove the call order deterministically.

use async_trait::async_trait;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot,
    EngineSwitchStage,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_switch::{
    EngineOwnerBindingLease, EngineOwnerLease, EngineOwnerStoppedSwitchLease,
    EngineOwnerSwitchLease, EngineSwitchJournal, EngineSwitchSelectionChangedLease,
};
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex, MutexGuard};

/// A complete in-memory journal with `EngineId` as the selection identity.
#[derive(Clone)]
pub struct MemoryEngineSwitchJournal {
    state: Arc<Mutex<MemorySwitchState>>,
}

impl MemoryEngineSwitchJournal {
    /// An unowned journal whose current selection is `selection`.
    pub fn new(selection: EngineId) -> Self {
        Self {
            state: Arc::new(Mutex::new(MemorySwitchState {
                revision: EngineSwitchRevision::new(0),
                selection,
                owner: EngineOwnerState::Unowned,
                pending_target: None,
                reservation: None,
            })),
        }
    }
}

struct MemorySwitchState {
    revision: EngineSwitchRevision,
    selection: EngineId,
    owner: EngineOwnerState<EngineId>,
    pending_target: Option<PendingTarget>,
    reservation: Option<CandidateReservation>,
}

struct PendingTarget {
    previous: EngineId,
    selection: EngineId,
    owner_identity: CoreIdentity,
    terminal_generation: CoreGeneration,
    engine_id: EngineId,
}

struct CandidateReservation {
    identity: CoreIdentity,
    generation: CoreGeneration,
}

fn lock(
    state: &Arc<Mutex<MemorySwitchState>>,
) -> Result<MutexGuard<'_, MemorySwitchState>, PortError> {
    state
        .lock()
        .map_err(|_| PortError::Failed("engine switch journal lock is poisoned".to_owned()))
}

fn bump(state: &mut MemorySwitchState) -> Result<EngineSwitchRevision, PortError> {
    let next = state
        .revision
        .next()
        .map_err(|error| PortError::Failed(error.to_string()))?;
    state.revision = next;
    Ok(next)
}

fn rejected(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message.into(), false))
}

fn stale_revision(expected: EngineSwitchRevision, observed: EngineSwitchRevision) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        format!(
            "engine switch revision {} is stale; observed {}",
            expected.value(),
            observed.value()
        ),
        true,
    ))
}

fn snapshot_of(state: &MemorySwitchState) -> EngineSwitchSnapshot<EngineId> {
    EngineSwitchSnapshot {
        revision: state.revision,
        selection: state.selection.clone(),
        owner: state.owner.clone(),
    }
}

fn stage_matches(state: &MemorySwitchState, stage: &EngineSwitchStage<EngineId>) -> bool {
    matches!(
        &state.owner,
        EngineOwnerState::Switching { stage: current } if current == stage
    )
}

#[async_trait]
impl EngineSwitchJournal for MemoryEngineSwitchJournal {
    type Selection = EngineId;

    async fn snapshot(&self) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let state = lock(&self.state)?;
        Ok(snapshot_of(&state))
    }

    async fn bind_owner(
        &self,
        expected_revision: EngineSwitchRevision,
        selection: EngineId,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != expected_revision {
            return Err(stale_revision(expected_revision, state.revision));
        }
        if !matches!(state.owner, EngineOwnerState::Unowned) {
            return Err(rejected(
                "an engine owner can only be bound while the journal is unowned",
            ));
        }
        if identity.engine != selection {
            return Err(rejected(
                "bound owner identity must match the selected engine",
            ));
        }
        bump(&mut state)?;
        state.selection = selection;
        state.owner = EngineOwnerState::Bound {
            identity,
            generation,
        };
        state.pending_target = None;
        state.reservation = None;
        Ok(snapshot_of(&state))
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        previous: EngineId,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<EngineId>>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != expected_revision {
            return Err(stale_revision(expected_revision, state.revision));
        }
        match &state.owner {
            EngineOwnerState::Bound {
                identity,
                generation,
            } if identity == &owner_identity
                && *generation == owner_generation
                && state.selection == previous => {}
            EngineOwnerState::Unowned
            | EngineOwnerState::Bound { .. }
            | EngineOwnerState::Switching { .. } => {
                return Err(rejected(
                    "owner switch must start from the exact bound owner and selection",
                ));
            }
        }
        let revision = bump(&mut state)?;
        let stage = EngineSwitchStage::ClosingOld {
            previous,
            owner_identity,
            owner_generation,
        };
        state.owner = EngineOwnerState::Switching {
            stage: stage.clone(),
        };
        state.pending_target = None;
        state.reservation = None;
        Ok(Box::new(MemoryOwnerSwitchLease {
            state: Arc::clone(&self.state),
            expected_revision: revision,
            stage,
        }))
    }

    async fn recover_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
    ) -> Result<EngineSwitchRecovery<EngineId>, PortError> {
        let state = lock(&self.state)?;
        if state.revision != expected_revision {
            return Err(stale_revision(expected_revision, state.revision));
        }
        Ok(match &state.owner {
            EngineOwnerState::Unowned | EngineOwnerState::Bound { .. } => {
                EngineSwitchRecovery::NoPendingSwitch
            }
            EngineOwnerState::Switching { stage } => EngineSwitchRecovery::from(stage),
        })
    }

    async fn recover_old_owner_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<EngineId>>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != expected_revision {
            return Err(stale_revision(expected_revision, state.revision));
        }
        match state.owner.clone() {
            EngineOwnerState::Switching {
                stage:
                    EngineSwitchStage::ClosingOld {
                        previous,
                        owner_identity,
                        owner_generation,
                    },
            } => {
                if terminal_generation < owner_generation {
                    return Err(rejected(
                        "old owner terminal generation predates its bound generation",
                    ));
                }
                let revision = bump(&mut state)?;
                let stage = EngineSwitchStage::OldStopped {
                    previous,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                };
                state.owner = EngineOwnerState::Switching {
                    stage: stage.clone(),
                };
                Ok(Box::new(MemoryStoppedSwitchLease {
                    state: Arc::clone(&self.state),
                    expected_revision: revision,
                    stage,
                }))
            }
            EngineOwnerState::Switching {
                stage:
                    stage @ (EngineSwitchStage::OldStopped {
                        terminal_generation: recorded,
                        ..
                    }
                    | EngineSwitchStage::SelectionChanged {
                        terminal_generation: recorded,
                        ..
                    }),
            } => {
                if recorded != terminal_generation {
                    return Err(rejected(
                        "old owner terminal generation does not match the journal",
                    ));
                }
                Ok(Box::new(MemoryStoppedSwitchLease {
                    state: Arc::clone(&self.state),
                    expected_revision: state.revision,
                    stage,
                }))
            }
            EngineOwnerState::Unowned
            | EngineOwnerState::Bound { .. }
            | EngineOwnerState::Switching { .. } => Err(rejected(
                "no old-owner switch stage is available for terminal recovery",
            )),
        }
    }

    async fn recover_candidate_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != expected_revision {
            return Err(stale_revision(expected_revision, state.revision));
        }
        let selection = match &state.owner {
            EngineOwnerState::Switching {
                stage:
                    EngineSwitchStage::BindingNew {
                        selection,
                        engine_id,
                    },
            } => {
                let reservation = state
                    .reservation
                    .as_ref()
                    .ok_or_else(|| rejected("binding stage has no candidate reservation"))?;
                if &reservation.identity.engine != engine_id
                    || reservation.generation != terminal_generation
                {
                    return Err(rejected(
                        "candidate terminal generation does not match the reservation",
                    ));
                }
                selection.clone()
            }
            _ => {
                return Err(rejected(
                    "no candidate reservation is available for terminal recovery",
                ));
            }
        };
        bump(&mut state)?;
        state.selection = selection;
        state.owner = EngineOwnerState::Unowned;
        state.pending_target = None;
        state.reservation = None;
        Ok(snapshot_of(&state))
    }
}

struct MemoryOwnerSwitchLease {
    state: Arc<Mutex<MemorySwitchState>>,
    expected_revision: EngineSwitchRevision,
    stage: EngineSwitchStage<EngineId>,
}

#[async_trait]
impl EngineOwnerSwitchLease<EngineId> for MemoryOwnerSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<EngineId> {
        &self.stage
    }

    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != self.expected_revision {
            return Err(stale_revision(self.expected_revision, state.revision));
        }
        if !stage_matches(&state, &self.stage) {
            return Err(rejected("engine switch lease is stale"));
        }
        let (previous, owner_identity, owner_generation) = match &self.stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => (previous.clone(), owner_identity.clone(), *owner_generation),
            _ => {
                return Err(rejected(
                    "cancellation is only available from the closing-old stage",
                ));
            }
        };
        bump(&mut state)?;
        state.selection = previous;
        state.owner = EngineOwnerState::Bound {
            identity: owner_identity,
            generation: owner_generation,
        };
        state.pending_target = None;
        state.reservation = None;
        Ok(snapshot_of(&state))
    }

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<EngineId>>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != self.expected_revision {
            return Err(stale_revision(self.expected_revision, state.revision));
        }
        if !stage_matches(&state, &self.stage) {
            return Err(rejected("engine switch lease is stale"));
        }
        let (previous, owner_identity, owner_generation) = match &self.stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => (previous.clone(), owner_identity.clone(), *owner_generation),
            _ => {
                return Err(rejected(
                    "old owner can only be confirmed from the closing-old stage",
                ));
            }
        };
        if terminal_generation < owner_generation {
            return Err(rejected(
                "old owner terminal generation predates its bound generation",
            ));
        }
        let revision = bump(&mut state)?;
        let stage = EngineSwitchStage::OldStopped {
            previous,
            owner_identity,
            owner_generation,
            terminal_generation,
        };
        state.owner = EngineOwnerState::Switching {
            stage: stage.clone(),
        };
        Ok(Box::new(MemoryStoppedSwitchLease {
            state: Arc::clone(&self.state),
            expected_revision: revision,
            stage,
        }))
    }
}

struct MemoryStoppedSwitchLease {
    state: Arc<Mutex<MemorySwitchState>>,
    expected_revision: EngineSwitchRevision,
    stage: EngineSwitchStage<EngineId>,
}

#[async_trait]
impl EngineOwnerStoppedSwitchLease<EngineId> for MemoryStoppedSwitchLease {
    fn stage(&self) -> &EngineSwitchStage<EngineId> {
        &self.stage
    }

    async fn compare_and_set_target(
        self: Box<Self>,
        selection: EngineId,
        engine_id: EngineId,
    ) -> Result<Box<dyn EngineSwitchSelectionChangedLease<EngineId>>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != self.expected_revision {
            return Err(stale_revision(self.expected_revision, state.revision));
        }
        if !stage_matches(&state, &self.stage) {
            return Err(rejected("engine switch lease is stale"));
        }
        match &self.stage {
            EngineSwitchStage::OldStopped {
                previous,
                owner_identity,
                terminal_generation,
                ..
            } => {
                let revision = bump(&mut state)?;
                let stage = EngineSwitchStage::SelectionChanged {
                    previous: previous.clone(),
                    selection: selection.clone(),
                    owner_identity: owner_identity.clone(),
                    terminal_generation: *terminal_generation,
                };
                state.selection = selection.clone();
                state.owner = EngineOwnerState::Switching {
                    stage: stage.clone(),
                };
                state.pending_target = Some(PendingTarget {
                    previous: previous.clone(),
                    selection,
                    owner_identity: owner_identity.clone(),
                    terminal_generation: *terminal_generation,
                    engine_id,
                });
                state.reservation = None;
                Ok(Box::new(MemorySelectionChangedLease {
                    state: Arc::clone(&self.state),
                    expected_revision: revision,
                    stage,
                }))
            }
            EngineSwitchStage::SelectionChanged {
                selection: current_selection,
                ..
            } => {
                let pending = state
                    .pending_target
                    .as_ref()
                    .ok_or_else(|| rejected("selection-changed stage has no target"))?;
                if current_selection != &selection || pending.engine_id != engine_id {
                    return Err(rejected(
                        "selection-changed target cannot be replaced without recovery",
                    ));
                }
                Ok(Box::new(MemorySelectionChangedLease {
                    state: Arc::clone(&self.state),
                    expected_revision: state.revision,
                    stage: self.stage.clone(),
                }))
            }
            EngineSwitchStage::ClosingOld { .. } | EngineSwitchStage::BindingNew { .. } => Err(
                rejected("target compare-and-set requires a stopped old owner"),
            ),
        }
    }
}

struct MemorySelectionChangedLease {
    state: Arc<Mutex<MemorySwitchState>>,
    expected_revision: EngineSwitchRevision,
    stage: EngineSwitchStage<EngineId>,
}

#[async_trait]
impl EngineSwitchSelectionChangedLease<EngineId> for MemorySelectionChangedLease {
    fn stage(&self) -> &EngineSwitchStage<EngineId> {
        &self.stage
    }

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
        candidate_identity: CoreIdentity,
        candidate_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerBindingLease<EngineId>>, PortError> {
        let mut state = lock(&self.state)?;
        if state.revision != self.expected_revision {
            return Err(stale_revision(self.expected_revision, state.revision));
        }
        if !stage_matches(&state, &self.stage) {
            return Err(rejected("engine switch lease is stale"));
        }
        let pending = state
            .pending_target
            .as_ref()
            .ok_or_else(|| rejected("selection-changed stage has no target"))?;
        if pending.engine_id != engine_id {
            return Err(rejected(
                "candidate engine does not match the committed selection",
            ));
        }
        if candidate_identity.engine != engine_id {
            return Err(rejected(
                "candidate identity engine does not match the reservation",
            ));
        }
        if candidate_generation != CoreGeneration::new(0) {
            return Err(rejected("a prepared candidate must not have started"));
        }
        let selection = match &self.stage {
            EngineSwitchStage::SelectionChanged { selection, .. } => selection.clone(),
            _ => {
                return Err(rejected(
                    "new owner can only be reserved after the selection changed",
                ));
            }
        };
        let revision = bump(&mut state)?;
        let stage = EngineSwitchStage::BindingNew {
            selection,
            engine_id,
        };
        state.owner = EngineOwnerState::Switching {
            stage: stage.clone(),
        };
        state.reservation = Some(CandidateReservation {
            identity: candidate_identity,
            generation: candidate_generation,
        });
        Ok(Box::new(MemoryOwnerBindingLease {
            state: Arc::clone(&self.state),
            expected_revision: revision,
            stage,
        }))
    }
}

struct MemoryOwnerBindingLease {
    state: Arc<Mutex<MemorySwitchState>>,
    expected_revision: EngineSwitchRevision,
    stage: EngineSwitchStage<EngineId>,
}

impl MemoryOwnerBindingLease {
    fn verify(&self, state: &MemorySwitchState) -> Result<(), PortError> {
        if state.revision != self.expected_revision {
            return Err(stale_revision(self.expected_revision, state.revision));
        }
        if !stage_matches(state, &self.stage) {
            return Err(rejected("engine switch lease is stale"));
        }
        Ok(())
    }
}

#[async_trait]
impl EngineOwnerBindingLease<EngineId> for MemoryOwnerBindingLease {
    fn stage(&self) -> &EngineSwitchStage<EngineId> {
        &self.stage
    }

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerLease>, PortError> {
        let mut state = lock(&self.state)?;
        self.verify(&state)?;
        let engine_id = match &self.stage {
            EngineSwitchStage::BindingNew { engine_id, .. } => engine_id.clone(),
            _ => {
                return Err(rejected(
                    "finalize is only available from the binding-new stage",
                ));
            }
        };
        if identity.engine != engine_id {
            return Err(rejected(
                "bound owner identity engine does not match the candidate",
            ));
        }
        let reservation = state
            .reservation
            .as_ref()
            .ok_or_else(|| rejected("binding stage has no candidate reservation"))?;
        if !reservation.identity.same_instance(&identity) {
            return Err(rejected(
                "bound owner instance does not match the reserved candidate",
            ));
        }
        if generation < reservation.generation {
            return Err(rejected(
                "bound owner generation predates the candidate reservation",
            ));
        }
        let selection = match &self.stage {
            EngineSwitchStage::BindingNew { selection, .. } => selection.clone(),
            _ => unreachable!("stage was checked above"),
        };
        bump(&mut state)?;
        state.selection = selection;
        state.owner = EngineOwnerState::Bound {
            identity: identity.clone(),
            generation,
        };
        state.pending_target = None;
        state.reservation = None;
        Ok(Box::new(MemoryOwnerLease {
            identity,
            generation,
        }))
    }

    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let mut state = lock(&self.state)?;
        self.verify(&state)?;
        let pending = state
            .pending_target
            .as_ref()
            .ok_or_else(|| rejected("binding stage has no pending target"))?;
        let reservation = state
            .reservation
            .as_ref()
            .ok_or_else(|| rejected("binding stage has no candidate reservation"))?;
        if reservation.generation != terminal_generation {
            return Err(rejected(
                "candidate terminal generation does not match its reservation",
            ));
        }
        let stage = EngineSwitchStage::SelectionChanged {
            previous: pending.previous.clone(),
            selection: pending.selection.clone(),
            owner_identity: pending.owner_identity.clone(),
            terminal_generation: pending.terminal_generation,
        };
        bump(&mut state)?;
        state.owner = EngineOwnerState::Switching { stage };
        state.reservation = None;
        Ok(snapshot_of(&state))
    }

    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<EngineId>, PortError> {
        let mut state = lock(&self.state)?;
        self.verify(&state)?;
        let selection = match &self.stage {
            EngineSwitchStage::BindingNew { selection, .. } => selection.clone(),
            _ => {
                return Err(rejected(
                    "abort is only available from the binding-new stage",
                ));
            }
        };
        bump(&mut state)?;
        state.selection = selection;
        state.owner = EngineOwnerState::Unowned;
        state.pending_target = None;
        state.reservation = None;
        Ok(snapshot_of(&state))
    }
}

struct MemoryOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
}

impl EngineOwnerLease for MemoryOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }
}
