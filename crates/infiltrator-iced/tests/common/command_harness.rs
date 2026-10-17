//! test-intent: behavior
//! Hostless command application shared by UI behavior tests.
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::command_application::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, ValidatedCoreCommandScope,
};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::{DocumentInspection, DocumentValidation};
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_contract::yaml_snippets::{SnippetCaret, insert_at_caret};
use infiltrator_iced::state::AppState;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentSnippetInsertion, ProfileDocumentSnippetOutcome,
};
use std::collections::VecDeque;
use std::iter::empty;
use std::sync::{Arc, Mutex};

pub struct HostlessProcess;

#[async_trait::async_trait]
impl CoreProcess for HostlessProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid test engine"),
            CoreInstanceId::new("iced-test-core").expect("valid test instance"),
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

#[async_trait::async_trait]
impl CoreReadiness for HostlessProcess {
    async fn probe(&self) -> Result<String, PortError> {
        Err(PortError::Failed("unexpected readiness probe".into()))
    }
}

#[derive(Default)]
pub struct RecordingHandler(
    pub Mutex<Vec<CommandIntent>>,
    pub Mutex<Option<Failure>>,
    pub Mutex<VecDeque<CommandOutput>>,
    pub Mutex<VecDeque<Result<DocumentInspection, Failure>>>,
    pub Mutex<VecDeque<Result<ProfileDocumentSnippetOutcome, Failure>>>,
    pub Mutex<VecDeque<Result<(), Failure>>>,
);

impl CommandHandler for RecordingHandler {
    fn inspect_profile_buffer(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.3
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(DocumentInspection::Inspected(DocumentValidation::Valid)))
    }

    fn insert_profile_snippet(
        &self,
        _identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        if let Some(result) = self.4.lock().unwrap().pop_front() {
            return result;
        }
        let insertion = insert_at_caret(content, snippet_id, SnippetCaret { line, column })
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        Ok(ProfileDocumentSnippetOutcome::Inserted(
            ProfileDocumentSnippetInsertion {
                snippet_id: insertion.snippet_id.to_owned(),
                content: insertion.content,
                cursor_line: insertion.cursor_line,
                cursor_column: insertion.cursor_column,
                validation: DocumentValidation::Valid,
            },
        ))
    }

    fn inspect_mixin_draft(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.5.lock().unwrap().pop_front().unwrap_or(Ok(()))
    }

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

    fn handle_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandFuture {
        self.handle(intent)
    }

    fn handle_output_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        let result = self.handle(intent);
        let output = self
            .2
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(CommandOutput::Unit);
        Box::pin(async move { result.await.map(|()| output) })
    }
}

struct TestCoreCapabilityProvider {
    operations: Vec<CoreOperation>,
}

impl CoreCapabilityProvider for TestCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = self
            .operations
            .iter()
            .copied()
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

pub(crate) fn test_command_application_for(
    core_operations: impl IntoIterator<Item = CoreOperation>,
    host_capabilities: impl IntoIterator<Item = Capability>,
) -> CommandApplication {
    CommandApplication::new()
        .with_core_capabilities(CoreCapabilityApplication::new(Arc::new(
            TestCoreCapabilityProvider {
                operations: core_operations.into_iter().collect(),
            },
        )))
        .with_host_capabilities(Arc::new(TestHostCapabilityProvider {
            capabilities: host_capabilities.into_iter().collect(),
        }))
}

pub fn core_test_command_application(
    operations: impl IntoIterator<Item = CoreOperation>,
) -> CommandApplication {
    test_command_application_for(operations, empty())
}

pub fn install_test_core_capabilities(
    core: &CoreApplication,
    operations: impl IntoIterator<Item = CoreOperation>,
) {
    let resolver = core_test_command_application(operations).core_capability_application();
    core.install_core_capability_application(resolver)
        .expect("explicit test capabilities install before use");
}

pub fn install_lifecycle_test_capabilities(core: &CoreApplication) {
    install_test_core_capabilities(
        core,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ],
    );
}

pub fn recording_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let handler = Arc::new(RecordingHandler::default());
    let application = CoreApplication::new(
        Arc::new(HostlessProcess),
        Arc::new(HostlessProcess),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(handler.clone());
    (application, handler)
}

pub fn rejecting_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let (application, handler) = recording_application();
    *handler.1.lock().unwrap() = Some(Failure::new(
        ErrorCode::InvalidInput,
        "profile is read-only",
        false,
    ));
    (application, handler)
}

/// Attach a product owner only after this fixture accepts a full surface
/// snapshot carrying that same owner's current identity and generation.
/// Demo snapshots are discarded at the explicit demo-to-product test seam;
/// live snapshots from a different owner remain fenced by the normal reducer.
pub fn attach_product_application_with_accepted_owner_snapshot(
    state: &mut AppState,
    application: CoreApplication,
) {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(
            ErrorCode::NotReady,
            "test fixture has not installed page readers",
            true,
        ),
    );
    attach_product_application_with_accepted_snapshot(state, application, snapshot);
}

/// Attach a product owner and accept a surface read from that same owner.
/// The accepted scope is then captured by AppState's normal immutable client.
pub fn attach_product_application_with_accepted_snapshot(
    state: &mut AppState,
    application: CoreApplication,
    mut snapshot: SurfaceSnapshot,
) {
    if state
        .surface
        .latest()
        .is_some_and(|snapshot| snapshot.origin == SurfaceOrigin::Demo)
    {
        state.surface = Default::default();
    }
    state.shell.demo = false;

    let core = application.snapshot();
    assert!(
        core.scope().is_some(),
        "product command fixture must expose an owner scope"
    );
    // AppState::demo seeds toolkit-local data from a demo owner. This helper
    // creates a fresh product observation seam while retaining that local
    // data for the behavior test; the accepted Core snapshot is still the
    // actual owner snapshot below.
    state.runtime.core_lifecycle = core.lifecycle_snapshot();
    snapshot.origin = SurfaceOrigin::Live;
    snapshot.generation = core.generation;
    snapshot.revision = state.surface.revision().saturating_add(1).max(1);
    snapshot.core = core;

    state.attach_product_application(application);
    assert!(
        state.apply_shared_surface_snapshot(snapshot),
        "fixture must accept its identified owner's surface snapshot before command ingress"
    );
}
