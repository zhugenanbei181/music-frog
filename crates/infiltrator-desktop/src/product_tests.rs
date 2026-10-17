//! Desktop product-session owner handoff and recovery-state tests.

use super::{
    CoreRecompositionResume, DesktopProductBackendFactory, DesktopProductSession,
    DesktopProductSessionAction, DesktopProductSessionActionFailure,
    DesktopProductSessionAvailability, DesktopProductSessionState, DesktopProductSwitchFailure,
    core_recomposition_resume,
};
use crate::composition::DesktopProductBackend;
use async_trait::async_trait;
use infiltrator_application::command_application::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, ValidatedCoreCommandScope,
};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_application::{SurfacePump, UnavailableSurfaceReader};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision, ProfileOperation,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::connection::{ConnectionPort, UnsupportedConnectionPort};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::resources::{CoreResourcePort, UnsupportedCoreResourcePort};
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::runtime_http_proxy_endpoint::{
    RuntimeHttpProxyEndpoint, RuntimeHttpProxyEndpointPort,
};
use infiltrator_ports::runtime_stream::{RuntimeStreamPort, UnsupportedRuntimeStreamPort};
use infiltrator_test_support::log_capture_fixtures::LogCaptureProcess;
use infiltrator_test_support::rule_trace_fixtures::RuleTraceStore;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreVersion, EngineId};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::runtime::Builder;

struct LogCaptureLifecycleCapabilities;

impl CoreCapabilityProvider for LogCaptureLifecycleCapabilities {
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
        let entries = [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
            CoreOperation::Profile(ProfileOperation::Apply),
        ]
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
}

#[derive(Clone, Default)]
struct TestCommandHandler {
    intents: Arc<Mutex<Vec<CommandIntent>>>,
}

impl CommandHandler for TestCommandHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        self.record(intent)
    }

    fn handle_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandFuture {
        self.record(intent)
    }

    fn handle_output_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        let future = self.record(intent);
        Box::pin(async move { future.await.map(|()| CommandOutput::Unit) })
    }
}

impl TestCommandHandler {
    fn record(&self, intent: CommandIntent) -> CommandFuture {
        let intents = Arc::clone(&self.intents);
        Box::pin(async move {
            intents.lock().expect("record command intent").push(intent);
            Ok(())
        })
    }
}

struct TestProductBackendFactory;

#[async_trait]
impl DesktopProductBackendFactory for TestProductBackendFactory {
    async fn switch_owner(
        &self,
        _previous_application: Arc<CoreApplication>,
        _target: ActiveProfileReferenceTarget,
        _surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        Err(DesktopProductSwitchFailure::BeforeReservation(failure(
            "test backend factory does not switch owners",
        )))
    }

    async fn retry_owner_switch(
        &self,
        _target: ActiveProfileReferenceTarget,
        _old_owner_terminal: &CoreLifecycleSnapshot,
        _surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        Err(DesktopProductSwitchFailure::OldOwnerClosed(failure(
            "test backend factory does not retry owners",
        )))
    }
}

struct RetryAfterCloseFactory {
    retry_backend: Mutex<Option<DesktopProductBackend>>,
    switch_calls: AtomicUsize,
    retry_calls: AtomicUsize,
}

#[async_trait]
impl DesktopProductBackendFactory for RetryAfterCloseFactory {
    async fn switch_owner(
        &self,
        _previous_application: Arc<CoreApplication>,
        _target: ActiveProfileReferenceTarget,
        _surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        self.switch_calls.fetch_add(1, Ordering::SeqCst);
        Err(DesktopProductSwitchFailure::OldOwnerClosed(failure(
            "injected after the previous owner closed",
        )))
    }

    async fn retry_owner_switch(
        &self,
        _target: ActiveProfileReferenceTarget,
        old_owner_terminal: &CoreLifecycleSnapshot,
        _surface: SurfaceKind,
    ) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
        self.retry_calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(old_owner_terminal.lifecycle, CoreLifecycle::Stopped);
        self.retry_backend
            .lock()
            .expect("take retry backend")
            .take()
            .ok_or_else(|| {
                DesktopProductSwitchFailure::OldOwnerClosed(failure(
                    "fake retry backend was already consumed",
                ))
            })
    }
}

struct TestHostRuntime {
    application: Arc<CoreApplication>,
}

#[async_trait]
impl ActiveProfileReferenceApplier for TestHostRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        CoreLifecyclePort::core_identity(self.application.as_ref()).map(|identity| identity.engine)
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        CoreLifecyclePort::lifecycle(self.application.as_ref())
    }

    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.application
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)
    }
}

#[async_trait]
impl RuntimeHttpProxyEndpointPort for TestHostRuntime {
    async fn observe_http_proxy_endpoint(&self) -> Result<RuntimeHttpProxyEndpoint, PortError> {
        Err(PortError::unsupported(
            Capability::SystemProxy,
            "test host does not compose a proxy endpoint",
        ))
    }
}

impl HostRuntime for TestHostRuntime {
    fn controller_url(&self) -> String {
        "http://127.0.0.1:1".to_owned()
    }

    fn core_binary_path(&self) -> PathBuf {
        PathBuf::from("test-core")
    }

    fn tun_service_status(&self) -> TunServiceStatus {
        TunServiceStatus::Unsupported
    }

    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort> {
        Arc::new(self.application.as_ref().clone())
    }

    fn runtime_configuration_ports(&self) -> RuntimeConfigurationPorts {
        RuntimeConfigurationPorts::unsupported()
    }

    fn connection_port(&self) -> Arc<dyn ConnectionPort> {
        Arc::new(UnsupportedConnectionPort)
    }

    fn runtime_stream_port(&self) -> Arc<dyn RuntimeStreamPort> {
        Arc::new(UnsupportedRuntimeStreamPort)
    }

    fn resource_port(&self) -> Arc<dyn CoreResourcePort> {
        Arc::new(UnsupportedCoreResourcePort)
    }

    fn core_scope_source(&self) -> Arc<dyn CoreScopeSource> {
        Arc::new(self.application.as_ref().clone())
    }
}

fn application() -> Arc<CoreApplication> {
    application_with_handler().0
}

fn application_with_handler() -> (Arc<CoreApplication>, TestCommandHandler) {
    let process = Arc::new(LogCaptureProcess::default());
    let application = Arc::new(CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().expect("create application runtime"),
    ));
    application
        .install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
            LogCaptureLifecycleCapabilities,
        )))
        .expect("install Desktop lifecycle capabilities");
    let handler = TestCommandHandler::default();
    application.install_command_handler(Arc::new(handler.clone()));
    (application, handler)
}

fn surface_pump() -> SurfacePump {
    let initial = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial test snapshot", true),
    );
    SurfacePump::spawn(
        Arc::new(UnavailableSurfaceReader::new("test surface is unavailable")),
        Duration::from_millis(1),
        tokio_application_runtime().expect("create surface runtime"),
        initial,
    )
}

fn backend() -> DesktopProductBackend {
    let application = application();
    backend_for(application)
}

fn backend_for(application: Arc<CoreApplication>) -> DesktopProductBackend {
    let profiles = ProfileApplication::new(Arc::new(RuleTraceStore::default()));
    backend_with_profiles(application, profiles)
}

fn backend_with_profiles(
    application: Arc<CoreApplication>,
    profiles: ProfileApplication,
) -> DesktopProductBackend {
    let host_runtime: Arc<dyn HostRuntime> = Arc::new(TestHostRuntime {
        application: Arc::clone(&application),
    });
    DesktopProductBackend {
        application,
        profiles,
        host_runtime,
        surface_pump: surface_pump(),
    }
}

fn session() -> DesktopProductSession {
    session_for(application())
}

fn session_for(application: Arc<CoreApplication>) -> DesktopProductSession {
    session_for_with_factory(application, Arc::new(TestProductBackendFactory))
}

fn session_for_with_factory(
    application: Arc<CoreApplication>,
    backend_factory: Arc<dyn DesktopProductBackendFactory>,
) -> DesktopProductSession {
    let executor = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create session executor");
    let backend = backend_for(application);
    DesktopProductSession {
        executor,
        application: backend.application,
        profiles: backend.profiles,
        surface: SurfaceKind::IcedDesktop,
        backend_factory,
        state: DesktopProductSessionState::Open {
            host_runtime: backend.host_runtime,
            pump: backend.surface_pump,
        },
    }
}

fn profile_target(session: &DesktopProductSession, profile: &str) -> ActiveProfileReferenceTarget {
    let engine_id = session
        .application()
        .snapshot()
        .scope()
        .expect("Core owner scope")
        .identity
        .engine
        .clone();
    ActiveProfileReferenceTarget::selected(
        engine_id,
        ProfileId::new(profile).expect("valid profile ID"),
    )
}

fn resume_policy(session: &DesktopProductSession) -> CoreRecompositionResume {
    core_recomposition_resume(session.application.snapshot().lifecycle)
        .expect("stable Core lifecycle has a switch resume policy")
}

fn failure(message: &str) -> Failure {
    Failure::new(ErrorCode::NotReady, message, true)
}

#[test]
fn before_reservation_failure_keeps_the_previous_session_and_handles() {
    let mut session = session();
    let application = session.application();
    let host_runtime = session.host_runtime();
    let pump_id = session.surface_pump().bridge().identity();
    let target = profile_target(&session, "next");
    let resume = resume_policy(&session);
    let (host_runtime_before, pump_before) = session
        .begin_engine_switch()
        .expect("open session begins switch");

    let result = session.complete_engine_switch(
        target,
        resume,
        host_runtime_before,
        pump_before,
        Err(DesktopProductSwitchFailure::BeforeReservation(failure(
            "target preflight rejected",
        ))),
    );

    assert!(matches!(
        result,
        Err(DesktopProductSwitchFailure::BeforeReservation(_))
    ));
    assert_eq!(
        session.availability(),
        DesktopProductSessionAvailability::Open
    );
    assert!(Arc::ptr_eq(&application, &session.application()));
    assert!(Arc::ptr_eq(&host_runtime, &session.host_runtime()));
    assert_eq!(session.surface_pump().bridge().identity(), pump_id);
}

#[test]
fn post_reservation_failures_require_recovery_and_refuse_a_second_switch() {
    let failures = [
        DesktopProductSwitchFailure::ReservationPending(failure("reservation remains pending")),
        DesktopProductSwitchFailure::OwnerTerminationUnconfirmed(failure(
            "old owner termination is unconfirmed",
        )),
        DesktopProductSwitchFailure::OldOwnerClosed(failure("old owner closed")),
    ];

    for failure in failures {
        let mut session = session();
        let target = profile_target(&session, "next");
        let resume = resume_policy(&session);
        let (host_runtime, pump) = session
            .begin_engine_switch()
            .expect("open session begins switch");
        let result = session.complete_engine_switch(
            target.clone(),
            resume,
            host_runtime,
            pump,
            Err(failure.clone()),
        );

        assert!(result.is_err());
        assert_eq!(
            session.availability(),
            DesktopProductSessionAvailability::RecoveryRequired
        );
        match &session.state {
            DesktopProductSessionState::RecoveryRequired {
                target: recovery_target,
                failure: recovery_failure,
                ..
            } => {
                assert_eq!(recovery_target, &target);
                assert_eq!(recovery_failure, &failure);
            }
            _ => panic!("expected recovery state"),
        }
        let retry = session.begin_engine_switch();
        assert!(matches!(
            retry,
            Err(DesktopProductSwitchFailure::BeforeReservation(_))
        ));
        assert_eq!(
            session.availability(),
            DesktopProductSessionAvailability::RecoveryRequired
        );
    }
}

#[test]
fn profile_selection_is_rejected_while_the_session_requires_owner_recovery() {
    let mut session = session();
    let engine_id = session
        .application()
        .snapshot()
        .scope()
        .expect("Core owner scope")
        .identity
        .engine
        .clone();
    let resume = resume_policy(&session);
    let (host_runtime, pump) = session
        .begin_engine_switch()
        .expect("open session begins switch");
    let target = ActiveProfileReferenceTarget::selected(
        engine_id.clone(),
        ProfileId::new("main").expect("valid profile ID"),
    );
    session
        .complete_engine_switch(
            target,
            resume,
            host_runtime,
            pump,
            Err(DesktopProductSwitchFailure::OldOwnerClosed(failure(
                "old owner closed",
            ))),
        )
        .expect_err("post-close failure requires owner recovery");

    let rejected = session
        .apply(DesktopProductSessionAction::SelectProfile(
            ActiveProfileReferenceTarget::selected(
                engine_id,
                ProfileId::new("main").expect("valid profile ID"),
            ),
        ))
        .expect_err("recovery-required session rejects profile selection");

    assert!(matches!(
        rejected,
        DesktopProductSessionActionFailure::SessionUnavailable(_)
    ));
}

#[path = "product_tests/switch.rs"]
mod switch;
