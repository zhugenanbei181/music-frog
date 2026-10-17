//! Resolve the unique active profile reference before composing one Core backend.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityKey, CoreCapabilityStatus, CoreLifecycleOperation,
    CoreOperation, EngineCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{ProfileDocument, ProfileId};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState,
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
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

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

/// A typed owner-switch failure records whether the previous session is still
/// usable, needs durable reservation recovery, has uncertain termination, or
/// has closed and requires a new product session.
#[derive(Debug)]
pub enum ActiveProfileBackendSwitchError {
    BeforeReservation(PortError),
    ReservationPending(PortError),
    OwnerTerminationUnconfirmed(PortError),
    OldOwnerClosed(PortError),
}

impl ActiveProfileBackendSwitchError {
    pub fn phase(&self) -> ActiveProfileBackendSwitchFailurePhase {
        match self {
            Self::BeforeReservation(_) => ActiveProfileBackendSwitchFailurePhase::BeforeReservation,
            Self::ReservationPending(_) => {
                ActiveProfileBackendSwitchFailurePhase::ReservationPending
            }
            Self::OwnerTerminationUnconfirmed(_) => {
                ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
            }
            Self::OldOwnerClosed(_) => ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed,
        }
    }

    pub fn failure(&self) -> &PortError {
        match self {
            Self::BeforeReservation(failure)
            | Self::ReservationPending(failure)
            | Self::OwnerTerminationUnconfirmed(failure)
            | Self::OldOwnerClosed(failure) => failure,
        }
    }

    pub fn into_failure(self) -> PortError {
        match self {
            Self::BeforeReservation(failure)
            | Self::ReservationPending(failure)
            | Self::OwnerTerminationUnconfirmed(failure)
            | Self::OldOwnerClosed(failure) => failure,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveProfileBackendSwitchFailurePhase {
    BeforeReservation,
    ReservationPending,
    OwnerTerminationUnconfirmed,
    OldOwnerClosed,
}

impl fmt::Display for ActiveProfileBackendSwitchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failure = match self {
            Self::BeforeReservation(failure)
            | Self::ReservationPending(failure)
            | Self::OwnerTerminationUnconfirmed(failure)
            | Self::OldOwnerClosed(failure) => failure,
        };
        write!(formatter, "{failure}")
    }
}

impl Error for ActiveProfileBackendSwitchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BeforeReservation(failure)
            | Self::ReservationPending(failure)
            | Self::OwnerTerminationUnconfirmed(failure)
            | Self::OldOwnerClosed(failure) => Some(failure),
        }
    }
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
        context: &CoreProcessStartContext,
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

    /// Release this candidate when host-specific CoreApplication construction
    /// fails before an actor exists. Once an application exists, callers must
    /// close it and present its exact terminal readback instead.
    pub async fn abort_before_core_application(
        self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        match self.owner_admission {
            SelectedOwnerAdmission::SelectionLease(selection_lease) => {
                let state = selection_lease.state().clone();
                drop(selection_lease);
                Ok(state)
            }
            SelectedOwnerAdmission::SwitchReservation(binding) => {
                binding.abort_before_core_application().await
            }
        }
    }

    /// Close a newly constructed but unbound candidate, verify its exact
    /// terminal snapshot, and release the durable switch reservation.
    pub async fn discard_application_candidate(
        self,
        application: &CoreApplication,
    ) -> Result<(), PortError> {
        let before = application.snapshot().lifecycle_snapshot();
        application.close().await.map_err(PortError::Rejected)?;
        let terminal = application.snapshot().lifecycle_snapshot();
        verify_candidate_terminal(&before, &terminal)?;
        match self.owner_admission {
            SelectedOwnerAdmission::SelectionLease(selection_lease) => {
                drop(selection_lease);
                Ok(())
            }
            SelectedOwnerAdmission::SwitchReservation(binding) => binding
                .discard_unstarted_candidate(&terminal)
                .await
                .map(|_| ()),
        }
    }

    /// Finalize a cross-engine reservation only after the product has built
    /// the candidate CoreApplication. A mismatching candidate is closed and
    /// must produce its exact Stopped/no-session receipt before the reservation
    /// can be discarded.
    pub async fn into_owner_binding_for_application(
        self,
        application: &CoreApplication,
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
        application: &CoreApplication,
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
    pub fn install_core_capabilities(&self, application: &CoreApplication) -> Result<(), Failure> {
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
    selection_gate: Mutex<()>,
}

#[path = "active_profile_backend/selector.rs"]
mod selector;
use selector::{classify_candidate_application, verify_candidate_terminal};

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
                Capability::Profiles,
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
            Capability::CoreLifecycle,
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
        PortError::unsupported(Capability::CoreLifecycle, failure.message)
    } else {
        PortError::Rejected(failure)
    }
}
