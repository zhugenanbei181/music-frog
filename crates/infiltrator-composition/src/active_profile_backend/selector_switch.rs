// The neutral typed failure is the contract-required return type; boxing it would
// hide the product switch disposition behind an extra allocation.
#![allow(clippy::result_large_err)]

use super::*;
use lurker_core_contract::engine_switch::{
    EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchRecovery, EngineSwitchStage,
    EngineSwitchTarget,
};
use lurker_engine_runtime::switch::coordinator::EngineSwitchCoordinator;
use lurker_engine_runtime::switch::error::EngineSwitchError;
use lurker_engine_runtime::switch::host::EngineSwitchHost;

type SwitchError = EngineSwitchError<ActiveProfileReferenceState>;
type SwitchResult<T> = Result<T, SwitchError>;

impl ActiveProfileBackendSelector {
    /// Replace a live product owner for a selected profile through the durable,
    /// revision-fenced owner-switch journal.
    ///
    /// The sequence is fixed: resolve and authorize the target, verify the
    /// exact bound previous owner, reserve `ClosingOld`, re-read the target
    /// document, terminate the old owner and confirm its exact terminal,
    /// compare-and-set the selection, dispatch and reserve the candidate
    /// owner, then hand the `BindingNew` admission to the host that constructs
    /// the candidate CoreApplication.
    pub async fn replace_owner_for_profile(
        &self,
        target: ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
        previous_owner: &PreviousCoreOwner,
    ) -> SwitchResult<SelectedActiveProfileBackend> {
        self.replace_owner_for_profile_transaction(target, prepared_document, previous_owner)
            .await
    }

    /// Continue a durable owner switch after its previous product session has
    /// closed. The caller supplies the exact terminal snapshot produced by
    /// that close; an ambiguous live candidate remains fail-closed.
    pub async fn resume_owner_switch_for_profile(
        &self,
        target: ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
        old_owner_terminal: &CoreLifecycleSnapshot,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let authorized = self
            .preflight_recovery_target(&target, prepared_document)
            .await?;
        let snapshot = self.references.snapshot().await?;
        match self
            .references
            .recover_owner_switch(snapshot.revision)
            .await?
        {
            EngineSwitchRecovery::NoPendingSwitch => {
                self.select_recovery_target_unowned(target).await
            }
            EngineSwitchRecovery::ResumeClosingOld {
                owner_identity,
                owner_generation,
                ..
            } => {
                let terminal_generation = verify_recovery_owner_terminal(
                    old_owner_terminal,
                    &owner_identity,
                    Some(owner_generation),
                    None,
                )?;
                let stopped = self
                    .references
                    .recover_old_owner_after_terminal(snapshot.revision, terminal_generation)
                    .await?;
                self.resume_from_old_stopped(target, authorized, stopped)
                    .await
            }
            EngineSwitchRecovery::ResumeOldStopped {
                owner_identity,
                owner_generation,
                terminal_generation,
                ..
            } => {
                verify_recovery_owner_terminal(
                    old_owner_terminal,
                    &owner_identity,
                    Some(owner_generation),
                    Some(terminal_generation),
                )?;
                let stopped = self
                    .references
                    .recover_old_owner_after_terminal(snapshot.revision, terminal_generation)
                    .await?;
                self.resume_from_old_stopped(target, authorized, stopped)
                    .await
            }
            EngineSwitchRecovery::ResumeSelectionChanged {
                owner_identity,
                terminal_generation,
                ..
            } => {
                verify_recovery_owner_terminal(
                    old_owner_terminal,
                    &owner_identity,
                    None,
                    Some(terminal_generation),
                )?;
                let stopped = self
                    .references
                    .recover_old_owner_after_terminal(snapshot.revision, terminal_generation)
                    .await?;
                self.resume_from_old_stopped(target, authorized, stopped)
                    .await
            }
            EngineSwitchRecovery::ResumeBindingNew { selection, .. } => {
                if !reference_matches_target(&selection, &target) {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "durable owner reservation target differs from the retry request",
                        false,
                    )));
                }
                // The reserved candidate has no CoreApplication: the switch
                // stopped before the host constructed it. Release the
                // never-started admission and compose the committed selection
                // again under a fresh provider selection lease.
                self.references
                    .recover_candidate_after_terminal(snapshot.revision, CoreGeneration::new(0))
                    .await?;
                self.select_recovery_target_unowned(target).await
            }
        }
    }

    async fn preflight_recovery_target(
        &self,
        target: &ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
    ) -> Result<AuthorizedProfile, PortError> {
        let (engine_id, profile_id) = selected_recovery_target(target)?;
        let observed = self.references.read().await?;
        let reference = ActiveProfileReference::new(engine_id, profile_id, observed.revision());
        let referenced = self.resolve_profile_reference(reference).await?;
        let validated = self.load_and_validate_document(referenced).await?;
        if &validated.workspace.document != prepared_document {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed before owner-switch recovery",
                true,
            )));
        }
        self.authorize_target(validated).await
    }

    async fn select_recovery_target_unowned(
        &self,
        target: ActiveProfileReferenceTarget,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let current = self.references.read().await?;
        if !reference_matches_target(&current, &target) {
            let published = self
                .references
                .compare_and_set(
                    current.revision(),
                    target.clone(),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await?;
            if !reference_matches_target(&published, &target) {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "unowned recovery selection did not read back the requested profile",
                    true,
                )));
            }
        }
        self.select_backend(&PreviousCoreOwner::Unowned).await
    }

    async fn resume_from_old_stopped(
        &self,
        target: ActiveProfileReferenceTarget,
        authorized: AuthorizedProfile,
        stopped: Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let previous_owner = previous_owner_from_old_stopped(stopped.stage())?;
        let (engine_id, profile_id) = selected_recovery_target(&target)?;
        // A recovered `SelectionChanged` journal already committed the target;
        // repeating the same selection is idempotent and must not advance the
        // revision a second time.
        let selection = match stopped.stage() {
            EngineSwitchStage::SelectionChanged { selection, .. } => selection.clone(),
            EngineSwitchStage::OldStopped { previous, .. } => {
                let revision = previous
                    .revision()
                    .next()
                    .ok_or_else(|| PortError::Failed("active profile revision exhausted".into()))?;
                ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                    engine_id.clone(),
                    profile_id,
                    revision,
                ))
            }
            EngineSwitchStage::ClosingOld { .. } | EngineSwitchStage::BindingNew { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "owner-switch recovery is not at the old-stopped stage",
                    true,
                )));
            }
        };
        let changed = stopped.compare_and_set_target(selection, engine_id).await?;
        self.resume_from_selection_changed(target, authorized, changed, previous_owner)
            .await
    }

    async fn resume_from_selection_changed(
        &self,
        target: ActiveProfileReferenceTarget,
        authorized: AuthorizedProfile,
        changed: Box<dyn EngineSwitchSelectionChangedLease<ActiveProfileReferenceState>>,
        previous_owner: PreviousCoreDisposition,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        validate_recovery_selection(changed.stage(), &target)?;
        let reference = selected_reference_from_stage(changed.stage())?;
        let backend = match self.registry.backend(&reference.engine_id) {
            Ok(backend) => backend,
            Err(error) => {
                return Err(error);
            }
        };
        let core_identity = match backend.process().core_identity() {
            Some(identity) => identity,
            None => {
                return Err(PortError::Failed(
                    "recovered backend has no Core identity".to_owned(),
                ));
            }
        };
        if core_identity.engine != reference.engine_id
            || backend.engine_id() != &reference.engine_id
        {
            return Err(PortError::Failed(format!(
                "recovered profile engine {} does not match backend identity {}",
                reference.engine_id.as_str(),
                core_identity.engine.as_str()
            )));
        }
        let binding = changed
            .reserve_new_owner(
                reference.engine_id.clone(),
                core_identity.clone(),
                CoreGeneration::new(0),
            )
            .await?;
        Ok(SelectedActiveProfileBackend {
            reference,
            document: authorized.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner,
            owner_admission: SelectedOwnerAdmission::SwitchReservation(binding),
            capability_provider: Arc::clone(&self.capabilities),
            owner_store: Arc::clone(&self.references),
        })
    }

    async fn replace_owner_for_profile_transaction(
        &self,
        target: ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
        previous_owner: &PreviousCoreOwner,
    ) -> SwitchResult<SelectedActiveProfileBackend> {
        let _writer = self.selection_gate.lock().await;
        let snapshot = match self.references.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(cause) => {
                return Err(SwitchError::recovery_required(
                    EngineSwitchFailurePhase::BeforeReservation,
                    cause,
                ));
            }
        };
        let (target_engine, target_profile) = match target {
            ActiveProfileReferenceTarget::Unselected => {
                return Err(self
                    .before_reservation(
                        snapshot.selection,
                        PortError::unsupported(
                            Capability::Profiles,
                            "Core owner replacement requires a selected target profile",
                        ),
                    )
                    .await);
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => (engine_id, profile_id),
        };

        let previous_reference = snapshot.selection;
        if matches!(
            &previous_reference,
            ActiveProfileReferenceState::Unselected { .. }
        ) {
            return Err(self
                .before_reservation(
                    previous_reference.clone(),
                    PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "a live Core owner cannot switch from an unselected product reference",
                        true,
                    )),
                )
                .await);
        }
        let expected_revision = snapshot.revision;
        let target_revision = match previous_reference.revision().next() {
            Some(revision) => revision,
            None => {
                return Err(self
                    .before_reservation(
                        previous_reference.clone(),
                        PortError::Failed("active profile reference revision exhausted".into()),
                    )
                    .await);
            }
        };
        let target_reference = ActiveProfileReference::new(
            target_engine.clone(),
            target_profile.clone(),
            target_revision,
        );
        let referenced = match self
            .resolve_profile_reference(target_reference.clone())
            .await
        {
            Ok(referenced) => referenced,
            Err(cause) => {
                return Err(self
                    .before_reservation(previous_reference.clone(), cause)
                    .await);
            }
        };
        let target_store = Arc::clone(&referenced.store);
        let validated = match self.load_and_validate_document(referenced).await {
            Ok(validated) => validated,
            Err(cause) => {
                return Err(self
                    .before_reservation(previous_reference.clone(), cause)
                    .await);
            }
        };
        let authorized = match self.authorize_target(validated).await {
            Ok(authorized) => authorized,
            Err(cause) => {
                return Err(self
                    .before_reservation(previous_reference.clone(), cause)
                    .await);
            }
        };
        if &authorized.document != prepared_document {
            return Err(
                self.before_reservation(
                    previous_reference.clone(),
                    PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "prepared target backend does not match the exact selected profile document",
                        true,
                    )),
                )
                .await,
            );
        }
        let owner = match previous_owner {
            PreviousCoreOwner::Unowned => {
                return Err(self
                    .before_reservation(
                        previous_reference.clone(),
                        PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            "a live Core owner replacement requires the current owned owner",
                            true,
                        )),
                    )
                    .await);
            }
            PreviousCoreOwner::Owned(owner) => Arc::clone(owner),
        };

        // The neutral coordinator owns the whole forward order: dispatch and
        // candidate preparation happen before the durable reservation, then
        // the exact old owner is terminated, the selection is compared-and-set
        // and the `BindingNew` admission is reserved. The product root never
        // drives the journal leases itself.
        let switch_host: Arc<
            dyn EngineSwitchHost<ProductEngineBackendPorts, ActiveProfileReferenceState>,
        > = Arc::new(ProfileEngineSwitchHost {
            target_store,
            authorized: AuthorizedProfile {
                reference: authorized.reference.clone(),
                document: authorized.document.clone(),
            },
            previous_owner: owner,
        });
        let coordinator = Arc::new(EngineSwitchCoordinator::new(
            Arc::clone(&self.registry),
            Arc::clone(&self.references),
            switch_host,
        ));
        let requested = EngineSwitchTarget::Selected {
            engine_id: target_engine.clone(),
            selection: ActiveProfileReferenceState::Selected(target_reference.clone()),
        };
        let reservation = coordinator
            .reserve_switch(expected_revision, requested)
            .await?;

        // The backend exists but its application actor has not been built.
        // Keep `BindingNew` until the caller verifies the CoreApplication
        // scope and hands the reservation back for finalization.
        let backend = reservation.backend().clone();
        let core_identity = reservation.candidate_identity().clone();
        let previous_identity = reservation.previous_owner().0.clone();
        Ok(SelectedActiveProfileBackend {
            reference: target_reference,
            document: authorized.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner: PreviousCoreDisposition::Stopped {
                identity: previous_identity,
                generation: reservation.terminal_generation(),
            },
            owner_admission: SelectedOwnerAdmission::CoordinatorReservation {
                coordinator,
                reservation: Box::new(reservation),
            },
            capability_provider: Arc::clone(&self.capabilities),
            owner_store: Arc::clone(&self.references),
        })
    }

    async fn before_reservation(
        &self,
        fallback: ActiveProfileReferenceState,
        cause: PortError,
    ) -> SwitchError {
        match self.references.snapshot().await {
            Ok(snapshot) => SwitchError::new(
                EngineSwitchFailurePhase::BeforeReservation,
                cause,
                EngineSwitchDisposition::RolledBackToPrevious {
                    selection: snapshot.selection,
                },
            ),
            Err(_) => {
                let _ = fallback;
                SwitchError::recovery_required(EngineSwitchFailurePhase::BeforeReservation, cause)
            }
        }
    }
}

/// Verify the caller's terminal receipt against the stage's exact owner fence
/// and reject any state that is not the exact Stopped/no-session owner.
fn verify_recovery_owner_terminal(
    terminal: &CoreLifecycleSnapshot,
    owner_identity: &CoreIdentity,
    owner_generation: Option<CoreGeneration>,
    recorded_terminal: Option<CoreGeneration>,
) -> Result<CoreGeneration, PortError> {
    let terminal_identity = terminal.identity.as_ref().ok_or_else(|| {
        recovery_required("old Core owner terminal readback has no typed identity")
    })?;
    if !owner_identity.same_instance(terminal_identity)
        || terminal.lifecycle != CoreLifecycle::Stopped
        || terminal.session_token.is_some()
    {
        return Err(recovery_required(
            "the previous Core owner has no exact Stopped/no-session terminal readback",
        ));
    }
    let terminal_generation = CoreGeneration::new(terminal.generation);
    if owner_generation.is_some_and(|owner_generation| terminal_generation < owner_generation) {
        return Err(recovery_required(
            "the previous Core owner terminal generation predates its binding",
        ));
    }
    if recorded_terminal.is_some_and(|recorded| recorded != terminal_generation) {
        return Err(recovery_required(
            "the previous Core owner terminal generation differs from the durable switch",
        ));
    }
    Ok(terminal_generation)
}

/// Host seam between the neutral [`EngineSwitchCoordinator`] and the product.
///
/// The coordinator owns the order and the durable journal; this host owns the
/// product facts: the exact target document, the exact dispatch identity and
/// the exact live previous owner. It never applies the candidate itself,
/// because the product root constructs the candidate `CoreApplication` and
/// hands the reservation back to the coordinator.
struct ProfileEngineSwitchHost {
    target_store: Arc<dyn ProfileStore>,
    authorized: AuthorizedProfile,
    previous_owner: Arc<dyn CoreOwnerTerminationPort>,
}

#[async_trait::async_trait]
impl EngineSwitchHost<ProductEngineBackendPorts, ActiveProfileReferenceState>
    for ProfileEngineSwitchHost
{
    async fn validate_target(
        &self,
        target: &EngineSwitchTarget<ActiveProfileReferenceState>,
    ) -> Result<(), PortError> {
        let (engine_id, selection) = match target {
            EngineSwitchTarget::Selected {
                engine_id,
                selection,
            } => (engine_id, selection),
            EngineSwitchTarget::Unselected => {
                return Err(not_ready(
                    "owner switch requires a selected product profile target",
                ));
            }
        };
        if engine_id != &self.authorized.reference.engine_id {
            return Err(not_ready(
                "owner switch target engine differs from the authorized profile engine",
            ));
        }
        match selection {
            ActiveProfileReferenceState::Selected(reference)
                if reference == &self.authorized.reference =>
            {
                Ok(())
            }
            ActiveProfileReferenceState::Unselected { .. }
            | ActiveProfileReferenceState::Selected(_) => Err(not_ready(
                "owner switch target selection differs from the authorized profile reference",
            )),
        }
    }

    async fn prepare_candidate(
        &self,
        engine_id: &EngineId,
        selection: &ActiveProfileReferenceState,
        backend: &ProductEngineBackendPorts,
    ) -> Result<CoreIdentity, PortError> {
        match selection {
            ActiveProfileReferenceState::Selected(reference)
                if reference == &self.authorized.reference => {}
            ActiveProfileReferenceState::Unselected { .. }
            | ActiveProfileReferenceState::Selected(_) => {
                return Err(not_ready(
                    "owner switch candidate differs from the authorized profile reference",
                ));
            }
        }
        // The exact target document is re-verified after dispatch but before
        // the durable reservation, so a target that disappeared after the
        // product preflight rolls back without ever touching the old owner.
        verify_target_document(
            self.target_store.as_ref(),
            &self.authorized.reference,
            &self.authorized.document,
        )
        .await?;
        let core_identity = backend
            .process()
            .core_identity()
            .ok_or_else(|| PortError::Failed("selected backend has no Core identity".to_owned()))?;
        if core_identity.engine != *engine_id || backend.engine_id() != engine_id {
            return Err(PortError::Failed(format!(
                "selected profile engine {} does not match backend identity {}",
                engine_id.as_str(),
                core_identity.engine.as_str()
            )));
        }
        Ok(core_identity)
    }

    async fn terminate_old_owner(
        &self,
        identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<CoreGeneration, PortError> {
        let before = self.previous_owner.lifecycle_snapshot();
        let live_identity = before.identity.clone().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "previous Core owner has no typed identity to terminate",
            )
        })?;
        if !live_identity.same_instance(identity) {
            return Err(not_ready(
                "active Core owner identity differs from the durable switch owner",
            ));
        }
        match self
            .previous_owner
            .active_profile_reference_write_authorization()?
        {
            ActiveProfileReferenceWriteAuthorization::BoundOwner {
                identity: bound_identity,
                generation: bound_generation,
            } if bound_identity.same_instance(identity) && bound_generation == generation => {}
            ActiveProfileReferenceWriteAuthorization::BoundOwner { .. } => {
                return Err(not_ready(
                    "active Core owner binding generation differs from the durable switch owner",
                ));
            }
            ActiveProfileReferenceWriteAuthorization::Unowned => {
                return Err(not_ready(
                    "a live Core owner replacement requires the exact bound owner lease",
                ));
            }
        }
        let lifecycle_generation = CoreGeneration::new(before.generation);
        if generation > lifecycle_generation {
            return Err(not_ready(
                "active Core owner binding generation is ahead of its lifecycle snapshot",
            ));
        }
        let terminal = self.previous_owner.terminate_owner().await?;
        verify_stopped_owner(identity, lifecycle_generation, &terminal)?;
        Ok(CoreGeneration::new(terminal.generation))
    }

    async fn apply_candidate(
        &self,
        _backend: ProductEngineBackendPorts,
        _selection: &ActiveProfileReferenceState,
    ) -> Result<(CoreIdentity, CoreGeneration), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the product root applies the candidate CoreApplication and finalizes the reservation itself",
        ))
    }

    async fn close_candidate(&self, _backend: ProductEngineBackendPorts) -> Result<(), PortError> {
        Ok(())
    }
}

fn not_ready(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message.to_owned(), true))
}
