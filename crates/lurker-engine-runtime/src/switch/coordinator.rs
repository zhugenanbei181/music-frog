//! Strict owner-switch coordinator.
//!
//! The coordinator is the only place allowed to order a switch. The order is
//! fixed and observable:
//!
//! 1. validate the target;
//! 2. read the snapshot and require an exact bound owner plus a selected
//!    target;
//! 3. dispatch the registry and prepare the candidate **before** any durable
//!    reservation;
//! 4. reserve the switch (`ClosingOld`);
//! 5. terminate the exact old owner and confirm its terminal generation;
//! 6. compare-and-set the new selection (`SelectionChanged`);
//! 7. reserve the new owner admission (`BindingNew`);
//! 8. apply the candidate and finalize the exact bound owner.
//!
//! A failure before step 5 can be cancelled back to the previous binding. A
//! failure from step 5 onwards never pretends the old owner still exists: the
//! result carries `RecoveryRequired`, and the journal keeps the exact stage
//! and terminal evidence needed to resume.

use crate::registry::EngineBackendProviderRegistry;
use crate::switch::error::EngineSwitchError;
use crate::switch::host::EngineSwitchHost;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchRevision,
    EngineSwitchSnapshot, EngineSwitchTarget,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_backend::EngineBackendBundle;
use lurker_core_ports::engine_switch::{EngineOwnerSwitchLease, EngineSwitchJournal};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// One successful pre-termination reservation.
///
/// The caller may either hand it back to [`EngineSwitchCoordinator::cancel_switch`]
/// or continue with [`EngineSwitchCoordinator::finalize_switch`].
pub struct EngineSwitchReservation<B, S>
where
    B: EngineBackendBundle,
    S: Clone + Eq + Send + Sync + 'static,
{
    previous: S,
    owner_identity: CoreIdentity,
    owner_generation: CoreGeneration,
    engine_id: EngineId,
    selection: S,
    candidate_identity: CoreIdentity,
    backend: B,
    lease: Box<dyn EngineOwnerSwitchLease<S>>,
}

impl<B, S> EngineSwitchReservation<B, S>
where
    B: EngineBackendBundle,
    S: Clone + Eq + Send + Sync + 'static,
{
    pub fn previous_selection(&self) -> &S {
        &self.previous
    }

    pub fn selection(&self) -> &S {
        &self.selection
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    pub fn candidate_identity(&self) -> &CoreIdentity {
        &self.candidate_identity
    }
}

/// Engine- and product-neutral switch coordinator.
pub struct EngineSwitchCoordinator<B, J>
where
    B: EngineBackendBundle,
    J: EngineSwitchJournal,
{
    registry: Arc<EngineBackendProviderRegistry<B>>,
    journal: Arc<J>,
    host: Arc<dyn EngineSwitchHost<B, J::Selection>>,
}

impl<B, J> EngineSwitchCoordinator<B, J>
where
    B: EngineBackendBundle,
    J: EngineSwitchJournal,
{
    pub fn new(
        registry: Arc<EngineBackendProviderRegistry<B>>,
        journal: Arc<J>,
        host: Arc<dyn EngineSwitchHost<B, J::Selection>>,
    ) -> Self {
        Self {
            registry,
            journal,
            host,
        }
    }

    pub fn journal(&self) -> &J {
        &self.journal
    }

    pub async fn snapshot(&self) -> Result<EngineSwitchSnapshot<J::Selection>, PortError> {
        self.journal.snapshot().await
    }

    /// Validate, dispatch, prepare and reserve a switch without touching the
    /// previous owner.
    pub async fn reserve_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        target: EngineSwitchTarget<J::Selection>,
    ) -> Result<EngineSwitchReservation<B, J::Selection>, EngineSwitchError<J::Selection>> {
        let (engine_id, selection) = match &target {
            EngineSwitchTarget::Unselected => {
                return Err(self
                    .reject_before_reservation("an engine switch requires a selected target")
                    .await);
            }
            EngineSwitchTarget::Selected {
                engine_id,
                selection,
            } => (engine_id.clone(), selection.clone()),
        };
        if let Err(cause) = self.host.validate_target(&target).await {
            return Err(self.before_reservation(cause).await);
        }
        let snapshot = self.journal.snapshot().await.map_err(|cause| {
            EngineSwitchError::recovery_required(EngineSwitchFailurePhase::BeforeReservation, cause)
        })?;
        if snapshot.revision != expected_revision {
            let cause = stale_revision(expected_revision, snapshot.revision);
            return Err(self.before_reservation(cause).await);
        }
        let (owner_identity, owner_generation) = match &snapshot.owner {
            EngineOwnerState::Bound {
                identity,
                generation,
            } => (identity.clone(), *generation),
            EngineOwnerState::Unowned | EngineOwnerState::Switching { .. } => {
                return Err(self
                    .reject_before_reservation(
                        "an engine switch requires the exact bound previous owner",
                    )
                    .await);
            }
        };
        let previous = snapshot.selection.clone();
        let backend = match self.registry.backend(&engine_id) {
            Ok(backend) => backend,
            Err(cause) => return Err(self.before_reservation(cause).await),
        };
        let candidate_identity = match self
            .host
            .prepare_candidate(&engine_id, &selection, &backend)
            .await
        {
            Ok(identity) => identity,
            Err(cause) => return Err(self.before_reservation(cause).await),
        };
        if candidate_identity.engine != engine_id {
            let cause = PortError::Failed(format!(
                "engine switch candidate for {} reported identity for {}",
                engine_id.as_str(),
                candidate_identity.engine.as_str()
            ));
            let error = self.before_reservation(cause).await;
            let _ = self.host.close_candidate(backend).await;
            return Err(error);
        }
        let lease = match self
            .journal
            .begin_owner_switch(
                expected_revision,
                previous.clone(),
                owner_identity.clone(),
                owner_generation,
            )
            .await
        {
            Ok(lease) => lease,
            Err(cause) => {
                let _ = self.host.close_candidate(backend).await;
                return Err(EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::ReservationPending,
                    cause,
                ));
            }
        };
        Ok(EngineSwitchReservation {
            previous,
            owner_identity,
            owner_generation,
            engine_id,
            selection,
            candidate_identity,
            backend,
            lease,
        })
    }

    /// Cancel a reservation before the previous owner was asked to stop.
    pub async fn cancel_switch(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let EngineSwitchReservation {
            previous,
            owner_identity,
            owner_generation,
            backend,
            lease,
            ..
        } = reservation;
        let snapshot = lease
            .cancel_before_old_owner_close()
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::ReservationPending,
                    cause,
                )
            })?;
        let restored = snapshot.selection == previous
            && snapshot.owner
                == EngineOwnerState::Bound {
                    identity: owner_identity,
                    generation: owner_generation,
                };
        if !restored {
            return Err(EngineSwitchError::recovery_required(
                EngineSwitchFailurePhase::ReservationPending,
                PortError::Failed(
                    "cancelled engine switch did not restore the exact previous binding".to_owned(),
                ),
            ));
        }
        if let Err(cause) = self.host.close_candidate(backend).await {
            return Err(EngineSwitchError::new(
                EngineSwitchFailurePhase::BeforeReservation,
                cause,
                EngineSwitchDisposition::RolledBackToPrevious {
                    selection: snapshot.selection.clone(),
                },
            ));
        }
        Ok(snapshot)
    }

    /// Continue a reservation: terminate the old owner, publish the selection,
    /// reserve and start the candidate, then bind it.
    pub async fn finalize_switch(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let EngineSwitchReservation {
            owner_identity,
            owner_generation,
            engine_id,
            selection,
            candidate_identity,
            backend,
            lease,
            ..
        } = reservation;
        let terminal_generation = self
            .host
            .terminate_old_owner(&owner_identity, owner_generation)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OwnerTerminationUnconfirmed,
                    cause,
                )
            })?;
        let stopped = lease
            .confirm_old_owner_stopped(terminal_generation)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        let changed = stopped
            .compare_and_set_target(selection.clone(), engine_id.clone())
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        let binding = changed
            .reserve_new_owner(
                engine_id.clone(),
                candidate_identity.clone(),
                CoreGeneration::new(0),
            )
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        let (applied_identity, applied_generation) = self
            .host
            .apply_candidate(backend, &selection)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        if applied_identity.engine != engine_id
            || !applied_identity.same_instance(&candidate_identity)
        {
            return Err(EngineSwitchError::recovery_required(
                EngineSwitchFailurePhase::OldOwnerClosed,
                PortError::Failed(format!(
                    "applied engine switch owner for {} did not match the reserved candidate",
                    engine_id.as_str()
                )),
            ));
        }
        binding
            .finalize(applied_identity, applied_generation)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        self.journal.snapshot().await.map_err(|cause| {
            EngineSwitchError::recovery_required(EngineSwitchFailurePhase::OldOwnerClosed, cause)
        })
    }

    /// Reserve and immediately run the whole switch.
    pub async fn switch_and_finalize(
        &self,
        expected_revision: EngineSwitchRevision,
        target: EngineSwitchTarget<J::Selection>,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let reservation = self.reserve_switch(expected_revision, target).await?;
        self.finalize_switch(reservation).await
    }

    async fn before_reservation(&self, cause: PortError) -> EngineSwitchError<J::Selection> {
        match self.journal.snapshot().await {
            Ok(snapshot) => EngineSwitchError::new(
                EngineSwitchFailurePhase::BeforeReservation,
                cause,
                EngineSwitchDisposition::RolledBackToPrevious {
                    selection: snapshot.selection,
                },
            ),
            Err(_) => EngineSwitchError::recovery_required(
                EngineSwitchFailurePhase::BeforeReservation,
                cause,
            ),
        }
    }

    async fn reject_before_reservation(&self, message: &str) -> EngineSwitchError<J::Selection> {
        self.before_reservation(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            message.to_owned(),
            false,
        )))
        .await
    }
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
