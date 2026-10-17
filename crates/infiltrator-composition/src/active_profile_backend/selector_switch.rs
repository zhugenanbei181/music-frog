use super::*;

impl ActiveProfileBackendSelector {
    /// Replace a live product owner for a selected profile through a durable,
    /// revision-fenced transaction. The profile may use the current backend or
    /// another engine. Target validation completes before the old owner closes.
    /// The lease persists each phase without holding a root lock while draining.
    pub async fn replace_owner_for_profile(
        &self,
        target: ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, ActiveProfileBackendSwitchError> {
        let mut phase = SwitchExecutionPhase::BeforeReservation;
        self.replace_owner_for_profile_transaction(
            target,
            prepared_document,
            previous_owner,
            &mut phase,
        )
        .await
        .map_err(|failure| match phase {
            SwitchExecutionPhase::BeforeReservation => {
                ActiveProfileBackendSwitchError::BeforeReservation(failure)
            }
            SwitchExecutionPhase::ReservationPending => {
                ActiveProfileBackendSwitchError::ReservationPending(failure)
            }
            SwitchExecutionPhase::OwnerTerminationUnconfirmed => {
                ActiveProfileBackendSwitchError::OwnerTerminationUnconfirmed(failure)
            }
            SwitchExecutionPhase::OldOwnerClosed => {
                ActiveProfileBackendSwitchError::OldOwnerClosed(failure)
            }
        })
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
        match self.references.recover_owner_switch().await? {
            ActiveProfileOwnerSwitchRecovery::NoPendingSwitch => {
                self.select_recovery_target_unowned(target).await
            }
            ActiveProfileOwnerSwitchRecovery::ResumeClosingOld(lease) => {
                let stopped = lease.confirm_old_owner_stopped(old_owner_terminal).await?;
                self.resume_from_old_stopped(target, authorized, stopped)
                    .await
            }
            ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(lease) => {
                self.resume_from_old_stopped(target, authorized, lease)
                    .await
            }
            ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired { .. } => {
                let stopped = self
                    .references
                    .recover_old_owner_after_terminal(old_owner_terminal)
                    .await?;
                self.resume_from_old_stopped(target, authorized, stopped)
                    .await
            }
            ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(lease) => {
                let previous_owner = previous_owner_from_reference_changed(lease.state())?;
                self.resume_from_reference_changed(target, authorized, lease, previous_owner)
                    .await
            }
            ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(lease) => {
                self.resume_from_binding_new(
                    target,
                    authorized,
                    lease,
                    PreviousCoreDisposition::NoPriorOwner,
                )
                .await
            }
            ActiveProfileOwnerSwitchRecovery::OwnerStillLive { .. } => Err(recovery_required(
                "the previous Core owner is still live after session close",
            )),
            ActiveProfileOwnerSwitchRecovery::CandidateStillLive { .. } => Err(recovery_required(
                "the replacement Core candidate is still live after session close",
            )),
            ActiveProfileOwnerSwitchRecovery::CandidateTerminalEvidenceRequired { .. } => {
                Err(recovery_required(
                    "the replacement Core candidate needs exact terminal evidence before retry",
                ))
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
        lease: Box<dyn ActiveProfileOwnerStoppedSwitchLease>,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let previous_owner = previous_owner_from_old_stopped(lease.state())?;
        let changed = lease.compare_and_set_target(target.clone()).await?;
        self.resume_from_reference_changed(target, authorized, changed, previous_owner)
            .await
    }

    async fn resume_from_reference_changed(
        &self,
        target: ActiveProfileReferenceTarget,
        authorized: AuthorizedProfile,
        lease: Box<dyn ActiveProfileReferenceChangedLease>,
        previous_owner: PreviousCoreDisposition,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        validate_recovery_selection(lease.state(), &target)?;
        let (engine_id, _) = selected_recovery_target(&target)?;
        let binding = lease.reserve_new_owner(engine_id).await?;
        self.resume_from_binding_new(target, authorized, binding, previous_owner)
            .await
    }

    async fn resume_from_binding_new(
        &self,
        target: ActiveProfileReferenceTarget,
        authorized: AuthorizedProfile,
        binding: Box<dyn ActiveProfileOwnerBindingLease>,
        previous_owner: PreviousCoreDisposition,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        validate_recovery_selection(binding.state(), &target)?;
        let reference = selected_reference_from_switch_state(binding.state())?;
        let backend = match self.factory.backend_ports(&reference.engine_id) {
            Ok(backend) => backend,
            Err(error) => {
                binding.abort_before_core_application().await?;
                return Err(error);
            }
        };
        let core_identity = match backend.process().core_identity() {
            Some(identity) => identity,
            None => {
                binding.abort_before_core_application().await?;
                return Err(PortError::Failed(
                    "recovered backend has no Core identity".to_owned(),
                ));
            }
        };
        if core_identity.engine != reference.engine_id
            || backend.engine_id() != &reference.engine_id
        {
            binding.abort_before_core_application().await?;
            return Err(PortError::Failed(format!(
                "recovered profile engine {} does not match backend identity {}",
                reference.engine_id.as_str(),
                core_identity.engine.as_str()
            )));
        }
        Ok(SelectedActiveProfileBackend {
            reference,
            document: authorized.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner,
            owner_admission: SelectedOwnerAdmission::SwitchReservation(binding),
            capability_provider: Arc::clone(&self.capabilities),
        })
    }

    async fn replace_owner_for_profile_transaction(
        &self,
        target: ActiveProfileReferenceTarget,
        prepared_document: &ProfileDocument,
        previous_owner: &PreviousCoreOwner,
        phase: &mut SwitchExecutionPhase,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
        let (target_engine, target_profile) = match target {
            ActiveProfileReferenceTarget::Unselected => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "Core owner replacement requires a selected target profile",
                ));
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => (engine_id, profile_id),
        };

        let previous_reference = self.references.read().await?;
        let current_reference = match &previous_reference {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "a live Core owner cannot switch from an unselected product reference",
                    true,
                )));
            }
        };
        let expected_revision = previous_reference.revision();
        let target_revision = expected_revision.next().ok_or_else(|| {
            PortError::Failed("active profile reference revision exhausted".into())
        })?;
        let target_reference = ActiveProfileReference::new(
            target_engine.clone(),
            target_profile.clone(),
            target_revision,
        );
        let referenced = self.resolve_profile_reference(target_reference).await?;
        let target_store = Arc::clone(&referenced.store);
        let validated = self.load_and_validate_document(referenced).await?;
        let authorized = self.authorize_target(validated).await?;
        if &authorized.document != prepared_document {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "prepared target backend does not match the exact selected profile document",
                true,
            )));
        }

        let owner = match previous_owner {
            PreviousCoreOwner::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "a live Core owner replacement requires the current owned owner",
                    true,
                )));
            }
            PreviousCoreOwner::Owned(owner) => owner,
        };
        let before = owner.lifecycle_snapshot();
        let expected_identity = before.identity.clone().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "previous Core owner has no typed identity to terminate",
            )
        })?;
        let binding_generation = match owner.active_profile_reference_write_authorization()? {
            ActiveProfileReferenceWriteAuthorization::BoundOwner {
                identity,
                generation,
            } if identity.same_instance(&expected_identity) => generation,
            ActiveProfileReferenceWriteAuthorization::BoundOwner { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "active Core owner binding identity differs from its lifecycle snapshot",
                    true,
                )));
            }
            ActiveProfileReferenceWriteAuthorization::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "a live Core owner replacement requires the exact bound owner lease",
                    true,
                )));
            }
        };
        let lifecycle_generation = CoreGeneration::new(before.generation);
        if binding_generation > lifecycle_generation {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active Core owner binding generation is ahead of its lifecycle snapshot",
                true,
            )));
        }
        if expected_identity.engine != current_reference.engine_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                format!(
                    "active profile engine {} does not match current Core owner engine {}",
                    current_reference.engine_id.as_str(),
                    expected_identity.engine.as_str()
                ),
                true,
            )));
        }
        // This writes a durable reservation under the store's short root
        // critical section. The returned lease does not retain that lock.
        let switch = self
            .references
            .begin_owner_switch(
                expected_revision,
                expected_identity.clone(),
                binding_generation,
            )
            .await?;
        *phase = SwitchExecutionPhase::ReservationPending;
        validate_closing_old_switch(
            switch.as_ref(),
            &previous_reference,
            &expected_identity,
            binding_generation,
        )?;
        if let Err(target_error) = verify_target_document(
            target_store.as_ref(),
            &authorized.reference,
            &authorized.document,
        )
        .await
        {
            return match switch.cancel_before_old_owner_close().await {
                Ok(restored_reference) if restored_reference == previous_reference => {
                    *phase = SwitchExecutionPhase::BeforeReservation;
                    Err(target_error)
                }
                Ok(_) => Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "target preflight failed and cancellation did not restore the exact source reference",
                    true,
                ))),
                Err(cancel_error) => Err(cancel_error),
            };
        }
        // Once the durable marker exists it serializes competing selectors;
        // release even the composition-local gate before waiting for close.
        drop(_writer);
        *phase = SwitchExecutionPhase::OwnerTerminationUnconfirmed;
        let terminal = owner.terminate_owner().await?;
        verify_stopped_owner(&expected_identity, lifecycle_generation, &terminal)?;
        *phase = SwitchExecutionPhase::OldOwnerClosed;

        let stopped_switch = switch.confirm_old_owner_stopped(&terminal).await?;
        validate_old_stopped_switch(
            stopped_switch.as_ref(),
            &previous_reference,
            &expected_identity,
            binding_generation,
            CoreGeneration::new(terminal.generation),
        )?;
        let selection_reference = ActiveProfileReference::new(
            target_engine.clone(),
            target_profile.clone(),
            target_revision,
        );
        let expected_selection = ActiveProfileReferenceState::Selected(selection_reference.clone());
        let target =
            ActiveProfileReferenceTarget::selected(target_engine.clone(), target_profile.clone());
        let changed = stopped_switch.compare_and_set_target(target).await?;
        validate_reference_changed_switch(
            changed.as_ref(),
            &expected_selection,
            &previous_reference,
            &expected_identity,
            CoreGeneration::new(terminal.generation),
        )?;

        let binding = changed.reserve_new_owner(target_engine.clone()).await?;
        validate_new_owner_reservation(binding.as_ref(), &expected_selection, &target_engine)?;

        // Provider dispatch is deliberately after both the revision CAS and
        // the durable new-owner admission reservation.
        let backend = match self.factory.backend_ports(&target_engine) {
            Ok(backend) => backend,
            Err(error) => {
                binding.abort_before_core_application().await?;
                return Err(error);
            }
        };
        let core_identity = match backend.process().core_identity() {
            Some(identity) => identity,
            None => {
                binding.abort_before_core_application().await?;
                return Err(PortError::Failed(
                    "selected backend has no Core identity".to_owned(),
                ));
            }
        };
        if core_identity.engine != target_engine || backend.engine_id() != &target_engine {
            binding.abort_before_core_application().await?;
            return Err(PortError::Failed(format!(
                "selected profile engine {} does not match backend identity {}",
                target_engine.as_str(),
                core_identity.engine.as_str()
            )));
        }

        // The backend exists but its application actor has not been built.
        // Keep BindingNew until the caller verifies the CoreApplication scope.
        Ok(SelectedActiveProfileBackend {
            reference: selection_reference,
            document: authorized.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner: PreviousCoreDisposition::Stopped {
                identity: expected_identity,
                generation: CoreGeneration::new(terminal.generation),
            },
            owner_admission: SelectedOwnerAdmission::SwitchReservation(binding),
            capability_provider: Arc::clone(&self.capabilities),
        })
    }
}
