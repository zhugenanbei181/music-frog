//! test-intent: behavior
//! Hostless command application shared by UI behavior tests.
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::command_application::handler::{CommandFuture, CommandHandler};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_bevy_ui::command::UiCommand;
use infiltrator_bevy_ui::command::UiCommandSink;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreOperation, DnsOperation, EngineCapabilityDecision,
    FakeIpCacheOperation, HostCapabilityDecision,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::{
    identity::SnapshotRevision,
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundUiPreferenceIntent,
        OutboundUiPreferenceRequest, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
    },
};
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use mem::take;
use std::mem;
use std::sync::{Arc, Mutex};

/// Revision-fenced typed preference owner for Bevy native interaction tests.
/// It exercises the same request/readback contract while keeping these UI
/// tests independent from a concrete engine adapter.
#[derive(Clone)]
pub struct TypedOutboundPreferenceSink(Arc<Mutex<TypedPreferenceSinkState>>);

#[derive(Default)]
struct TypedPreferenceSinkState {
    snapshot: Option<OutboundUiPreferencesSnapshot>,
    commands: Vec<UiCommand>,
    events: Vec<CommandExecutedEvent>,
    failure: Option<Failure>,
    next_request_id: u64,
}

impl TypedOutboundPreferenceSink {
    pub fn new(snapshot: OutboundUiPreferencesSnapshot) -> Self {
        Self(Arc::new(Mutex::new(TypedPreferenceSinkState {
            snapshot: Some(snapshot),
            ..Default::default()
        })))
    }

    pub fn preferences(&self) -> OutboundUiPreferencesSnapshot {
        self.0
            .lock()
            .unwrap()
            .snapshot
            .clone()
            .expect("typed preference fixture is initialized")
    }

    pub fn commands(&self) -> Vec<UiCommand> {
        self.0.lock().unwrap().commands.clone()
    }

    pub fn fail_next(&self, failure: Failure) {
        self.0.lock().unwrap().failure = Some(failure);
    }
}

impl UiCommandSink for TypedOutboundPreferenceSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }

    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let UiCommand::ApplyOutboundUiPreference { request } = command.clone() else {
            self.0.lock().unwrap().commands.push(command);
            return None;
        };
        let mut state = self.0.lock().unwrap();
        state.commands.push(command.clone());
        state.next_request_id += 1;
        let request_id = RequestId(state.next_request_id);
        let result = if let Some(failure) = state.failure.take() {
            Err(failure)
        } else {
            apply_typed_preference(
                state
                    .snapshot
                    .as_ref()
                    .expect("typed preference fixture is initialized"),
                &request,
            )
            .map(|snapshot| {
                state.snapshot = Some(snapshot.clone());
                CommandOutput::OutboundUiPreferencesApplied(snapshot)
            })
        };
        state.events.push(CommandExecutedEvent {
            command,
            request_id,
            result,
        });
        Some(request_id)
    }

    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        take(&mut self.0.lock().unwrap().events)
    }
}

fn apply_typed_preference(
    current: &OutboundUiPreferencesSnapshot,
    request: &OutboundUiPreferenceRequest,
) -> Result<OutboundUiPreferencesSnapshot, Failure> {
    if current.revision() != request.expected_revision() {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "typed preference fixture received a stale revision",
            true,
        ));
    }
    request
        .validate_scope(current.scope())
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    let mut search_query = current.search_query.clone();
    let mut favorites = current.favorite_outbound_ids.clone();
    let mut group_order = current.group_order.clone();
    let mut collapsed = current.collapsed_group_ids.clone();
    let mut sort = current.sort_order;
    let mut health = current.health_filter;
    let mut layout = current.layout;
    match request.intent() {
        OutboundUiPreferenceIntent::SetSearchQuery(query) => search_query.clone_from(query),
        OutboundUiPreferenceIntent::SetFavorite { outbound_id, state } => match state {
            OutboundFavoriteState::Pinned => {
                if !favorites.contains(outbound_id) {
                    favorites.push(outbound_id.clone());
                }
            }
            OutboundFavoriteState::Unpinned => favorites.retain(|id| id != outbound_id),
        },
        OutboundUiPreferenceIntent::SetSortOrder(value) => sort = *value,
        OutboundUiPreferenceIntent::SetHealthFilter(value) => health = *value,
        OutboundUiPreferenceIntent::SetLayout(value) => layout = *value,
        OutboundUiPreferenceIntent::SetGroupExpansion {
            group_id,
            expansion,
        } => match expansion {
            OutboundGroupExpansion::Expanded => collapsed.retain(|id| id != group_id),
            OutboundGroupExpansion::Collapsed => {
                if !collapsed.contains(group_id) {
                    collapsed.push(group_id.clone());
                }
            }
        },
        OutboundUiPreferenceIntent::ReorderGroups { group_ids, .. } => {
            group_order.clone_from(group_ids)
        }
        OutboundUiPreferenceIntent::ResetGroupOrder { .. } => group_order.clear(),
    }
    let applied = OutboundUiPreferencesSnapshot::new(
        current.scope().clone(),
        SnapshotRevision::new(current.revision().value() + 1),
        OutboundUiPreferencesValues {
            search_query,
            favorite_outbound_ids: favorites,
            group_order,
            collapsed_group_ids: collapsed,
            sort_order: sort,
            health_filter: health,
            layout,
        },
    )
    .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    if !request.is_confirmed_by(&applied) {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "typed preference fixture could not confirm its request",
            false,
        ));
    }
    Ok(applied)
}

pub struct HostlessProcess;

#[async_trait::async_trait]
impl CoreProcess for HostlessProcess {
    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }
    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

#[async_trait::async_trait]
impl CoreReadiness for HostlessProcess {
    async fn probe(&self) -> Result<String, PortError> {
        Err(PortError::Failed("unexpected readiness probe".into()))
    }
}

pub struct IdentifiedHostlessProcess;

/// A stopped, identified Core owner for behavior tests that persist an active
/// profile without claiming a runtime reload.
pub struct StoppedTestCoreProfileOwner;

#[async_trait::async_trait]
impl ActiveProfileReferenceApplier for StoppedTestCoreProfileOwner {
    fn engine_id(&self) -> Option<EngineId> {
        EngineId::new("mihomo").ok()
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        CoreLifecycle::Stopped
    }

    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        Ok(ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("test engine"),
                CoreInstanceId::new("test-core").expect("test owner instance"),
                Some(CoreVersion::new("1.19.12").expect("test core version")),
            ),
            generation: CoreGeneration::new(1),
        })
    }
}

#[async_trait::async_trait]
impl CoreProcess for IdentifiedHostlessProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid test engine"),
            CoreInstanceId::new("test-core").expect("valid test instance"),
            Some(CoreVersion::new("1.19.12").expect("valid test version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

#[derive(Default)]
pub struct TestCoreCapabilityProvider;

impl CoreCapabilityProvider for TestCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        test_matrix_for(
            engine_id,
            core_version,
            [
                CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
                CoreOperation::Dns(DnsOperation::ApplyConfiguration),
            ],
        )
    }
}

pub struct TestScopedCoreCapabilityProvider {
    operations: Vec<CoreOperation>,
}

impl TestScopedCoreCapabilityProvider {
    pub fn supporting(operations: impl IntoIterator<Item = CoreOperation>) -> Self {
        Self {
            operations: operations.into_iter().collect(),
        }
    }
}

impl CoreCapabilityProvider for TestScopedCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        test_matrix_for(engine_id, core_version, self.operations.iter().copied())
    }
}

fn test_matrix_for(
    engine_id: &EngineId,
    core_version: Option<&CoreVersion>,
    operations: impl IntoIterator<Item = CoreOperation>,
) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
    let scope = CoreCapabilityScope::new(
        engine_id.clone(),
        core_version.cloned(),
        HostKind::Desktop,
        AdapterRevision::new(1),
    );
    let entries = operations
        .into_iter()
        .map(|operation| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        })
        .collect();
    CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
        CoreCapabilityProviderError::MatrixUnavailable {
            reason: error.to_string(),
        }
    })
}

pub fn test_command_application(
    operations: impl IntoIterator<Item = CoreOperation>,
) -> CommandApplication {
    CommandApplication::new().with_core_capabilities(CoreCapabilityApplication::new(Arc::new(
        TestScopedCoreCapabilityProvider::supporting(operations),
    )))
}

pub fn test_command_application_for(
    core_operations: impl IntoIterator<Item = CoreOperation>,
    host_capabilities: impl IntoIterator<Item = Capability>,
) -> CommandApplication {
    test_command_application(core_operations).with_host_capabilities(Arc::new(
        TestHostCapabilityProvider {
            capabilities: host_capabilities.into_iter().collect(),
        },
    ))
}

pub fn install_test_core_capability_application(
    core: &CoreApplication,
    operations: impl IntoIterator<Item = CoreOperation>,
) {
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        TestScopedCoreCapabilityProvider::supporting(operations),
    )))
    .expect("install test core capability resolver");
}

struct TestHostCapabilityProvider {
    capabilities: Vec<Capability>,
}

impl CapabilityProvider for TestHostCapabilityProvider {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        let entries = self
            .capabilities
            .iter()
            .copied()
            .map(|capability| CapabilityStatus {
                capability,
                availability: Availability::Supported,
            })
            .collect();
        CapabilitySnapshot::new(HostKind::Desktop, 1, entries)
    }
}

#[derive(Default)]
pub struct TestDnsHostCapabilityProvider;

impl CapabilityProvider for TestDnsHostCapabilityProvider {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![CapabilityStatus {
                capability: Capability::Dns,
                availability: Availability::Supported,
            }],
        )
    }
}

#[derive(Default)]
pub struct TestCoreLifecycleHostCapabilityProvider;

impl CapabilityProvider for TestCoreLifecycleHostCapabilityProvider {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![CapabilityStatus {
                capability: Capability::CoreLifecycle,
                availability: Availability::Supported,
            }],
        )
    }
}

#[derive(Default)]
pub struct RecordingHandler(pub Mutex<Vec<CommandIntent>>, pub Mutex<Option<Failure>>);

impl CommandHandler for RecordingHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        self.0.lock().unwrap().push(intent);
        let failure = self.1.lock().unwrap().clone();
        Box::pin(async move {
            match failure {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        })
    }
}

pub fn recording_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let handler = Arc::new(RecordingHandler::default());
    let application = CoreApplication::new(
        Arc::new(IdentifiedHostlessProcess),
        Arc::new(HostlessProcess),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(handler.clone());
    (application, handler)
}

pub fn identified_recording_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let handler = Arc::new(RecordingHandler::default());
    let application = CoreApplication::new(
        Arc::new(IdentifiedHostlessProcess),
        Arc::new(HostlessProcess),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(handler.clone());
    (application, handler)
}
