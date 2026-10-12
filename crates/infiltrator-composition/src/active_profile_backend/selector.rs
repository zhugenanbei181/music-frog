use super::*;

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
            selection_gate: tokio::sync::Mutex::new(()),
        }
    }

    /// Resolve the persisted global reference and compose its provider for an
    /// unowned startup. A live owner must use `switch_backend` so termination
    /// cannot bypass the revision-fenced owner-switch transaction.
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
                return Err(PortError::unsupported(
                    infiltrator_contract::capability::Capability::CoreLifecycle,
                    reason,
                ));
            }
        }
        let store = self
            .profile_stores
            .get(&preflight.reference.engine_id)
            .ok_or_else(|| {
                PortError::unsupported(
                    infiltrator_contract::capability::Capability::Profiles,
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

    /// Switch a live product owner to a different engine through a durable,
    /// revision-fenced transaction. Target validation is complete before the
    /// old owner is asked to close. The switch lease persists each phase but
    /// holds no root lock while `terminate_owner` drains CoreApplication.
    pub async fn switch_backend(
        &self,
        target: ActiveProfileReferenceTarget,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
        let (target_engine, target_profile) = match target {
            ActiveProfileReferenceTarget::Unselected => {
                return Err(PortError::unsupported(
                    infiltrator_contract::capability::Capability::Profiles,
                    "a Core owner switch requires a selected target profile",
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
        if current_reference.engine_id == target_engine {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "cross-engine Core switching requires a different target engine",
                false,
            )));
        }
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

        let owner = match previous_owner {
            PreviousCoreOwner::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "cross-engine Core switching requires the current owned Core owner",
                    true,
                )));
            }
            PreviousCoreOwner::Owned(owner) => owner,
        };
        let before = owner.lifecycle_snapshot();
        let expected_identity = before.identity.clone().ok_or_else(|| {
            PortError::unsupported(
                infiltrator_contract::capability::Capability::CoreLifecycle,
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
                    "cross-engine Core switching requires the exact bound owner lease",
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
        let terminal = owner.terminate_owner().await?;
        verify_stopped_owner(&expected_identity, lifecycle_generation, &terminal)?;

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
                return Err(PortError::unsupported(
                    infiltrator_contract::capability::Capability::CoreLifecycle,
                    reason,
                ));
            }
        }

        let store = self
            .profile_stores
            .get(&reference.engine_id)
            .ok_or_else(|| {
                PortError::unsupported(
                    infiltrator_contract::capability::Capability::Profiles,
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
                    infiltrator_contract::capability::Capability::Profiles,
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
                    infiltrator_contract::capability::Capability::CoreLifecycle,
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
            "an owned Core must use the revision-fenced cross-engine switch transaction",
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
