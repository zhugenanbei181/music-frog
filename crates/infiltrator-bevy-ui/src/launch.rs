//! Native product composition; fixture mode is an explicit development choice.
use crate::command::{UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use std::sync::Mutex;
use std::{env, mem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Product,
    Demo,
}
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
    let demo = env::var("INFILTRATOR_DEMO").ok();
    match LaunchMode::from_demo_flag(demo.as_deref()) {
        LaunchMode::Demo => crate::run_demo(),
        LaunchMode::Product => run_product(),
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn run_product() {
    use crate::surface::ApplicationSurfaceSource;
    use infiltrator_desktop::product::DesktopProductSession;

    match DesktopProductSession::open(SurfaceKind::BevyDesktop) {
        Ok(session) => {
            let application = session.application();
            let source = ApplicationSurfaceSource::from_application(
                session.surface_pump(),
                application.clone(),
            );
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

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn run_product() {
    if let Some(app) = crate::attached_application() {
        crate::run_with_application(app);
        return;
    }
    // No native host attached an application. A silently interactive showcase
    // here would present working-looking controls over no VPN/kernel, so the
    // product reports the real terminal state instead (BANDROID-004: do not
    // turn "not composed" into fake success). `fixture-demo` is an explicit
    // build-time opt-in used only by the packaging smoke driver; the shipping
    // Gradle host must never enable it.
    #[cfg(feature = "fixture-demo")]
    {
        crate::run_demo();
        return;
    }
    #[cfg(not(feature = "fixture-demo"))]
    {
        #[cfg(target_os = "android")]
        let (surface, host) = (SurfaceKind::BevyAndroid, HostKind::Android);
        #[cfg(not(target_os = "android"))]
        let (surface, host) = (SurfaceKind::IosCompose, HostKind::Ios);
        crate::run_unavailable(surface, host, missing_host_failure());
    }
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
