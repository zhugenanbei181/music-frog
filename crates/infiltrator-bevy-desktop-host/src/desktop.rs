use crate::command_sink::DesktopCommandSink;
use crate::surface::DesktopSurfaceSource;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_bevy_ui::surface::host_insets::window_insets_input_channel;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_desktop::product::DesktopProductSession;
use std::sync::Arc;
use std::time::Duration;

const SURFACE_REFRESH_INTERVAL: Duration = Duration::from_millis(80);

pub fn run_desktop() {
    match DesktopProductSession::open(SurfaceKind::BevyDesktop) {
        Ok(session) => run_session(session),
        Err(error) => infiltrator_bevy_ui::run_unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, error.to_string(), true),
        ),
    }
}

fn run_session(session: DesktopProductSession) {
    let application: Arc<CoreApplication> = session.application();
    let source = Arc::new(DesktopSurfaceSource::new(session.surface_pump()));
    let sink = Arc::new(DesktopCommandSink::new_for_surface(
        application,
        source.scope_observation(),
    ));
    let (_insets_sender, insets_receiver) = window_insets_input_channel();
    infiltrator_bevy_ui::run_with_host_surface_and_command_sink(
        source,
        sink,
        insets_receiver,
        SURFACE_REFRESH_INTERVAL,
    );
    drop(session);
}
