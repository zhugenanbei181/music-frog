//! The crash-watchdog capture owner: a managed core that really exits.
//!
//! The reconnect mask and the lifecycle recovery panel are projections of the
//! shared watchdog policy, so the capture drives a *running* core into the
//! exit the policy observes. The recovery state the surface renders is then
//! the watchdog policy's own answer, never a hand-written flag.

use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::reconnect_mask_application::ReconnectMaskApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::core_watchdog::CoreWatchdogPort;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The engine identity of the isolated watchdog capture owner.
pub(super) const WATCHDOG_CAPTURE_ENGINE: &str = "watchdog-capture";

/// A capture core whose managed process exits on demand.
///
/// The watchdog only observes the exit of a *running* session, so the fixture
/// starts as a running core and reports `Stopped` once the capture asks for
/// the exit.
pub(super) struct DemoWatchdogCaptureCore {
    running: Arc<AtomicBool>,
}

impl Default for DemoWatchdogCaptureCore {
    /// The managed process starts as a running core: the watchdog only
    /// observes the exit of a session that was really running.
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl DemoWatchdogCaptureCore {
    /// The managed process disappears: the watchdog observes the exit.
    pub(super) fn exit(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl CoreProcess for DemoWatchdogCaptureCore {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new(WATCHDOG_CAPTURE_ENGINE).expect("valid capture engine"),
            CoreInstanceId::new("watchdog-capture").expect("valid capture instance"),
            Some(CoreVersion::new("1.19.12").expect("valid capture version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(if self.running.load(Ordering::SeqCst) {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

#[async_trait::async_trait]
impl CoreReadiness for DemoWatchdogCaptureCore {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("watchdog-capture".into())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid capture version"),
        ))
    }
}

/// The isolated capture owner whose watchdog really observed a core exit.
///
/// The published snapshot carries the owner's own core projection (including the
/// watchdog state the policy published) and the reconnect mask the shared
/// application derived from it.
pub(super) fn attach_watchdog_capture_owner(state: &mut AppState) -> Task<Message> {
    let process = Arc::new(DemoWatchdogCaptureCore::default());
    let runtime = tokio_application_runtime().expect("isolated watchdog capture runtime");
    let application = CoreApplication::new(process.clone(), process.clone(), runtime.clone());
    let ready = application.clone();
    runtime.block_on(Box::pin(async move {
        assert!(
            ready
                .adopt_if_running()
                .await
                .expect("adopted running fixture core"),
            "the watchdog fixture core must be running before it can exit"
        );
    }));
    // The managed process exits: the shared watchdog policy observes the exit
    // and schedules its first recovery attempt.
    process.exit();
    let ticking = application.clone();
    runtime.block_on(Box::pin(async move {
        ticking
            .watchdog_tick()
            .await
            .expect("the watchdog observed the fixture core exit");
    }));
    let snapshot = application.snapshot();
    let mask = ReconnectMaskApplication.project(&snapshot);
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(
            ErrorCode::NotReady,
            "the watchdog capture is preparing its recovery observation",
            true,
        ),
    );
    surface.core = snapshot;
    surface.reconnect_mask = mask;
    attach_capture_owner_with_snapshot(state, application, surface);
    Task::none()
}
