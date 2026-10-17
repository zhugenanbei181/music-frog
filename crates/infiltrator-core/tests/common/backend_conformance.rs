use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationSupport, ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreOperation, EngineBinaryTarget, OutboundOperation, ProxyControlOperation,
};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::details::OutboundDetailsState;
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[path = "backend_conformance_profile.rs"]
pub(crate) mod profile_support;
#[path = "backend_conformance_runtime.rs"]
mod runtime;

pub const REAL_READINESS_TIMEOUT: Duration = Duration::from_secs(10);
const READINESS_RETRY_INTERVAL: Duration = Duration::from_millis(100);

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
pub enum DetailsBehavior {
    Ready,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeBehavior {
    Measured,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutingBehavior {
    Writable {
        initial: RoutingIntent,
        requested: RoutingIntent,
    },
    ReadOnlyUnsupported {
        initial: RoutingIntent,
        requested: RoutingIntent,
    },
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

pub(super) struct BackendConformancePorts<'a> {
    pub process: &'a dyn CoreProcess,
    pub readiness: &'a dyn CoreReadiness,
    pub codec: &'a dyn ProfileDocumentCodecPort,
    pub configuration_transaction: &'a dyn ProfileConfigurationTransactionPort,
    pub profile_store: &'a dyn ProfileStore,
    pub transaction_owner: &'a dyn profile_support::ConformanceLifecycleControl,
    pub outbound: &'a dyn OutboundPort,
    pub routing: &'a dyn RoutingIntentPort,
}

pub async fn run_backend_conformance(
    case: &BackendConformanceCase,
    declarations: &ValidatedAdapterOperationDeclarations,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    verify_operation_declarations(case, declarations)?;
    verify_engine_bindings(case, ports)?;
    verify_initial_process_state(ports.process).await?;
    verify_document_inspection(case, ports)?;
    profile_support::verify_native_profile_behavior(case, ports).await?;

    let start_context = ports
        .process
        .core_identity()
        .map(CoreProcessStartContext::standalone)
        .ok_or_else(|| "backend process has no stable identity for start".to_owned())?;
    match ports.process.start(start_context).await {
        Ok(()) => {}
        Err(start_error) => {
            let stop_result = ports.process.stop().await;
            let stopped_result = runtime::verify_stopped(ports.process).await;
            return match (stop_result, stopped_result) {
                (Ok(()), Ok(())) => Err(format!("process start failed: {start_error}")),
                (Err(stop_error), Ok(())) => Err(format!(
                    "process start failed: {start_error}; cleanup after failed start also failed: {stop_error}"
                )),
                (Ok(()), Err(status_error)) => Err(format!(
                    "process start failed: {start_error}; stopped-state verification failed: {status_error}"
                )),
                (Err(stop_error), Err(status_error)) => Err(format!(
                    "process start failed: {start_error}; cleanup failed: {stop_error}; stopped-state verification failed: {status_error}"
                )),
            };
        }
    }

    let running_result = runtime::run_while_process_is_running(case, ports).await;
    let stop_result = ports.process.stop().await;
    let stopped_result = runtime::verify_stopped(ports.process).await;
    if stopped_result.is_ok() {
        ports
            .transaction_owner
            .update_observed_lifecycle(CoreLifecycle::Stopped);
    }
    runtime::combine_run_and_cleanup(running_result, stop_result, stopped_result)
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
    let process_identity = ports
        .process
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
    let codec_engine = ports.codec.scope().engine_id;
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

fn verify_document_inspection(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    match ports.codec.inspect(&case.valid_profile) {
        Ok(DocumentValidation::Valid) => {}
        Ok(other) => {
            return Err(format!(
                "valid {} profile was not accepted by its codec: {other:?}",
                case.engine.as_str()
            ));
        }
        Err(error) => {
            return Err(format!(
                "valid {} profile inspection failed: {error}",
                case.engine.as_str()
            ));
        }
    }
    match ports.codec.inspect(&case.invalid_profile) {
        Ok(DocumentValidation::Invalid { .. }) => Ok(()),
        Ok(other) => Err(format!(
            "invalid {} profile was not rejected by its codec: {other:?}",
            case.engine.as_str()
        )),
        Err(error) => Err(format!(
            "invalid {} profile inspection failed unexpectedly: {error}",
            case.engine.as_str()
        )),
    }
}

async fn verify_initial_process_state(process: &dyn CoreProcess) -> Result<(), String> {
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => Ok(()),
        Ok(status) => Err(format!(
            "conformance requires an initially stopped process, observed {status:?}"
        )),
        Err(error) => Err(format!("could not observe initial process state: {error}")),
    }
}

pub fn expected_support(
    declarations: &ValidatedAdapterOperationDeclarations,
    operation: CoreOperation,
) -> OperationDeclarationState {
    use infiltrator_contract::adapter_operation_declaration::AdapterOperationSupport;
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
