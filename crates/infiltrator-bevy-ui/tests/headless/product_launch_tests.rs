//! test-intent: behavior
//! Product commands and read models share the composed application; failed hosts stay failed.
use crate::command_harness::{HostlessProcess, RecordingHandler};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::{SurfacePump, UnavailableSurfaceReader};
use infiltrator_bevy_ui::command::{UiCommand, UiCommandSink};
use infiltrator_bevy_ui::launch::{LaunchMode, UnavailableCommandSink, missing_host_failure};
use infiltrator_bevy_ui::projection::{OverviewSource, SourceKind};
use infiltrator_bevy_ui::surface::{
    ApplicationSurfaceSource, SurfaceSource, UnavailableSurfaceSource,
};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceOrigin, SurfaceSnapshot};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

#[derive(Default)]
struct ModePort {
    calls: Mutex<Vec<ProxyMode>>,
    failure: Mutex<Option<PortError>>,
}
#[async_trait::async_trait]
impl OverviewReader for ModePort {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        Err(PortError::Failed(
            "sampling is not part of this mode test".into(),
        ))
    }
    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError> {
        self.calls.lock().unwrap().push(mode);
        match self.failure.lock().unwrap().clone() {
            Some(failure) => Err(failure),
            None => Ok(mode),
        }
    }
}

#[test]
fn fixture_mode_requires_explicit_opt_in() {
    for flag in [None, Some(""), Some("0"), Some("true"), Some("invalid")] {
        assert_eq!(LaunchMode::from_demo_flag(flag), LaunchMode::Product);
    }
    assert_eq!(LaunchMode::from_demo_flag(Some("1")), LaunchMode::Demo);
}

#[test]
fn mobile_product_without_a_host_is_not_ready_not_a_silent_demo() {
    // BANDROID-004: an unattached host must surface a typed terminal state,
    // never an implicitly interactive demo over no VPN/kernel.
    let failure = missing_host_failure();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.retryable);
}

#[test]
fn composed_surface_executes_mode_in_the_same_application_and_propagates_rejection() {
    let mode_port = Arc::new(ModePort::default());
    let handler = Arc::new(RecordingHandler::default());
    let application = CoreApplication::new_with_overview(
        Arc::new(HostlessProcess),
        Arc::new(HostlessProcess),
        mode_port.clone(),
        tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(handler.clone());
    let initial = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "stopped fixture host", true),
    );
    let pump = SurfacePump::spawn(
        Arc::new(UnavailableSurfaceReader::new("stopped fixture host")),
        Duration::ZERO,
        tokio_application_runtime().unwrap(),
        initial,
    );
    let source = ApplicationSurfaceSource::from_application(pump, Arc::new(application));
    assert_eq!(source.kind(), SourceKind::LiveCore);
    assert_eq!(source.surface_snapshot().origin, SurfaceOrigin::Live);

    let (ack, result) = mpsc::channel();
    source.set_mode(ProxyMode::Global, ack);
    assert_eq!(
        result.recv_timeout(Duration::from_secs(2)).unwrap(),
        Ok(ProxyMode::Global)
    );
    assert_eq!(*mode_port.calls.lock().unwrap(), vec![ProxyMode::Global]);
    let failure = PortError::PermissionDenied("controller permission denied".into());
    *mode_port.failure.lock().unwrap() = Some(failure.clone());
    let (ack, result) = mpsc::channel();
    source.set_mode(ProxyMode::Direct, ack);
    assert_eq!(
        result.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(Failure::from(failure.clone()))
    );
    assert_eq!(
        *mode_port.calls.lock().unwrap(),
        vec![ProxyMode::Global, ProxyMode::Direct]
    );
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn failed_host_projects_failure_on_every_page_and_rejects_each_operation_once() {
    let failure = Failure::new(ErrorCode::Storage, "profile directory is read only", true);
    let source =
        UnavailableSurfaceSource::new(SurfaceKind::BevyDesktop, HostKind::Desktop, failure.clone());
    let snapshot = source.surface_snapshot();
    assert_eq!(snapshot.origin, SurfaceOrigin::Live);
    for page in PageId::ALL {
        assert_eq!(
            snapshot.pages.status(page),
            &PageStatus::Unavailable {
                failure: failure.clone()
            }
        );
    }
    let (ack, result) = mpsc::channel();
    source.set_mode(ProxyMode::Rule, ack);
    assert_eq!(result.recv().unwrap(), Err(failure.clone()));

    let sink = UnavailableCommandSink::new(failure.clone());
    let first = sink.submit_tracked(UiCommand::StartCore).unwrap();
    let second = sink.submit_tracked(UiCommand::CloseAllConnections).unwrap();
    assert_ne!(first, second);
    let results = sink.drain_results();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].request_id, first);
    assert_eq!(results[1].request_id, second);
    for result in results {
        assert!(result.result.is_err());
        assert_eq!(result.result.as_ref().err(), Some(&failure));
    }
    assert!(sink.drain_results().is_empty());
}
