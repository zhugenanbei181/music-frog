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
//! 7. reserve the new owner admission (`BindingNew`).
//!
//! [`EngineSwitchCoordinator::reserve_switch`] stops at step 7 and hands the
//! dispatched bundle, the prepared candidate, the durable
//! [`EngineOwnerBindingLease`] and the exact snapshot to the embedding host.
//! The host finishes the switch with
//! [`EngineSwitchCoordinator::finalize_reserved`], releases an unstarted
//! candidate with [`EngineSwitchCoordinator::cancel_reserved`] or
//! [`EngineSwitchCoordinator::discard_reserved`]. Hosts that own application
//! themselves use [`EngineSwitchCoordinator::finalize_switch`], and
//! [`EngineSwitchCoordinator::switch_and_finalize`] is the thin one-shot
//! wrapper over the very same protocol.
//!
//! Dispatch happens before the durable reservation on purpose: a provider that
//! cannot produce the candidate bundle fails while the previous binding is
//! still exactly bound, so a failed dispatch can never leave a changed
//! selection behind.
//!
//! A failure before step 4 leaves the journal untouched and is reported as
//! rolled back to the previous binding. A failure from step 5 onwards never
//! pretends the old owner still exists: the result carries `RecoveryRequired`,
//! and the journal keeps the exact stage and terminal evidence needed to
//! resume.

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
use lurker_core_ports::engine_switch::{
    EngineOwnerBindingLease, EngineOwnerLease, EngineSwitchJournal,
};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// One switch advanced all the way to its durable `BindingNew` admission.
///
/// The value is the exact handoff between the coordinator and the host that
/// owns application: it carries the dispatched bundle the host must apply, the
/// identity the coordinator prepared, the durable binding lease and the
/// readback snapshot of the stopped-old-owner reservation.
pub struct EngineSwitchReservation<B, S>
where
    B: EngineBackendBundle,
    S: Clone + Eq + Send + Sync + 'static,
{
    previous: S,
    previous_owner: CoreIdentity,
    previous_generation: CoreGeneration,
    terminal_generation: CoreGeneration,
    engine_id: EngineId,
    selection: S,
    candidate: CoreIdentity,
    backend: B,
    binding: Box<dyn EngineOwnerBindingLease<S>>,
    snapshot: EngineSwitchSnapshot<S>,
}

impl<B, S> EngineSwitchReservation<B, S>
where
    B: EngineBackendBundle,
    S: Clone + Eq + Send + Sync + 'static,
{
    /// The selection the exact previous owner was bound to.
    pub fn previous_selection(&self) -> &S {
        &self.previous
    }

    /// The committed replacement selection.
    pub fn selection(&self) -> &S {
        &self.selection
    }

    /// The engine of the committed replacement selection.
    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    /// The exact identity the coordinator prepared for the candidate.
    pub fn candidate_identity(&self) -> &CoreIdentity {
        &self.candidate
    }

    /// The exact identity and binding generation of the terminated owner.
    pub fn previous_owner(&self) -> (&CoreIdentity, CoreGeneration) {
        (&self.previous_owner, self.previous_generation)
    }

    /// The exact terminal generation observed for the previous owner.
    pub fn terminal_generation(&self) -> CoreGeneration {
        self.terminal_generation
    }

    /// The dispatched bundle the host applies to construct the candidate owner.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The durable readback at the `BindingNew` admission.
    pub fn snapshot(&self) -> &EngineSwitchSnapshot<S> {
        &self.snapshot
    }
}

/// Engine- and product-neutral switch coordinator.
pub struct EngineSwitchCoordinator<B, J>
where
    B: EngineBackendBundle,
    J: EngineSwitchJournal + ?Sized,
{
    registry: Arc<EngineBackendProviderRegistry<B>>,
    journal: Arc<J>,
    host: Arc<dyn EngineSwitchHost<B, J::Selection>>,
}

impl<B, J> Clone for EngineSwitchCoordinator<B, J>
where
    B: EngineBackendBundle,
    J: EngineSwitchJournal + ?Sized,
{
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            journal: Arc::clone(&self.journal),
            host: Arc::clone(&self.host),
        }
    }
}

impl<B, J> EngineSwitchCoordinator<B, J>
where
    B: EngineBackendBundle,
    J: EngineSwitchJournal + ?Sized,
{
    /// Creates a coordinator over one registry, journal and switch host.
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

    /// Returns the durable journal the coordinator drives.
    pub fn journal(&self) -> &J {
        &self.journal
    }

    /// Reads the current selection and owner state from the journal.
    pub async fn snapshot(&self) -> Result<EngineSwitchSnapshot<J::Selection>, PortError> {
        self.journal.snapshot().await
    }

    /// Validate, dispatch, prepare and run the whole forward switch until the
    /// durable `BindingNew` admission exists.
    ///
    /// The candidate is dispatched and prepared **before** the durable
    /// reservation, so a failed dispatch leaves the previous binding exactly
    /// as it was. On success the previous owner is already terminated and the
    /// caller must finish with [`Self::finalize_reserved`],
    /// [`Self::cancel_reserved`] or [`Self::discard_reserved`]; every later
    /// failure is `RecoveryRequired`.
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
        let (previous_owner, previous_generation) = match &snapshot.owner {
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
        let candidate = match self
            .host
            .prepare_candidate(&engine_id, &selection, &backend)
            .await
        {
            Ok(identity) => identity,
            Err(cause) => return Err(self.before_reservation(cause).await),
        };
        if candidate.engine != engine_id {
            let cause = PortError::Failed(format!(
                "engine switch candidate for {} reported identity for {}",
                engine_id.as_str(),
                candidate.engine.as_str()
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
                previous_owner.clone(),
                previous_generation,
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
        let terminal_generation = self
            .host
            .terminate_old_owner(&previous_owner, previous_generation)
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
            .reserve_new_owner(engine_id.clone(), candidate.clone(), CoreGeneration::new(0))
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        let snapshot = self.journal.snapshot().await.map_err(|cause| {
            EngineSwitchError::recovery_required(EngineSwitchFailurePhase::OldOwnerClosed, cause)
        })?;
        Ok(EngineSwitchReservation {
            previous,
            previous_owner,
            previous_generation,
            terminal_generation,
            engine_id,
            selection,
            candidate,
            backend,
            binding,
            snapshot,
        })
    }

    /// Bind the exact candidate the host actually applied and clear the
    /// switch. The applied identity must be the same instance the coordinator
    /// prepared; a mismatching identity is `RecoveryRequired`.
    pub async fn finalize_reserved(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let EngineSwitchReservation {
            engine_id,
            candidate,
            binding,
            ..
        } = reservation;
        let (snapshot, _lease) = self
            .bind_reserved(&engine_id, &candidate, binding, identity, generation)
            .await?;
        Ok(snapshot)
    }

    /// Bind one exact reserved candidate and read the cleared journal back.
    async fn bind_reserved(
        &self,
        engine_id: &EngineId,
        candidate: &CoreIdentity,
        binding: Box<dyn EngineOwnerBindingLease<J::Selection>>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<
        (
            EngineSwitchSnapshot<J::Selection>,
            Box<dyn EngineOwnerLease>,
        ),
        EngineSwitchError<J::Selection>,
    > {
        if identity.engine != *engine_id || !identity.same_instance(candidate) {
            return Err(EngineSwitchError::recovery_required(
                EngineSwitchFailurePhase::OldOwnerClosed,
                PortError::Failed(format!(
                    "applied engine switch owner for {} did not match the reserved candidate",
                    engine_id.as_str()
                )),
            ));
        }
        let lease = binding
            .finalize(identity, generation)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })?;
        let snapshot = self.journal.snapshot().await.map_err(|cause| {
            EngineSwitchError::recovery_required(EngineSwitchFailurePhase::OldOwnerClosed, cause)
        })?;
        Ok((snapshot, lease))
    }

    /// Release a reserved candidate that never started. The selection stays at
    /// the committed target; the previous owner cannot be restored because it
    /// already reached its exact terminal.
    pub async fn cancel_reserved(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let EngineSwitchReservation { binding, .. } = reservation;
        binding
            .abort_before_core_application()
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })
    }

    /// Release a reserved candidate only after its exact never-started
    /// terminal generation proves it never ran. The journal returns to the
    /// committed `SelectionChanged` stage so a later attempt can reserve again.
    pub async fn discard_reserved(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        let EngineSwitchReservation { binding, .. } = reservation;
        binding
            .discard_unstarted_candidate(terminal_generation)
            .await
            .map_err(|cause| {
                EngineSwitchError::recovery_required(
                    EngineSwitchFailurePhase::OldOwnerClosed,
                    cause,
                )
            })
    }

    /// Finish a reservation for a host that owns application: apply the
    /// candidate through the host, then bind the exact applied identity.
    pub async fn finalize_switch(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
    ) -> Result<EngineSwitchSnapshot<J::Selection>, EngineSwitchError<J::Selection>> {
        self.finalize_switch_with_lease(reservation)
            .await
            .map(|(snapshot, _lease)| snapshot)
    }

    /// Finish a reservation and hand the exact bound owner lease back to the
    /// host. The lease must be retained until the matching terminal; dropping
    /// it leaves the journal bound.
    pub async fn finalize_switch_with_lease(
        &self,
        reservation: EngineSwitchReservation<B, J::Selection>,
    ) -> Result<
        (
            EngineSwitchSnapshot<J::Selection>,
            Box<dyn EngineOwnerLease>,
        ),
        EngineSwitchError<J::Selection>,
    > {
        let EngineSwitchReservation {
            engine_id,
            selection,
            candidate,
            backend,
            binding,
            ..
        } = reservation;
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
        self.bind_reserved(
            &engine_id,
            &candidate,
            binding,
            applied_identity,
            applied_generation,
        )
        .await
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
