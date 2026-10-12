//! Resolve the unique active profile reference before composing one Core backend.

use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityKey, CoreCapabilityStatus, CoreLifecycleOperation,
    CoreOperation, EngineCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileDocument;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchState,
    ActiveProfileReferenceChangedLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_lifecycle::CoreOwnerTerminationPort;
use infiltrator_ports::engine_backend_factory::{
    EngineBackendFactory, EngineBackendPorts, EngineBackendProviderAvailability,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
#[path = "active_profile_backend_tests.rs"]
mod tests;

/// The owner state entering a provider switch. The enum keeps the no-owner
/// startup case separate from an owned Core that must reach a terminal state.
pub enum PreviousCoreOwner {
    Unowned,
    Owned(Arc<dyn CoreOwnerTerminationPort>),
}

/// Proof that any previous owner was closed before a new backend was composed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviousCoreDisposition {
    NoPriorOwner,
    Stopped {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
}

/// One validated backend candidate. Its Core process has not been started and
/// its configuration transaction has not been invoked.
pub struct SelectedActiveProfileBackend {
    pub reference: ActiveProfileReference,
    pub document: ProfileDocument,
    pub backend: EngineBackendPorts,
    pub core_identity: CoreIdentity,
    pub initial_generation: CoreGeneration,
    pub previous_owner: PreviousCoreDisposition,
    owner_admission: SelectedOwnerAdmission,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
}

enum SelectedOwnerAdmission {
    SelectionLease(Box<dyn ActiveProfileReferenceLease>),
    SwitchReservation(Box<dyn ActiveProfileOwnerBindingLease>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateApplicationAdmission {
    ExactUnstarted,
    ForeignUnstarted,
    StartedOrUnknown,
}

struct SharedOwnerLease {
    inner: Arc<dyn ActiveProfileOwnerLease>,
}

#[async_trait::async_trait]
impl ActiveProfileOwnerLease for SharedOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        self.inner.identity()
    }

    fn generation(&self) -> CoreGeneration {
        self.inner.generation()
    }

    async fn record_start_context(
        &self,
        context: &infiltrator_contract::core_process::CoreProcessStartContext,
    ) -> Result<(), PortError> {
        self.inner.record_start_context(context).await
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        self.inner.release_after_terminal(terminal).await
    }
}

/// Profile/document/capability preflight held under an exclusive active
/// reference lease. Product roots can finish binary and host preparation while
/// the lease prevents a concurrent selection update, then pass it to the
/// backend selector for provider dispatch and owner binding.
pub struct ActiveProfileSelectionPreflight {
    pub reference: ActiveProfileReference,
    pub document: ProfileDocument,
    selection_lease: Box<dyn ActiveProfileReferenceLease>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
}

impl ActiveProfileSelectionPreflight {
    pub fn capability_provider(&self) -> Arc<dyn CoreCapabilityProvider> {
        Arc::clone(&self.capability_provider)
    }

    /// Refresh the exact document identity after an authorized runtime
    /// preparation wrote the same selected profile while the reference lease
    /// remained held.
    pub fn refresh_document(&mut self, document: ProfileDocument) -> Result<(), PortError> {
        validate_profile_identity(&self.reference, &document)?;
        self.document = document;
        Ok(())
    }
}

impl SelectedActiveProfileBackend {
    pub fn capability_provider(&self) -> Arc<dyn CoreCapabilityProvider> {
        Arc::clone(&self.capability_provider)
    }

    /// Finalize a cross-engine reservation only after the product has built
    /// the candidate CoreApplication. A mismatching candidate is closed and
    /// must produce its exact Stopped/no-session receipt before the reservation
    /// can be discarded.
    pub async fn into_owner_binding_for_application(
        self,
        application: &infiltrator_application::core_application::CoreApplication,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        match self.owner_admission {
            SelectedOwnerAdmission::SelectionLease(selection_lease) => {
                let snapshot = application.snapshot().lifecycle_snapshot();
                if classify_candidate_application(
                    &snapshot,
                    &self.core_identity,
                    self.initial_generation,
                ) != CandidateApplicationAdmission::ExactUnstarted
                {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::InvalidState,
                        "constructed CoreApplication does not match the selected backend identity and initial state",
                        false,
                    )));
                }
                selection_lease
                    .into_owner_binding(self.core_identity, self.initial_generation)
                    .await
            }
            SelectedOwnerAdmission::SwitchReservation(binding) => {
                let before = application.snapshot().lifecycle_snapshot();
                let admission = classify_candidate_application(
                    &before,
                    &self.core_identity,
                    self.initial_generation,
                );
                match admission {
                    CandidateApplicationAdmission::ExactUnstarted => {
                        return binding
                            .finalize(self.core_identity, self.initial_generation)
                            .await;
                    }
                    CandidateApplicationAdmission::ForeignUnstarted
                    | CandidateApplicationAdmission::StartedOrUnknown => {}
                }

                application
                    .close()
                    .await
                    .map_err(|failure| PortError::Rejected(failure))?;
                let terminal = application.snapshot().lifecycle_snapshot();
                verify_candidate_terminal(&before, &terminal)?;
                match admission {
                    CandidateApplicationAdmission::ExactUnstarted => unreachable!(),
                    CandidateApplicationAdmission::ForeignUnstarted => {
                        binding.discard_unstarted_candidate(&terminal).await?;
                    }
                    CandidateApplicationAdmission::StartedOrUnknown => {}
                }
                Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "constructed CoreApplication identity or initial state does not match the selected backend",
                    false,
                )))
            }
        }
    }

    /// Finalize and install the binding as one product-composition step. If
    /// installation fails, retain a second lease handle until the candidate
    /// application has a matching terminal readback.
    pub async fn install_owner_binding_for_application(
        self,
        application: &infiltrator_application::core_application::CoreApplication,
    ) -> Result<(), PortError> {
        let owner_lease = self.into_owner_binding_for_application(application).await?;
        let retained: Arc<dyn ActiveProfileOwnerLease> = Arc::from(owner_lease);
        let install_lease: Box<dyn ActiveProfileOwnerLease> = Box::new(SharedOwnerLease {
            inner: Arc::clone(&retained),
        });
        if let Err(failure) = application.install_active_profile_owner_binding(install_lease) {
            application.close().await.map_err(PortError::Rejected)?;
            let terminal = application.snapshot().lifecycle_snapshot();
            verify_stopped_owner(retained.identity(), retained.generation(), &terminal)?;
            retained.release_after_terminal(&terminal).await?;
            return Err(PortError::Rejected(failure));
        }
        Ok(())
    }

    /// Attach the same owner-authored capability provider that authorized the
    /// selected target to the new CoreApplication before it can accept work.
    pub fn install_core_capabilities(
        &self,
        application: &infiltrator_application::core_application::CoreApplication,
    ) -> Result<(), Failure> {
        application.install_core_capability_application(CoreCapabilityApplication::new(Arc::clone(
            &self.capability_provider,
        )))
    }
}

pub struct ActiveProfileBackendSelector {
    references: Arc<dyn ActiveProfileReferenceStore>,
    profile_stores: HashMap<EngineId, Arc<dyn ProfileStore>>,
    factory: Arc<dyn EngineBackendFactory>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
    selection_gate: tokio::sync::Mutex<()>,
}

struct ReferencedProfile {
    reference: ActiveProfileReference,
    store: Arc<dyn ProfileStore>,
}

struct ValidatedProfile {
    reference: ActiveProfileReference,
    workspace: ProfileWorkspace,
}

struct AuthorizedProfile {
    reference: ActiveProfileReference,
    document: ProfileDocument,
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

fn verify_candidate_terminal(
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

fn classify_candidate_application(
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

/// Resolve and hold the unique product reference before host/runtime work.
/// The returned lease blocks reference writes until the selected provider has
/// been composed and its owner capability gate installed.
pub async fn preflight_active_profile_reference(
    references: Arc<dyn ActiveProfileReferenceStore>,
    profile_stores: impl IntoIterator<Item = Arc<dyn ProfileStore>>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
) -> Result<ActiveProfileSelectionPreflight, PortError> {
    let selection_lease = references.acquire_provider_selection_lease().await?;
    let reference = match selection_lease.state() {
        ActiveProfileReferenceState::Unselected { .. } => {
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "no product-wide active profile reference is selected",
            ));
        }
        ActiveProfileReferenceState::Selected(reference) => reference.clone(),
    };
    let profile_stores: HashMap<EngineId, Arc<dyn ProfileStore>> = profile_stores
        .into_iter()
        .map(|store| (store.engine_id(), store))
        .collect();
    let store = profile_stores.get(&reference.engine_id).ok_or_else(|| {
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
    let workspace = store.load_workspace(&reference.profile_id).await?;
    validate_profile_identity(&reference, &workspace.document)?;
    authorize_lifecycle_start(&capabilities, &reference.engine_id)?;
    Ok(ActiveProfileSelectionPreflight {
        reference,
        document: workspace.document,
        selection_lease,
        capability_provider: capabilities,
    })
}

pub fn authorize_lifecycle_start(
    capabilities: &Arc<dyn CoreCapabilityProvider>,
    engine_id: &EngineId,
) -> Result<(), PortError> {
    let matrix = CoreCapabilityApplication::new(Arc::clone(capabilities))
        .matrix_for(engine_id, None)
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
        ) => Ok(()),
        (status, _) => Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            format!(
                "Core start is not evidenced for engine {} on the current target: {status:?}",
                engine_id.as_str()
            ),
        )),
    }
}

fn validate_profile_identity(
    reference: &ActiveProfileReference,
    document: &ProfileDocument,
) -> Result<(), PortError> {
    let identity = &document.identity;
    if identity.engine_id != reference.engine_id || identity.profile_id != reference.profile_id {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!(
                "active profile reference {}/{} resolved to document {}/{}",
                reference.engine_id.as_str(),
                reference.profile_id.as_str(),
                identity.engine_id.as_str(),
                identity.profile_id.as_str()
            ),
            false,
        )));
    }
    Ok(())
}

async fn verify_target_document(
    store: &dyn ProfileStore,
    reference: &ActiveProfileReference,
    expected: &ProfileDocument,
) -> Result<(), PortError> {
    let workspace = store.load_workspace(&reference.profile_id).await?;
    validate_profile_identity(reference, &workspace.document)?;
    if &workspace.document != expected {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "target profile document changed after active-profile preflight",
            true,
        )));
    }
    Ok(())
}

fn verify_stopped_owner(
    identity: &CoreIdentity,
    generation: CoreGeneration,
    terminal: &CoreLifecycleSnapshot,
) -> Result<(), PortError> {
    let is_exact_stopped_owner = terminal
        .identity
        .as_ref()
        .is_some_and(|terminal_identity| identity.same_instance(terminal_identity))
        && terminal.generation == generation.value()
        && terminal.lifecycle == CoreLifecycle::Stopped
        && terminal.session_token.is_none();
    match is_exact_stopped_owner {
        true => Ok(()),
        false => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            format!(
                "previous Core owner did not reach its exact Stopped terminal (expected {} instance {} generation {})",
                identity.engine.as_str(),
                identity.instance.as_str(),
                generation.value()
            ),
            true,
        ))),
    }
}

fn capability_failure(failure: Failure) -> PortError {
    if failure.code == ErrorCode::Unsupported {
        PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            failure.message,
        )
    } else {
        PortError::Rejected(failure)
    }
}
