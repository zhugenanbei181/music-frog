use super::{
    ActiveProfileBackendSelector, ActiveProfileBackendSwitchError,
    ActiveProfileBackendSwitchFailurePhase, PreviousCoreDisposition, PreviousCoreOwner,
};
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, CoreCapabilityEntry, CoreCapabilityMatrix, CoreCapabilityScope,
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::surface::HostKind;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState,
    ActiveProfileReferenceChangedLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};
use infiltrator_ports::engine_backend_factory::{
    EngineBackendPorts, EngineBackendProvider, EngineBackendProviderAvailability,
    EngineOperationPorts,
};
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

#[path = "active_profile_backend_tests/memory_leases.rs"]
mod memory_leases;
use memory_leases::*;

#[path = "active_profile_backend_tests/memory_store.rs"]
mod memory_store;
use memory_store::*;

#[path = "active_profile_backend_tests/fakes.rs"]
mod fakes;
use fakes::*;

#[path = "active_profile_backend_tests/cases_a.rs"]
mod cases_a;

#[path = "active_profile_backend_tests/cases_b.rs"]
mod cases_b;

#[path = "active_profile_backend_tests/abort_candidate.rs"]
mod abort_candidate;
