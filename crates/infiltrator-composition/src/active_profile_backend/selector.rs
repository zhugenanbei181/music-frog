use super::*;
use lurker_core_contract::active_profile_reference::ActiveProfileReference;
use lurker_core_contract::capability::Capability;
use tokio::sync::Mutex;

#[path = "selector_switch.rs"]
mod selector_switch;

impl ActiveProfileBackendSelector {
    pub fn new(
        references: Arc<dyn ActiveProfileReferenceStore>,
        profile_stores: impl IntoIterator<Item = Arc<dyn ProfileStore>>,
        registry: Arc<EngineBackendProviderRegistry<ProductEngineBackendPorts>>,
        capabilities: Arc<dyn CoreCapabilityProvider>,
    ) -> Self {
        let profile_stores = profile_stores
            .into_iter()
            .map(|store| (store.engine_id(), store))
            .collect();
        Self {
            references,
            profile_stores,
            registry,
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
        match self.registry.availability(&preflight.reference.engine_id) {
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

    async fn resolve_profile_reference(
        &self,
        reference: ActiveProfileReference,
    ) -> Result<ReferencedProfile, PortError> {
        match self.registry.availability(&reference.engine_id) {
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
        let backend = self.registry.backend(&authorized.reference.engine_id)?;
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
            owner_store: Arc::clone(&self.references),
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

fn selected_reference_from_stage(
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
) -> Result<ActiveProfileReference, PortError> {
    let selection = match stage {
        EngineSwitchStage::SelectionChanged { selection, .. }
        | EngineSwitchStage::BindingNew { selection, .. } => selection,
        EngineSwitchStage::ClosingOld { .. } | EngineSwitchStage::OldStopped { .. } => {
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
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
    target: &ActiveProfileReferenceTarget,
) -> Result<(), PortError> {
    let reference = selected_reference_from_stage(stage)?;
    let (engine_id, profile_id) = selected_recovery_target(target)?;
    if reference.engine_id != engine_id || reference.profile_id != profile_id {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "durable owner switch target differs from the retry request",
            false,
        )));
    }
    match stage {
        EngineSwitchStage::BindingNew {
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
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
) -> Result<PreviousCoreDisposition, PortError> {
    match stage {
        EngineSwitchStage::OldStopped {
            owner_identity,
            terminal_generation,
            ..
        }
        | EngineSwitchStage::SelectionChanged {
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

fn validate_closing_old_stage(
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    generation: CoreGeneration,
) -> Result<(), PortError> {
    match stage {
        EngineSwitchStage::ClosingOld {
            previous,
            owner_identity,
            owner_generation,
        } if previous == previous_reference
            && owner_identity.same_instance(identity)
            && *owner_generation == generation =>
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

fn validate_old_stopped_stage(
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    owner_generation: CoreGeneration,
    terminal_generation: CoreGeneration,
) -> Result<(), PortError> {
    match stage {
        EngineSwitchStage::OldStopped {
            previous,
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

fn validate_selection_changed_stage(
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
    expected_selection: &ActiveProfileReferenceState,
    previous_reference: &ActiveProfileReferenceState,
    identity: &CoreIdentity,
    terminal_generation: CoreGeneration,
) -> Result<(), PortError> {
    match stage {
        EngineSwitchStage::SelectionChanged {
            previous,
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
    stage: &EngineSwitchStage<ActiveProfileReferenceState>,
    expected_selection: &ActiveProfileReferenceState,
    expected_engine: &EngineId,
) -> Result<(), PortError> {
    match stage {
        EngineSwitchStage::BindingNew {
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
