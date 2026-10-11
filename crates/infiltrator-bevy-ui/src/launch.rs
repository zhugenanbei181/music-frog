//! Native product composition; fixture mode is an explicit development choice.
use crate::command::{UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
#[cfg(feature = "capture-tools")]
use std::env;
use std::mem;
use std::sync::Mutex;

#[cfg(feature = "capture-tools")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Product,
    Demo,
}
#[cfg(feature = "capture-tools")]
impl LaunchMode {
    pub fn from_demo_flag(flag: Option<&str>) -> Self {
        if flag == Some("1") {
            Self::Demo
        } else {
            Self::Product
        }
    }
}

/// Terminal failure for a product launch with no attached native host.
///
/// A missing host is an explicit, retryable `NotReady`; it is never silently
/// replaced by an interactive demo, which would present working-looking
/// controls over no VPN/kernel (BANDROID-004: do not turn "not composed" into
/// fake success).
pub fn missing_host_failure() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "native host bridge is not attached; the application composition is unavailable",
        true,
    )
}

pub(crate) fn run() {
    #[cfg(feature = "capture-tools")]
    {
        let demo = env::var("INFILTRATOR_DEMO").ok();
        run_with_mode(LaunchMode::from_demo_flag(demo.as_deref()));
    }
    #[cfg(not(feature = "capture-tools"))]
    run_product();
}

/// Launch an explicit composition mode. Screenshot and packaging drivers use
/// `Demo`; applications should select `Product` through [`run`].
#[cfg(feature = "capture-tools")]
pub fn run_with_mode(mode: LaunchMode) {
    match mode {
        LaunchMode::Demo => crate::run_demo(),
        LaunchMode::Product => run_product(),
    }
}

#[cfg(all(
    feature = "desktop-host",
    any(target_os = "linux", target_os = "windows", target_os = "macos")
))]
fn run_product() {
    use crate::surface::ApplicationSurfaceSource;
    use infiltrator_desktop::product::DesktopProductSession;

    match DesktopProductSession::open(SurfaceKind::BevyDesktop) {
        Ok(session) => {
            let application = session.application();
            let source = ApplicationSurfaceSource::from_pump(session.surface_pump());
            crate::run_with_application_surface_pump(application, source);
            drop(session);
        }
        Err(error) => crate::run_unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, error.to_string(), true),
        ),
    }
}

#[cfg(not(all(
    feature = "desktop-host",
    any(target_os = "linux", target_os = "windows", target_os = "macos")
)))]
fn run_product() {
    // Product mode has no scoped host composition. It reports unavailable;
    // screenshot tooling selects the explicit demo entrypoint.
    #[cfg(target_os = "android")]
    let (surface, host) = (SurfaceKind::BevyAndroid, HostKind::Android);
    #[cfg(all(
        not(target_os = "android"),
        any(target_os = "linux", target_os = "windows", target_os = "macos")
    ))]
    let (surface, host) = (SurfaceKind::BevyDesktop, HostKind::Desktop);
    #[cfg(not(any(
        target_os = "android",
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    )))]
    let (surface, host) = (SurfaceKind::IosCompose, HostKind::Ios);
    crate::run_unavailable(surface, host, missing_host_failure());
}

/// A failed composition returns terminal rejection for every attempted operation.
pub struct UnavailableCommandSink {
    failure: Failure,
    pending: Mutex<(u64, Vec<CommandExecutedEvent>)>,
}
impl UnavailableCommandSink {
    pub fn new(failure: Failure) -> Self {
        Self {
            failure,
            pending: Mutex::new((0, Vec::new())),
        }
    }
}
impl UiCommandSink for UnavailableCommandSink {
    fn submit(&self, command: UiCommand) {
        self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let mut pending = self.pending.lock().expect("unavailable command lock");
        pending.0 += 1;
        let request_id = RequestId::new(pending.0);
        pending.1.push(CommandExecutedEvent {
            command,
            request_id,
            result: Err(self.failure.clone()),
        });
        Some(request_id)
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        mem::take(&mut self.pending.lock().expect("unavailable command lock").1)
    }
}
