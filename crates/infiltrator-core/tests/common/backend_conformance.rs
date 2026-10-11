use async_trait::async_trait;
use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationSupport, ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterRevision, CoreOperation, EngineBinaryTarget, OutboundOperation, ProxyControlOperation,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::outbound::details::OutboundDetailsState;
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, OutboundScopeSource};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use std::time::Duration;
use tokio::time::{sleep, timeout};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeProfileFixtureApplyOutcome {
    Applied { readback: Vec<u8> },
    Rejected { reason: String },
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
}

#[async_trait]
pub trait NativeProfileFixturePort: Send + Sync {
    fn engine(&self) -> EngineId;

    async fn validate_native(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, String>;

    /// Test harness boundary for profile-fixture setup. Each implementation
    /// performs its available native check, installs the isolated fixture,
    /// and returns exact bytes read back from the owned file. This does not
    /// implement or exercise CORE-013.4's production typed transaction port,
    /// and does not change capability evidence declarations.
    async fn apply_and_readback(
        &self,
        raw_bytes: &[u8],
    ) -> Result<NativeProfileFixtureApplyOutcome, String>;

    async fn readback(&self) -> Result<Vec<u8>, String>;
}

pub struct BackendConformancePorts<'a> {
    pub process: &'a dyn CoreProcess,
    pub readiness: &'a dyn CoreReadiness,
    pub codec: &'a dyn ProfileDocumentCodecPort,
    pub native_profile: &'a dyn NativeProfileFixturePort,
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
    verify_native_profile_behavior(case, ports).await?;

    match ports.process.start().await {
        Ok(()) => {}
        Err(start_error) => {
            let stop_result = ports.process.stop().await;
            let stopped_result = verify_stopped(ports.process).await;
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

    let running_result = run_while_process_is_running(case, ports).await;
    let stop_result = ports.process.stop().await;
    let stopped_result = verify_stopped(ports.process).await;
    combine_run_and_cleanup(running_result, stop_result, stopped_result)
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
    let mut unique_operations = std::collections::HashSet::new();
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
    let codec_engine = ports.codec.scope().engine_id;
    if codec_engine != case.engine {
        return Err(format!(
            "document codec engine {} does not match conformance engine {}",
            codec_engine.as_str(),
            case.engine.as_str()
        ));
    }
    if ports.native_profile.engine() != case.engine {
        return Err(format!(
            "native profile engine {} does not match conformance engine {}",
            ports.native_profile.engine().as_str(),
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

async fn verify_native_profile_behavior(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    match ports
        .native_profile
        .validate_native(&case.valid_profile)
        .await
        .map_err(|error| {
            format!(
                "{} native validation of valid config failed: {error}",
                case.engine.as_str()
            )
        })? {
        DocumentValidation::Valid => {}
        other => {
            return Err(format!(
                "valid {} profile was rejected by its native validator: {other:?}",
                case.engine.as_str()
            ));
        }
    }

    match ports.codec.inspect(&case.native_invalid_profile) {
        Ok(DocumentValidation::Valid) => {}
        Ok(other) => {
            return Err(format!(
                "native-invalid {} fixture must pass neutral syntax inspection: {other:?}",
                case.engine.as_str()
            ));
        }
        Err(error) => {
            return Err(format!(
                "native-invalid {} fixture failed neutral syntax inspection: {error}",
                case.engine.as_str()
            ));
        }
    }

    let apply_outcome = ports
        .native_profile
        .apply_and_readback(&case.valid_profile)
        .await
        .map_err(|error| {
            format!(
                "{} native config check/apply/readback failed: {error}",
                case.engine.as_str()
            )
        })?;
    match apply_outcome {
        NativeProfileFixtureApplyOutcome::Applied { readback }
            if readback == case.valid_profile => {}
        NativeProfileFixtureApplyOutcome::Applied { .. } => {
            return Err(format!(
                "{} native config readback differs from submitted bytes",
                case.engine.as_str()
            ));
        }
        NativeProfileFixtureApplyOutcome::Rejected { reason } => {
            return Err(format!(
                "native fixture rejected valid {} config: {reason}",
                case.engine.as_str()
            ));
        }
    }

    match ports
        .native_profile
        .validate_native(&case.native_invalid_profile)
        .await
        .map_err(|error| {
            format!(
                "{} native validation of semantic-invalid config failed: {error}",
                case.engine.as_str()
            )
        })? {
        DocumentValidation::Invalid { .. } => {}
        other => {
            return Err(format!(
                "native validator accepted invalid {} config: {other:?}",
                case.engine.as_str()
            ));
        }
    }
    match ports
        .native_profile
        .apply_and_readback(&case.native_invalid_profile)
        .await
    {
        Ok(NativeProfileFixtureApplyOutcome::Rejected { .. }) => {}
        Ok(NativeProfileFixtureApplyOutcome::Applied { .. }) => {
            return Err(format!(
                "native {} fixture applied a config rejected by native validation",
                case.engine.as_str()
            ));
        }
        Err(error) => {
            return Err(format!(
                "native {} fixture failed while applying invalid config: {error}",
                case.engine.as_str()
            ));
        }
    }
    let after_rejection = ports.native_profile.readback().await.map_err(|error| {
        format!(
            "read {} fixture after native rejection: {error}",
            case.engine.as_str()
        )
    })?;
    if after_rejection != case.valid_profile {
        return Err(format!(
            "native rejection of invalid {} config changed fixture bytes",
            case.engine.as_str()
        ));
    }
    Ok(())
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

async fn run_while_process_is_running(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    wait_until_ready(ports.readiness).await?;
    let observed_version = ports
        .readiness
        .active_core_version()
        .await
        .map_err(|error| format!("live version observation failed: {error}"))?
        .ok_or_else(|| "ready core did not report an active engine version".to_owned())?;
    if observed_version != case.expected_version {
        return Err(format!(
            "live version {} differs from locked expected version {}",
            observed_version.raw(),
            case.expected_version.raw()
        ));
    }

    let mut identity = ports
        .process
        .core_identity()
        .ok_or_else(|| "running process lost its CoreIdentity".to_owned())?;
    if identity.engine != case.engine {
        return Err("running process engine identity changed".to_owned());
    }
    if let Some(process_version) = &identity.version
        && process_version != &observed_version
    {
        return Err(format!(
            "process identity version {} differs from live readiness version {}",
            process_version.raw(),
            observed_version.raw()
        ));
    }
    identity.version = Some(observed_version);
    let scope = CoreScope {
        identity,
        generation: CoreGeneration::new(1),
        session_token: None,
    };

    match ports.process.status().await {
        Ok(CoreLifecycle::Running) => {}
        Ok(status) => {
            return Err(format!(
                "ready controller is not backed by a Running process: {status:?}"
            ));
        }
        Err(error) => return Err(format!("process state read failed: {error}")),
    }

    let inventory = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("outbound inventory failed: {error}"))?;
    if inventory.scope() != &scope {
        return Err("outbound inventory escaped the active CoreScope".to_owned());
    }
    let initial_selection = selection_from_inventory(
        &inventory.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if initial_selection != case.initial_member_id {
        return Err(format!(
            "selector {} started on {}, expected {}",
            case.collection_id.value(),
            initial_selection.value(),
            case.initial_member_id.value()
        ));
    }

    verify_scope_fences_have_no_selection_side_effect(case, ports, &scope, &inventory).await?;
    verify_stale_revision_has_no_selection_side_effect(case, ports, &scope, &inventory).await?;
    let selected = apply_selection(case, ports, &scope, &inventory).await?;
    verify_details(case, ports, &scope, selected.revision()).await?;
    verify_probe(case, ports, &scope, selected.revision()).await?;
    verify_selection_restore(case, ports, &scope, &initial_selection).await?;
    verify_routing(case, ports, &scope).await?;
    Ok(())
}

async fn verify_scope_fences_have_no_selection_side_effect(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    active_scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<(), String> {
    let next_generation = active_scope
        .generation
        .value()
        .checked_add(1)
        .ok_or_else(|| "core generation cannot advance for scope-fence check".to_owned())?;
    let mut stale_scope = active_scope.clone();
    stale_scope.generation = CoreGeneration::new(next_generation);
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_scope,
        "stale generation",
    )
    .await?;

    let mut stale_version = active_scope.clone();
    stale_version.identity.version =
        Some(CoreVersion::new("v0.0.0").expect("static stale Core version is valid"));
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_version,
        "stale version",
    )
    .await?;

    let mut stale_session = active_scope.clone();
    stale_session.session_token = Some(SessionToken::new(2));
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_session,
        "stale session",
    )
    .await?;

    let mut foreign_scope = active_scope.clone();
    foreign_scope.identity.engine =
        EngineId::new("conformance-foreign-engine").expect("static foreign engine ID is valid");
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        foreign_scope,
        "foreign engine",
    )
    .await
}

async fn verify_scope_selection_rejection(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    active_scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
    rejected_scope: CoreScope,
    label: &str,
) -> Result<(), String> {
    match ports
        .outbound
        .select(
            rejected_scope,
            inventory.revision(),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
    {
        Err(error) if error.error_code() == ErrorCode::InvalidState => {}
        Err(error) => {
            return Err(format!(
                "{label} selector scope returned the wrong typed error: {error}"
            ));
        }
        Ok(_) => return Err(format!("{label} selector scope was accepted")),
    }

    let after_rejection = ports
        .outbound
        .inventory(active_scope.clone())
        .await
        .map_err(|error| format!("inventory read after {label} rejection failed: {error}"))?;
    if after_rejection.revision() != inventory.revision() {
        return Err(format!(
            "{label} selector scope changed the active inventory revision"
        ));
    }
    let selected = selection_from_inventory(
        &after_rejection.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if selected != case.initial_member_id {
        return Err(format!(
            "{label} selector scope changed the active outbound selection"
        ));
    }
    Ok(())
}

async fn wait_until_ready(readiness: &dyn CoreReadiness) -> Result<(), String> {
    let result = timeout(REAL_READINESS_TIMEOUT, async {
        loop {
            match readiness.probe().await {
                Ok(_) => break,
                Err(_) => sleep(READINESS_RETRY_INTERVAL).await,
            }
        }
    })
    .await;
    match result {
        Ok(()) => Ok(()),
        Err(_) => Err(format!(
            "controller did not become ready within {REAL_READINESS_TIMEOUT:?}"
        )),
    }
}

async fn verify_stale_revision_has_no_selection_side_effect(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<(), String> {
    let stale_value =
        inventory.revision().value().checked_add(1).ok_or_else(|| {
            "inventory revision cannot be advanced for stale-write check".to_owned()
        })?;
    let stale_revision = SnapshotRevision::new(stale_value);
    let stale_write = ports
        .outbound
        .select(
            scope.clone(),
            stale_revision,
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await;
    match stale_write {
        Err(error) if error.error_code() == ErrorCode::InvalidState => {}
        Err(error) => {
            return Err(format!(
                "stale selector revision returned the wrong typed error: {error}"
            ));
        }
        Ok(_) => return Err("stale selector revision was accepted".to_owned()),
    }

    let after_rejection = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read after stale write failed: {error}"))?;
    if after_rejection.revision() != inventory.revision() {
        return Err("stale selector revision changed the active inventory revision".to_owned());
    }
    let selected = selection_from_inventory(
        &after_rejection.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if selected != case.initial_member_id {
        return Err("stale selector revision caused an outbound selection side effect".to_owned());
    }
    Ok(())
}

async fn apply_selection(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<infiltrator_contract::outbound::inventory::OutboundInventorySnapshot, String> {
    let result = ports
        .outbound
        .select(
            scope.clone(),
            inventory.revision(),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
        .map_err(|error| format!("selector write/readback failed: {error}"))?;
    let selected = selection_from_inventory(
        &result.inventory(),
        &case.collection_id,
        &case.alternate_member_id,
    )?;
    if selected != case.alternate_member_id {
        return Err("selector write did not read back the requested member".to_owned());
    }
    Ok(result)
}

async fn verify_details(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<(), String> {
    let details = ports
        .outbound
        .details(scope.clone(), revision)
        .await
        .map_err(|error| format!("outbound details observation failed: {error}"))?;
    if details.scope() != scope {
        return Err("outbound details escaped the active CoreScope".to_owned());
    }
    match (&case.details, details.state()) {
        (DetailsBehavior::Ready, OutboundDetailsState::Ready { details })
            if details
                .iter()
                .any(|detail| detail.id == case.probe_outbound_id) =>
        {
            Ok(())
        }
        (DetailsBehavior::Unsupported, OutboundDetailsState::Unsupported) => Ok(()),
        (expected, actual) => Err(format!(
            "outbound details state {actual:?} does not match expected {expected:?}"
        )),
    }
}

async fn verify_probe(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<(), String> {
    let probe = ports
        .outbound
        .probe(
            scope.clone(),
            revision,
            case.probe_outbound_id.clone(),
            case.probe_request.clone(),
        )
        .await
        .map_err(|error| format!("outbound probe failed: {error}"))?;
    if probe.scope() != scope || probe.outbound_id() != &case.probe_outbound_id {
        return Err("outbound probe result escaped its requested scope or outbound".to_owned());
    }
    match (&case.probe, probe.state()) {
        (ProbeBehavior::Measured, OutboundProbeState::Measured { .. }) => Ok(()),
        (ProbeBehavior::Unsupported, OutboundProbeState::Unsupported) => Ok(()),
        (expected, actual) => Err(format!(
            "outbound probe state {actual:?} does not match expected {expected:?}"
        )),
    }?;
    let after_probe = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read after outbound probe failed: {error}"))?;
    if after_probe.scope() != scope {
        return Err("outbound probe changed the observed CoreScope".to_owned());
    }
    let selected = selection_from_inventory(
        &after_probe.inventory(),
        &case.collection_id,
        &case.alternate_member_id,
    )?;
    if selected != case.alternate_member_id {
        return Err("outbound probe changed the current selector member".to_owned());
    }
    if case.probe == ProbeBehavior::Unsupported && after_probe.revision() != revision {
        return Err("unsupported outbound probe changed the inventory revision".to_owned());
    }
    Ok(())
}

async fn verify_selection_restore(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    original_member: &OutboundId,
) -> Result<(), String> {
    let current = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read before selector restore failed: {error}"))?;
    let restored = ports
        .outbound
        .select(
            scope.clone(),
            current.revision(),
            case.collection_id.clone(),
            original_member.clone(),
        )
        .await
        .map_err(|error| format!("selector restore/readback failed: {error}"))?;
    let selected =
        selection_from_inventory(&restored.inventory(), &case.collection_id, original_member)?;
    if &selected != original_member {
        return Err("selector restore did not read back the original member".to_owned());
    }
    Ok(())
}

async fn verify_routing(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
) -> Result<(), String> {
    match &case.routing {
        RoutingBehavior::Writable { initial, requested } => {
            let observed = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("initial routing observation failed: {error}"))?;
            if &observed != initial {
                return Err(format!(
                    "initial routing intent {observed:?} differs from expected {initial:?}"
                ));
            }
            let changed = ports
                .routing
                .apply(scope, requested.clone())
                .await
                .map_err(|error| format!("routing write/readback failed: {error}"))?;
            if &changed != requested {
                return Err("routing write returned a mismatching readback".to_owned());
            }
            let restored = ports
                .routing
                .apply(scope, initial.clone())
                .await
                .map_err(|error| format!("routing restore/readback failed: {error}"))?;
            if &restored != initial {
                return Err("routing restore returned a mismatching readback".to_owned());
            }
            Ok(())
        }
        RoutingBehavior::ReadOnlyUnsupported { initial, requested } => {
            let observed = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("initial routing observation failed: {error}"))?;
            if &observed != initial {
                return Err(format!(
                    "initial routing intent {observed:?} differs from expected {initial:?}"
                ));
            }
            match ports.routing.apply(scope, requested.clone()).await {
                Err(error) if is_typed_unsupported(&error) => {}
                Err(error) => {
                    return Err(format!(
                        "unsupported routing write returned a non-Unsupported error: {error}"
                    ));
                }
                Ok(_) => return Err("unsupported routing write was accepted".to_owned()),
            }
            let after_rejection = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("routing readback after rejection failed: {error}"))?;
            if &after_rejection != initial {
                return Err(
                    "unsupported routing write changed the observed routing intent".to_owned(),
                );
            }
            Ok(())
        }
    }
}

fn selection_from_inventory(
    state: &OutboundInventoryState,
    collection_id: &OutboundId,
    expected_member: &OutboundId,
) -> Result<OutboundId, String> {
    let OutboundInventoryState::Ready { collections, .. } = state else {
        return Err(format!("outbound inventory was not ready: {state:?}"));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| format!("selector {} was absent", collection_id.value()))?;
    if !collection.member_ids.contains(expected_member) {
        return Err(format!(
            "selector {} does not contain expected member {}",
            collection_id.value(),
            expected_member.value()
        ));
    }
    match &collection.selection {
        OutboundSelectionState::Ready { selected } => Ok(selected.clone()),
        other => Err(format!(
            "selector {} did not expose a ready selection: {other:?}",
            collection_id.value()
        )),
    }
}

fn is_typed_unsupported(error: &PortError) -> bool {
    match error {
        PortError::Unsupported { .. } => true,
        PortError::Rejected(failure) => failure.code == ErrorCode::Unsupported,
        PortError::PermissionDenied(_)
        | PortError::NotFound(_)
        | PortError::Io(_)
        | PortError::Network(_)
        | PortError::Failed(_) => false,
    }
}

async fn verify_stopped(process: &dyn CoreProcess) -> Result<(), String> {
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => Ok(()),
        Ok(status) => Err(format!("process cleanup left lifecycle at {status:?}")),
        Err(error) => Err(format!("process state read after cleanup failed: {error}")),
    }
}

fn combine_run_and_cleanup(
    run_result: Result<(), String>,
    stop_result: Result<(), PortError>,
    stopped_result: Result<(), String>,
) -> Result<(), String> {
    match (run_result, stop_result, stopped_result) {
        (Ok(()), Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(()), Ok(())) => Err(run_error),
        (run_result, stop_result, stopped_result) => Err(format!(
            "conformance failure: {}; stop result: {}; stopped verification: {}",
            run_result.err().unwrap_or_else(|| "passed".to_owned()),
            stop_result
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "passed".to_owned()),
            stopped_result.err().unwrap_or_else(|| "passed".to_owned())
        )),
    }
}

pub fn expected_support(
    declarations: &infiltrator_contract::adapter_operation_declaration::ValidatedAdapterOperationDeclarations,
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
    declarations: &infiltrator_contract::adapter_operation_declaration::ValidatedAdapterOperationDeclarations,
    operation: CoreOperation,
) {
    assert!(matches!(
        declarations.support(operation),
        infiltrator_contract::adapter_operation_declaration::AdapterOperationSupport::EvidenceOpen { .. }
            | infiltrator_contract::adapter_operation_declaration::AdapterOperationSupport::Unsupported { .. }
    ));
    assert!(
        declarations
            .verified_supported_evidence(operation)
            .is_none()
    );
}

pub fn assert_declared_operation_differences_match_case(
    case: &BackendConformanceCase,
    declarations: &infiltrator_contract::adapter_operation_declaration::ValidatedAdapterOperationDeclarations,
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
