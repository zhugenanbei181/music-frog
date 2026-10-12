use super::*;
use infiltrator_contract::capability::Capability;
use tokio::sync::Mutex;

#[derive(Clone, Copy)]
enum SwitchExecutionPhase {
    BeforeReservation,
    ReservationPending,
    OwnerTerminationUnconfirmed,
    OldOwnerClosed,
}

impl ActiveProfileBackendSelector {
    pub fn new(
        references: Arc<dyn ActiveProfileReferenceStore>,
        profile_stores: impl IntoIterator<Item = Arc<dyn ProfileStore>>,
        factory: Arc<dyn EngineBackendFactory>,
        capabilities: Arc<dyn CoreCapabilityProvider>,
    ) -> Self {
        let profile_stores = profile_stores
            .into_iter()
            .map(|store| (store.engine_id(), store))
            .collect();
        Self {
            references,
            profile_stores,
            factory,
            capabilities,
            selection_gate: Mutex::new(()),
        }
    }

    /// Resolve the persisted global reference and compose its provider for an
    /// unowned startup. A live owner must use `replace_owner_for_profile` so
    /// termination cannot bypass the revision-fenced replacement transaction.
    pub async fn select_backend(
        &self,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
        require_unowned_startup(previous_owner)?;
        let selection_lease = self.references.acquire_provider_selection_lease().await?;
        let referenced = self
            .resolve_reference(selection_lease.state().clone())
            .await?;
        let validated = self.load_and_validate_document(referenced).await?;
        let authorized = self.authorize_target(validated).await?;
        self.compose_backend(
            authorized,
            PreviousCoreDisposition::NoPriorOwner,
            SelectedOwnerAdmission::SelectionLease(selection_lease),
            Arc::clone(&self.capabilities),
        )
        .await
    }

    /// Finish a product-root preflight after binary/host preparation. The
    /// supplied exclusive lease keeps the exact reference stable through
    /// provider creation and owner binding.
    pub async fn select_backend_from_preflight(
        &self,
        preflight: ActiveProfileSelectionPreflight,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
        require_unowned_startup(previous_owner)?;
        validate_preflight_lease(&preflight)?;
        match self
            .factory
            .provider_availability(&preflight.reference.engine_id)
        {
            EngineBackendProviderAvailability::Registered => {}
            EngineBackendProviderAvailability::Unsupported { reason } => {
                return Err(PortError::unsupported(Capability::CoreLifecycle, reason));
            }
        }
        let store = self
            .profile_stores
            .get(&preflight.reference.engine_id)
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::Profiles,
                    format!(
                        "no profile-document store is registered for engine {}",
                        preflight.reference.engine_id.as_str()
                    ),
                )
            })?;
        let workspace = store
            .load_workspace(&preflight.reference.profile_id)
            .await?;
        validate_profile_identity(&preflight.reference, &workspace.document)?;
        if workspace.document.identity != preflight.document.identity {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed after active-profile preflight",
                true,
            )));
        }
        let ActiveProfileSelectionPreflight {
            reference,
            document,
            selection_lease,
            capability_provider,
        } = preflight;
        let authorized = AuthorizedProfile {
            reference,
            document,
        };
        self.compose_backend(
            authorized,
            PreviousCoreDisposition::NoPriorOwner,
            SelectedOwnerAdmission::SelectionLease(selection_lease),
            capability_provider,
        )
        .await
    }

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

    async fn resolve_profile_reference(
        &self,
        reference: ActiveProfileReference,
    ) -> Result<ReferencedProfile, PortError> {
        match self.factory.provider_availability(&reference.engine_id) {
            EngineBackendProviderAvailability::Registered => {}
            EngineBackendProviderAvailability::Unsupported { reason } => {
                return Err(PortError::unsupported(Capability::CoreLifecycle, reason));
            }
        }

        let store = self
            .profile_stores
            .get(&reference.engine_id)
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::Profiles,
                    format!(
                        "no profile-document store is registered for engine {}",
                        reference.engine_id.as_str()
                    ),
                )
            })?;
        if store.engine_id() != reference.engine_id {
            return Err(PortError::Failed(format!(
                "profile store registry entry for {} reports engine {}",
                reference.engine_id.as_str(),
                store.engine_id().as_str()
            )));
        }
        Ok(ReferencedProfile {
            reference,
            store: Arc::clone(store),
        })
    }

    async fn resolve_reference(
        &self,
        state: ActiveProfileReferenceState,
    ) -> Result<ReferencedProfile, PortError> {
        let reference = match state {
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "no product-wide active profile reference is selected",
                ));
            }
            ActiveProfileReferenceState::Selected(reference) => reference,
        };

        self.resolve_profile_reference(reference).await
    }

    async fn load_and_validate_document(
        &self,
        referenced: ReferencedProfile,
    ) -> Result<ValidatedProfile, PortError> {
        let workspace = referenced
            .store
            .load_workspace(&referenced.reference.profile_id)
            .await?;
        validate_profile_identity(&referenced.reference, &workspace.document)?;
        Ok(ValidatedProfile {
            reference: referenced.reference,
            workspace,
        })
    }

    async fn authorize_target(
        &self,
        validated: ValidatedProfile,
    ) -> Result<AuthorizedProfile, PortError> {
        let matrix = CoreCapabilityApplication::new(Arc::clone(&self.capabilities))
            .matrix_for(&validated.reference.engine_id, None)
            .map_err(capability_failure)?;
        let key = CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
        );
        let resolution = matrix.resolve(key);
        let lifecycle_owner_is_core_application = matches!(
            resolution.engine,
            Some(EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::CoreLifecycle,
            })
        );
        match (resolution.status, lifecycle_owner_is_core_application) {
            (
                CoreCapabilityStatus::Available {
                    access: CapabilityAccess::Supported,
                    ..
                },
                true,
            ) => {}
            (status, _) => {
                return Err(PortError::unsupported(
                    Capability::CoreLifecycle,
                    format!(
                        "Core start is not evidenced for engine {} on the current target: {status:?}",
                        validated.reference.engine_id.as_str()
                    ),
                ));
            }
        }

        Ok(AuthorizedProfile {
            reference: validated.reference,
            document: validated.workspace.document,
        })
    }

    async fn compose_backend(
        &self,
        authorized: AuthorizedProfile,
        previous_owner: PreviousCoreDisposition,
        owner_admission: SelectedOwnerAdmission,
        capability_provider: Arc<dyn CoreCapabilityProvider>,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let backend = self
            .factory
            .backend_ports(&authorized.reference.engine_id)?;
        let core_identity = backend
            .process()
            .core_identity()
            .ok_or_else(|| PortError::Failed("selected backend has no Core identity".to_owned()))?;
        if core_identity.engine != authorized.reference.engine_id
            || backend.engine_id() != &authorized.reference.engine_id
        {
            return Err(PortError::Failed(format!(
                "selected profile engine {} does not match backend identity {}",
                authorized.reference.engine_id.as_str(),
                core_identity.engine.as_str()
            )));
        }

        Ok(SelectedActiveProfileBackend {
            reference: authorized.reference,
            document: authorized.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner,
            owner_admission,
            capability_provider,
        })
    }
}

fn selected_recovery_target(
    target: &ActiveProfileReferenceTarget,
) -> Result<(EngineId, ProfileId), PortError> {
    match target {
        ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } => Ok((engine_id.clone(), profile_id.clone())),
        ActiveProfileReferenceTarget::Unselected => Err(PortError::unsupported(
            Capability::Profiles,
            "desktop owner recovery requires a selected profile target",
        )),
    }
}

fn reference_matches_target(
    state: &ActiveProfileReferenceState,
    target: &ActiveProfileReferenceTarget,
) -> bool {
    match (state, target) {
        (
            ActiveProfileReferenceState::Selected(reference),
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            },
        ) => &reference.engine_id == engine_id && &reference.profile_id == profile_id,
        _ => false,
    }
}

fn selected_reference_from_switch_state(
    state: &ActiveProfileOwnerSwitchState,
) -> Result<infiltrator_contract::active_profile_reference::ActiveProfileReference, PortError> {
    let selection = match state {
        ActiveProfileOwnerSwitchState::ReferenceChanged { selection, .. }
        | ActiveProfileOwnerSwitchState::BindingNew { selection, .. } => selection,
        ActiveProfileOwnerSwitchState::ClosingOld { .. }
        | ActiveProfileOwnerSwitchState::OldStopped { .. } => {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "owner switch has not published its replacement profile",
                true,
            )));
        }
    };
    match selection {
        ActiveProfileReferenceState::Selected(reference) => Ok(reference.clone()),
        ActiveProfileReferenceState::Unselected { .. } => Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "owner switch replacement reference is unselected",
            true,
        ))),
    }
}

fn validate_recovery_selection(
    state: &ActiveProfileOwnerSwitchState,
    target: &ActiveProfileReferenceTarget,
) -> Result<(), PortError> {
    let reference = selected_reference_from_switch_state(state)?;
    let (engine_id, profile_id) = selected_recovery_target(target)?;
    if reference.engine_id != engine_id || reference.profile_id != profile_id {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "durable owner switch target differs from the retry request",
            false,
        )));
    }
    match state {
        ActiveProfileOwnerSwitchState::BindingNew {
            engine_id: reserved_engine,
            ..
        } if reserved_engine != &engine_id => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "durable owner reservation is for another engine",
            false,
        ))),
        _ => Ok(()),
    }
}

fn previous_owner_from_old_stopped(
    state: &ActiveProfileOwnerSwitchState,
) -> Result<PreviousCoreDisposition, PortError> {
    match state {
        ActiveProfileOwnerSwitchState::OldStopped {
            owner_identity,
            terminal_generation,
            ..
        } => Ok(PreviousCoreDisposition::Stopped {
            identity: owner_identity.clone(),
            generation: *terminal_generation,
        }),
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "owner-switch recovery did not produce an OldStopped lease",
            true,
        ))),
    }
}

fn previous_owner_from_reference_changed(
    state: &ActiveProfileOwnerSwitchState,
) -> Result<PreviousCoreDisposition, PortError> {
    match state {
        ActiveProfileOwnerSwitchState::ReferenceChanged {
            owner_identity,
            terminal_generation,
            ..
        } => Ok(PreviousCoreDisposition::Stopped {
            identity: owner_identity.clone(),
            generation: *terminal_generation,
        }),
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "owner-switch recovery did not produce a ReferenceChanged lease",
            true,
        ))),
    }
}

fn recovery_required(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message, true))
}

fn validate_preflight_lease(preflight: &ActiveProfileSelectionPreflight) -> Result<(), PortError> {
    match preflight.selection_lease.state() {
        ActiveProfileReferenceState::Selected(reference) if reference == &preflight.reference => {
            Ok(())
        }
        ActiveProfileReferenceState::Unselected { .. }
        | ActiveProfileReferenceState::Selected(_) => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "active profile reference changed after preflight",
            true,
        ))),
    }
}

fn require_unowned_startup(previous_owner: &PreviousCoreOwner) -> Result<(), PortError> {
    match previous_owner {
        PreviousCoreOwner::Unowned => Ok(()),
        PreviousCoreOwner::Owned(_) => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "an owned Core must use the revision-fenced profile-owner replacement transaction",
            true,
        ))),
    }
}

pub(super) fn verify_candidate_terminal(
    before: &CoreLifecycleSnapshot,
    terminal: &CoreLifecycleSnapshot,
) -> Result<(), PortError> {
    if terminal
        .identity
        .as_ref()
        .zip(before.identity.as_ref())
        .is_some_and(|(terminal, before)| terminal.same_instance(before))
        && terminal.generation == before.generation
        && terminal.lifecycle == CoreLifecycle::Stopped
        && terminal.session_token.is_none()
    {
        Ok(())
    } else {
        Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "candidate CoreApplication did not close to its exact Stopped/no-session scope",
            true,
        )))
    }
}

pub(super) fn classify_candidate_application(
    snapshot: &CoreLifecycleSnapshot,
    identity: &CoreIdentity,
    generation: CoreGeneration,
) -> CandidateApplicationAdmission {
    match snapshot {
        CoreLifecycleSnapshot {
            identity: Some(observed_identity),
            generation: observed_generation,
            lifecycle: CoreLifecycle::Stopped,
            session_token: None,
            ..
        } if observed_identity.same_instance(identity)
            && *observed_generation == generation.value() =>
        {
            CandidateApplicationAdmission::ExactUnstarted
        }
        CoreLifecycleSnapshot {
            lifecycle: CoreLifecycle::Stopped,
            session_token: None,
            ..
        } => CandidateApplicationAdmission::ForeignUnstarted,
        _ => CandidateApplicationAdmission::StartedOrUnknown,
    }
}

fn validate_old_stopped_switch(
    switch: &dyn ActiveProfileOwnerStoppedSwitchLease,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    owner_generation: CoreGeneration,
    terminal_generation: CoreGeneration,
) -> Result<(), PortError> {
    match switch.state() {
        ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference: previous,
            owner_identity,
            owner_generation: observed_owner_generation,
            terminal_generation: observed_terminal_generation,
        } if previous == previous_reference
            && owner_identity.same_instance(identity)
            && *observed_owner_generation == owner_generation
            && *observed_terminal_generation == terminal_generation =>
        {
            Ok(())
        }
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "owner-switch store did not retain the exact old-owner terminal receipt",
            true,
        ))),
    }
}

fn validate_closing_old_switch(
    switch: &dyn ActiveProfileOwnerSwitchLease,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    generation: CoreGeneration,
) -> Result<(), PortError> {
    match switch.state() {
        ActiveProfileOwnerSwitchState::ClosingOld {
            previous_reference: previous,
            owner_identity,
            owner_generation: observed_generation,
        } if previous == previous_reference
            && owner_identity.same_instance(identity)
            && *observed_generation == generation =>
        {
            Ok(())
        }
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "owner-switch store did not reserve the exact current owner and revision",
            true,
        ))),
    }
}

fn validate_reference_changed_switch(
    switch: &dyn ActiveProfileReferenceChangedLease,
    expected_selection: &ActiveProfileReferenceState,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    terminal_generation: CoreGeneration,
) -> Result<(), PortError> {
    match switch.state() {
        ActiveProfileOwnerSwitchState::ReferenceChanged {
            previous_reference: previous,
            selection,
            owner_identity,
            terminal_generation: terminal,
        } if selection == expected_selection
            && previous == previous_reference
            && owner_identity.same_instance(identity)
            && *terminal == terminal_generation =>
        {
            Ok(())
        }
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "owner-switch store did not read back the exact CAS selection",
            true,
        ))),
    }
}

fn validate_new_owner_reservation(
    binding: &dyn ActiveProfileOwnerBindingLease,
    expected_selection: &ActiveProfileReferenceState,
    expected_engine: &EngineId,
) -> Result<(), PortError> {
    match binding.state() {
        ActiveProfileOwnerSwitchState::BindingNew {
            selection,
            engine_id,
        } if selection == expected_selection && engine_id == expected_engine => Ok(()),
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "owner-switch store did not reserve the exact target engine",
            true,
        ))),
    }
}
