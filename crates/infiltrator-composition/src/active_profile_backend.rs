//! Resolve the unique active profile reference before composing one Core backend.

use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState,
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
    ActiveProfileOwnerLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
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
    _selection_lease: Box<dyn ActiveProfileReferenceLease>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
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

    pub fn into_owner_binding(self) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        let identity = self.core_identity.clone();
        let generation = self.initial_generation;
        self._selection_lease
            .into_owner_binding(identity, generation)
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

struct OwnerTerminalProfile {
    reference: ActiveProfileReference,
    document: ProfileDocument,
    previous_owner: PreviousCoreDisposition,
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

    /// Resolve the global reference, validate its exact document and target
    /// capability, close the prior owner, then and only then dispatch one
    /// provider. Per-engine `ProfileStore::get_current` is never consulted.
    pub async fn select_backend(
        &self,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
        let selection_lease = self.references.acquire_provider_selection_lease().await?;
        let referenced = self
            .resolve_reference(selection_lease.state().clone())
            .await?;
        let validated = self.load_and_validate_document(referenced).await?;
        let authorized = self.authorize_target(validated).await?;
        let owner_terminal = self
            .terminate_previous_owner(authorized, previous_owner)
            .await?;
        self.compose_backend(
            owner_terminal,
            selection_lease,
            Arc::clone(&self.capabilities),
        )
        .await
    }

    /// Finish a product-root preflight after binary/host preparation. The
    /// supplied exclusive lease keeps the exact reference stable through the
    /// previous-owner terminal and provider creation.
    pub async fn select_backend_from_preflight(
        &self,
        preflight: ActiveProfileSelectionPreflight,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let _writer = self.selection_gate.lock().await;
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
        let owner_terminal = self
            .terminate_previous_owner(authorized, previous_owner)
            .await?;
        self.compose_backend(owner_terminal, selection_lease, capability_provider)
            .await
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

    async fn terminate_previous_owner(
        &self,
        authorized: AuthorizedProfile,
        previous_owner: &PreviousCoreOwner,
    ) -> Result<OwnerTerminalProfile, PortError> {
        let previous_owner = match previous_owner {
            PreviousCoreOwner::Unowned => PreviousCoreDisposition::NoPriorOwner,
            PreviousCoreOwner::Owned(owner) => {
                let before = owner.lifecycle_snapshot();
                let expected_identity = before.identity.clone().ok_or_else(|| {
                    PortError::unsupported(
                        infiltrator_contract::capability::Capability::CoreLifecycle,
                        "previous Core owner has no typed identity to terminate",
                    )
                })?;
                let expected_generation = CoreGeneration::new(before.generation);
                let after = owner.terminate_owner().await?;
                verify_stopped_owner(&expected_identity, expected_generation, &after)?;
                PreviousCoreDisposition::Stopped {
                    identity: expected_identity,
                    generation: expected_generation,
                }
            }
        };

        Ok(OwnerTerminalProfile {
            reference: authorized.reference,
            document: authorized.document,
            previous_owner,
        })
    }

    async fn compose_backend(
        &self,
        owner_terminal: OwnerTerminalProfile,
        selection_lease: Box<dyn ActiveProfileReferenceLease>,
        capability_provider: Arc<dyn CoreCapabilityProvider>,
    ) -> Result<SelectedActiveProfileBackend, PortError> {
        let backend = self
            .factory
            .backend_ports(&owner_terminal.reference.engine_id)?;
        let core_identity = backend
            .process()
            .core_identity()
            .ok_or_else(|| PortError::Failed("selected backend has no Core identity".to_owned()))?;
        if core_identity.engine != owner_terminal.reference.engine_id
            || backend.engine_id() != &owner_terminal.reference.engine_id
        {
            return Err(PortError::Failed(format!(
                "selected profile engine {} does not match backend identity {}",
                owner_terminal.reference.engine_id.as_str(),
                core_identity.engine.as_str()
            )));
        }

        Ok(SelectedActiveProfileBackend {
            reference: owner_terminal.reference,
            document: owner_terminal.document,
            backend,
            core_identity,
            initial_generation: CoreGeneration::new(0),
            previous_owner: owner_terminal.previous_owner,
            _selection_lease: selection_lease,
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

fn verify_stopped_owner(
    identity: &CoreIdentity,
    generation: CoreGeneration,
    terminal: &CoreLifecycleSnapshot,
) -> Result<(), PortError> {
    let is_exact_stopped_owner = terminal.identity.as_ref() == Some(identity)
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
