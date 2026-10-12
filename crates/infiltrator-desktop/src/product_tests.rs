//! Desktop product-session owner handoff and recovery-state tests.

use super::{
    DesktopProductSession, DesktopProductSessionAvailability, DesktopProductSessionState,
    DesktopProductSwitchFailure,
};
use crate::composition::DesktopProductBackend;
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::LogCaptureProcess;
use infiltrator_application::surface_application::{SurfacePump, UnavailableSurfaceReader};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway, RuntimeStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;

struct TestHostRuntime {
    application: Arc<CoreApplication>,
}

fn unsupported<T>(capability: Capability) -> Result<T, PortError> {
    Err(PortError::unsupported(
        capability,
        "test host runtime does not implement controller operations",
    ))
}

#[async_trait]
impl RuntimeGateway for TestHostRuntime {
    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn test_delay(
        &self,
        _proxy: &str,
        _url: &str,
        _timeout_ms: u32,
    ) -> Result<u32, PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(Capability::RoutingIntent)
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        unsupported(Capability::Connections)
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        unsupported(Capability::Connections)
    }

    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        unsupported(Capability::Connections)
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        unsupported(Capability::Connections)
    }

    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<RuntimeStream<String>, PortError> {
        unsupported(Capability::Logs)
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        unsupported(Capability::Connections)
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        unsupported(Capability::Connections)
    }
}

#[async_trait]
impl ManagedRuntime for TestHostRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        CoreLifecyclePort::core_identity(self.application.as_ref()).map(|identity| identity.engine)
    }

    fn generation(&self) -> u64 {
        self.application.generation()
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        CoreLifecyclePort::lifecycle(self.application.as_ref())
    }

    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.application
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)
    }

    async fn is_running(&self) -> bool {
        matches!(
            self.core_lifecycle(),
            CoreLifecycle::Ready | CoreLifecycle::Running
        )
    }

    async fn restart(&self) -> Result<u64, PortError> {
        unsupported(Capability::CoreLifecycle)
    }

    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn shutdown(&self) -> Result<(), PortError> {
        CoreLifecyclePort::stop(self.application.as_ref())
            .await
            .map(|_| ())
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
}

fn application() -> Arc<CoreApplication> {
    let process = Arc::new(LogCaptureProcess::default());
    Arc::new(CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().expect("create application runtime"),
    ))
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
    let host_runtime: Arc<dyn HostRuntime> = Arc::new(TestHostRuntime {
        application: Arc::clone(&application),
    });
    DesktopProductBackend {
        application,
        host_runtime,
        surface_pump: surface_pump(),
    }
}

fn session() -> DesktopProductSession {
    let executor = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create session executor");
    let backend = backend();
    DesktopProductSession {
        executor,
        application: backend.application,
        surface: SurfaceKind::IcedDesktop,
        state: DesktopProductSessionState::Open {
            host_runtime: backend.host_runtime,
            pump: backend.surface_pump,
        },
    }
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
    let (host_runtime_before, pump_before) = session
        .begin_engine_switch()
        .expect("open session begins switch");

    let result = session.complete_engine_switch(
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
        let (host_runtime, pump) = session
            .begin_engine_switch()
            .expect("open session begins switch");
        let result = session.complete_engine_switch(host_runtime, pump, Err(failure));

        assert!(result.is_err());
        assert_eq!(
            session.availability(),
            DesktopProductSessionAvailability::RecoveryRequired
        );
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
fn successful_switch_replaces_application_and_surface_handles() {
    let mut session = session();
    let previous_application = session.application();
    let (host_runtime, pump) = session
        .begin_engine_switch()
        .expect("open session begins switch");
    let replacement = backend();
    let replacement_application = Arc::clone(&replacement.application);

    session
        .complete_engine_switch(host_runtime, pump, Ok(replacement))
        .expect("new owner becomes the open session");

    assert_eq!(
        session.availability(),
        DesktopProductSessionAvailability::Open
    );
    assert!(Arc::ptr_eq(
        &replacement_application,
        &session.application()
    ));
    assert!(!Arc::ptr_eq(&previous_application, &session.application()));
}
