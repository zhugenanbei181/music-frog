use crate::command_sink::DesktopCommandSink;
use crate::surface::DesktopSurfaceSource;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_bevy_ui::surface::host_insets::window_insets_input_channel;
use infiltrator_bevy_ui::tray_host::{TrayHost, initial_tray_spec};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::tray_menu::TraySpec;
use infiltrator_contract::tray_status::{TrayEvent, TrayIconColorMode, TraySupport};
use infiltrator_desktop::product::DesktopProductSession;
use infiltrator_desktop::tray::DesktopTrayHost;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::sync::Arc;
use std::time::Duration;

const SURFACE_REFRESH_INTERVAL: Duration = Duration::from_millis(80);

struct BevyDesktopTrayHost(DesktopTrayHost);

impl TrayHost for BevyDesktopTrayHost {
    fn push(&self, spec: TraySpec) {
        self.0.push(spec);
    }

    fn drain(&self) -> Vec<TrayEvent> {
        self.0.drain()
    }

    fn support(&self) -> TraySupport {
        self.0.support()
    }
}

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
    let initial_spec = initial_tray_spec(TrayIconColorMode::Colored);
    let tray: Option<Box<dyn TrayHost>> =
        DesktopTrayHost::start(initial_spec, TrayIconColorMode::Colored)
            .ok()
            .map(|host| Box::new(BevyDesktopTrayHost(host)) as Box<dyn TrayHost>);

    infiltrator_bevy_ui::run_with_host_surface_and_command_sink_and_tray(
        source,
        sink,
        insets_receiver,
        SURFACE_REFRESH_INTERVAL,
        tray,
    );
    drop(session);
}
