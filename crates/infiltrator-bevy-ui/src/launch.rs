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
    // When standalone mobile host bridge is unattached, run interactive self-contained
    // showcase rather than displaying an unresponsive unavailable failure screen.
    crate::run_demo();
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
