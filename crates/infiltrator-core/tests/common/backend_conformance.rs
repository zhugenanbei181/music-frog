//! Product wrapper over the neutral adapter conformance kit.
//!
//! The neutral checks live in `lurker_engine_runtime::conformance`. This file
//! keeps only product-owned facts: reflection operation declarations, evidence
//! locks, the selected profile store, the native configuration transaction and
//! the unsupported-live-save assertion.

use async_trait::async_trait;
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationSupport, ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreOperation, EngineBinaryTarget, OutboundOperation, ProxyControlOperation,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::engine_backend::{
    EngineBackendOwnerScopeBinding, EngineBackendPorts, EngineOperationPorts,
};
use lurker_core_ports::outbound::OutboundProbeRequest;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_engine_runtime::conformance::case::{
    AdapterConformanceCase, DetailsBehavior, LifecycleExpectation, ProbeBehavior, RoutingBehavior,
};
use lurker_engine_runtime::conformance::failure::{ConformanceCheck, ConformanceFailure};
use lurker_engine_runtime::conformance::group::ConformanceGroup;
use lurker_engine_runtime::conformance::runner::{
    AdapterConformanceRunningCheck, run_adapter_conformance_with_running_check,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

#[path = "backend_conformance_profile.rs"]
pub(crate) mod profile_support;

pub const REAL_READINESS_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationDeclarationState {
    Supported,
    OwnerManaged,
    EvidenceOpen,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclaredOperationExpectation {
    pub operation: CoreOperation,
    pub state: OperationDeclarationState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunningProfileTransactionBehavior {
    Unsupported,
    NotExercised,
}

#[derive(Clone, Debug)]
pub struct BackendConformanceCase {
    pub engine: EngineId,
    pub expected_version: CoreVersion,
    pub binary_target: EngineBinaryTarget,
    pub adapter_revision: AdapterRevision,
    /// Product declaration states are asserted separately from direct adapter
    /// behavior. A test-only fixture can exercise a native binary without
    /// making that operation available through a production factory.
    pub declared_operations: Vec<DeclaredOperationExpectation>,
    pub valid_profile: Vec<u8>,
    pub invalid_profile: Vec<u8>,
    /// Syntactically valid in the neutral codec but rejected by the engine's
    /// native validator, so this distinguishes native validation from JSON/YAML parsing.
    pub native_invalid_profile: Vec<u8>,
    pub collection_id: OutboundId,
    pub initial_member_id: OutboundId,
    pub alternate_member_id: OutboundId,
    pub probe_outbound_id: OutboundId,
    pub probe_request: OutboundProbeRequest,
    pub details: DetailsBehavior,
    pub probe: ProbeBehavior,
    pub routing: RoutingBehavior,
    pub running_profile_transaction: RunningProfileTransactionBehavior,
}

impl BackendConformanceCase {
    pub(super) fn adapter_case(&self, scope: &ProfileDocumentCodecScope) -> AdapterConformanceCase {
        AdapterConformanceCase {
            engine: self.engine.clone(),
            expected_version: self.expected_version.clone(),
            profile_format: scope.format_id.clone(),
            profile_schema: scope.schema_version,
            valid_document: self.valid_profile.clone(),
            invalid_document: self.invalid_profile.clone(),
            native_invalid_document: self.native_invalid_profile.clone(),
            collection_id: self.collection_id.clone(),
            initial_member_id: self.initial_member_id.clone(),
            alternate_member_id: self.alternate_member_id.clone(),
            probe_outbound_id: self.probe_outbound_id.clone(),
            probe_request: self.probe_request.clone(),
            details: self.details,
            probe: self.probe,
            routing: self.routing.clone(),
            lifecycle: LifecycleExpectation::ManagedStartStop,
            readiness_timeout: REAL_READINESS_TIMEOUT,
        }
    }
}

pub(super) struct BackendConformancePorts<'a> {
    pub backend: &'a EngineBackendPorts,
    pub configuration_transaction: &'a dyn ProfileConfigurationTransactionPort,
    pub profile_store: &'a dyn ProfileStore,
    pub transaction_owner: &'a dyn profile_support::ConformanceLifecycleControl,
}

/// Rebuild the neutral core bundle from a product provider's already-composed
/// ports.
///
/// Product bundles bind their owner-scope source when the host attaches the
/// single Core owner, so a second `EngineBackendPorts::new` cannot reuse that
/// binding. The conformance runner passes explicit scopes to the outbound and
/// routing ports, and the fresh fence is intentionally left unbound.
pub fn conformance_engine_backend(
    engine: &EngineId,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    configuration: Arc<dyn ProfileDocumentCodecPort>,
    operations: EngineOperationPorts,
) -> Result<EngineBackendPorts, String> {
    let operations = match operations {
        EngineOperationPorts::Available {
            outbound,
            routing_intent,
            ..
        } => EngineOperationPorts::available(
            outbound,
            routing_intent,
            Arc::new(EngineBackendOwnerScopeBinding::new(engine.clone())),
        ),
        EngineOperationPorts::Unsupported { reason } => EngineOperationPorts::unsupported(reason),
    };
    EngineBackendPorts::new(
        engine.clone(),
        process,
        readiness,
        configuration,
        operations,
    )
    .map_err(|error| {
        format!(
            "construct conformance engine bundle for {}: {error}",
            engine.as_str()
        )
    })
}

pub async fn run_backend_conformance(
    case: &BackendConformanceCase,
    declarations: &ValidatedAdapterOperationDeclarations,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    verify_operation_declarations(case, declarations)?;
    verify_engine_bindings(case, ports)?;
    profile_support::verify_native_profile_behavior(case, ports)
        .await
        .map_err(|error| {
            format!(
                "{} native profile behavior failed: {error}",
                case.engine.as_str()
            )
        })?;

    let adapter_case = case.adapter_case(&ports.backend.configuration().scope());
    let live_save = LiveSaveUnsupportedCheck { case, ports };
    let while_running: Option<&dyn AdapterConformanceRunningCheck> =
        match case.running_profile_transaction {
            RunningProfileTransactionBehavior::Unsupported => Some(&live_save),
            RunningProfileTransactionBehavior::NotExercised => None,
        };
    run_adapter_conformance_with_running_check(
        &adapter_case,
        ports.backend,
        &ConformanceGroup::ALL,
        while_running,
    )
    .await
    .map_err(|failure| {
        format!(
            "{} adapter conformance failed: {failure}",
            case.engine.as_str()
        )
    })
}

fn verify_operation_declarations(
    case: &BackendConformanceCase,
    declarations: &ValidatedAdapterOperationDeclarations,
) -> Result<(), String> {
    if declarations.engine() != &case.engine {
        return Err(format!(
            "{} conformance case received {} operation declarations",
            case.engine.as_str(),
            declarations.engine().as_str()
        ));
    }
    if case.declared_operations.is_empty() {
        return Err("conformance case has no declared operation expectations".to_owned());
    }
    let mut unique_operations = HashSet::new();
    for expected in &case.declared_operations {
        if !unique_operations.insert(expected.operation) {
            return Err(format!(
                "conformance operation expectation is duplicated: {:?}",
                expected.operation
            ));
        }
        let support = declarations.support(expected.operation);
        let actual_state = match support {
            AdapterOperationSupport::Supported { .. } => OperationDeclarationState::Supported,
            AdapterOperationSupport::OwnerManaged { .. } => OperationDeclarationState::OwnerManaged,
            AdapterOperationSupport::EvidenceOpen { .. } => OperationDeclarationState::EvidenceOpen,
            AdapterOperationSupport::Unsupported { .. } => OperationDeclarationState::Unsupported,
        };
        if actual_state != expected.state {
            return Err(format!(
                "{} {:?} declaration is {actual_state:?}, expected {:?}",
                case.engine.as_str(),
                expected.operation,
                expected.state
            ));
        }
        let has_evidence = declarations.has_verified_support_for_scope(
            expected.operation,
            &case.expected_version,
            case.binary_target,
            case.adapter_revision,
        );
        match expected.state {
            OperationDeclarationState::Supported if !has_evidence => {
                return Err(format!(
                    "{} {:?} lacks evidence for version {}, target {:?}, revision {}",
                    case.engine.as_str(),
                    expected.operation,
                    case.expected_version.raw(),
                    case.binary_target,
                    case.adapter_revision.value()
                ));
            }
            OperationDeclarationState::Supported => {}
            OperationDeclarationState::OwnerManaged
            | OperationDeclarationState::EvidenceOpen
            | OperationDeclarationState::Unsupported => {}
        }
    }
    Ok(())
}

fn verify_engine_bindings(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    if ports.backend.engine_id() != &case.engine {
        return Err(format!(
            "backend engine {} does not match conformance engine {}",
            ports.backend.engine_id().as_str(),
            case.engine.as_str()
        ));
    }
    let process_identity = ports
        .backend
        .process()
        .core_identity()
        .ok_or_else(|| "process adapter has no owner-authored CoreIdentity".to_owned())?;
    if process_identity.engine != case.engine {
        return Err(format!(
            "process engine {} does not match conformance engine {}",
            process_identity.engine.as_str(),
            case.engine.as_str()
        ));
    }
    let transaction_owner_identity = ports
        .transaction_owner
        .core_identity()
        .ok_or_else(|| "configuration transaction owner has no Core identity".to_owned())?;
    if transaction_owner_identity.engine != process_identity.engine
        || transaction_owner_identity.instance != process_identity.instance
    {
        return Err(format!(
            "{} configuration transaction owner does not match the provider process instance",
            case.engine.as_str()
        ));
    }
    let codec_engine = ports.backend.configuration().scope().engine_id;
    if codec_engine != case.engine {
        return Err(format!(
            "document codec engine {} does not match conformance engine {}",
            codec_engine.as_str(),
            case.engine.as_str()
        ));
    }
    if ports.configuration_transaction.scope().document.engine_id != case.engine {
        return Err(format!(
            "native transaction engine {} does not match conformance engine {}",
            ports
                .configuration_transaction
                .scope()
                .document
                .engine_id
                .as_str(),
            case.engine.as_str()
        ));
    }
    if ports.profile_store.engine_id() != case.engine {
        return Err(format!(
            "profile store engine {} does not match conformance engine {}",
            ports.profile_store.engine_id().as_str(),
            case.engine.as_str()
        ));
    }
    for id in [
        &case.collection_id,
        &case.initial_member_id,
        &case.alternate_member_id,
        &case.probe_outbound_id,
    ] {
        if id.engine_id() != &case.engine {
            return Err(format!(
                "outbound ID {} is not bound to engine {}",
                id.value(),
                case.engine.as_str()
            ));
        }
    }
    Ok(())
}

struct LiveSaveUnsupportedCheck<'a> {
    case: &'a BackendConformanceCase,
    ports: &'a BackendConformancePorts<'a>,
}

#[async_trait]
impl AdapterConformanceRunningCheck for LiveSaveUnsupportedCheck<'_> {
    async fn check(&self) -> Result<(), ConformanceFailure> {
        self.ports
            .transaction_owner
            .update_observed_lifecycle(CoreLifecycle::Running);
        let before = self
            .ports
            .profile_store
            .load_active_workspace()
            .await
            .map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RunningObserver,
                    "the active workspace is readable",
                    "the workspace read before the live-save check failed",
                    &error,
                )
            })?;
        let expected_reference = match self
            .ports
            .profile_store
            .read_active_profile_reference()
            .await
            .map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RunningObserver,
                    "the active reference is readable",
                    "the active reference read before the live-save check failed",
                    &error,
                )
            })? {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::RunningObserver,
                    "a selected active profile reference",
                    "unselected",
                    "the live-save check has no selected profile reference",
                ));
            }
        };
        let result = self
            .ports
            .configuration_transaction
            .transact(ProfileConfigurationTransactionRequest {
                expected_reference,
                expected: before.identity(),
                update: ProfileWorkspaceUpdate {
                    purpose: ProfileWorkspacePurpose::Derived,
                    document_bytes: self.case.valid_profile.clone(),
                    sidecar_bytes: before.sidecar_bytes.clone(),
                },
                authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
                strategy: ApplyStrategy::PreferReload,
            })
            .await;
        match result {
            Err(error) if error.error_code() == ErrorCode::Unsupported => {}
            Err(error) => {
                return Err(ConformanceFailure::adapter_error(
                    ConformanceCheck::RunningObserver,
                    "a typed Unsupported error",
                    error.to_string(),
                    "the live profile save returned the wrong typed error",
                ));
            }
            Ok(_) => {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::RunningObserver,
                    "a typed Unsupported error",
                    "live save accepted",
                    "the live profile save was accepted despite the provider contract",
                ));
            }
        }
        let after = self
            .ports
            .profile_store
            .load_active_workspace()
            .await
            .map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RunningObserver,
                    "the active workspace is readable",
                    "the workspace read after the live-save check failed",
                    &error,
                )
            })?;
        if after != before {
            return Err(ConformanceFailure::state(
                ConformanceCheck::RunningObserver,
                "an unchanged active workspace",
                "workspace changed",
                "an unsupported live profile save changed the workspace",
            ));
        }
        Ok(())
    }
}

pub fn expected_support(
    declarations: &ValidatedAdapterOperationDeclarations,
    operation: CoreOperation,
) -> OperationDeclarationState {
    match declarations.support(operation) {
        AdapterOperationSupport::Supported { .. } => OperationDeclarationState::Supported,
        AdapterOperationSupport::OwnerManaged { .. } => OperationDeclarationState::OwnerManaged,
        AdapterOperationSupport::EvidenceOpen { .. } => OperationDeclarationState::EvidenceOpen,
        AdapterOperationSupport::Unsupported { .. } => OperationDeclarationState::Unsupported,
    }
}

pub fn assert_evidence_open_or_unsupported(
    declarations: &ValidatedAdapterOperationDeclarations,
    operation: CoreOperation,
) {
    assert!(matches!(
        declarations.support(operation),
        AdapterOperationSupport::EvidenceOpen { .. } | AdapterOperationSupport::Unsupported { .. }
    ));
    assert!(
        declarations
            .verified_supported_evidence(operation)
            .is_none()
    );
}

pub fn assert_declared_operation_differences_match_case(
    case: &BackendConformanceCase,
    declarations: &ValidatedAdapterOperationDeclarations,
) {
    let probe_operation = CoreOperation::Outbound(OutboundOperation::Probe);
    let probe_support = expected_support(declarations, probe_operation);
    match case.probe {
        ProbeBehavior::Measured => assert_eq!(
            probe_support,
            OperationDeclarationState::Supported,
            "measured probe behavior must have declared operation support"
        ),
        ProbeBehavior::Unsupported => assert_eq!(
            probe_support,
            OperationDeclarationState::Unsupported,
            "unsupported probe behavior must stay outside declared support"
        ),
    }

    let routing_read = CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent);
    let routing_read_support = expected_support(declarations, routing_read);
    match &case.routing {
        RoutingBehavior::Writable { .. } => assert_eq!(
            routing_read_support,
            OperationDeclarationState::Supported,
            "writable routing behavior requires a supported read operation"
        ),
        RoutingBehavior::ReadOnlyUnsupported {
            initial: RoutingIntent::EngineDefined(_),
            ..
        } => assert_eq!(
            routing_read_support,
            OperationDeclarationState::Unsupported,
            "a standalone engine-defined read is not a supported Core operation before owner composition"
        ),
        RoutingBehavior::ReadOnlyUnsupported { .. } => {
            assert_eq!(routing_read_support, OperationDeclarationState::Unsupported)
        }
    }

    let routing_write = CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent);
    let routing_support = expected_support(declarations, routing_write);
    match &case.routing {
        RoutingBehavior::Writable { .. } => assert_eq!(
            routing_support,
            OperationDeclarationState::Supported,
            "writable routing behavior must have declared operation support"
        ),
        RoutingBehavior::ReadOnlyUnsupported { .. } => assert_eq!(
            routing_support,
            OperationDeclarationState::Unsupported,
            "read-only routing behavior must remain declared unsupported"
        ),
    }
}
