//! Actual composed command activation with an isolated echo host and shared report readback.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_leak_fixtures::IsolatedEcho;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use std::sync::Arc;
pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let application = DnsLeakApplication::new(
        Some(Arc::new(IsolatedEcho::default())),
        default_echo_sources(),
    );
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_leak(application.clone()),
    ));
    state.commands = Some(core);
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated capture services"),
        )
    });
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.revision += 1;
    snapshot.dns_leak = application.last_report();
    state.apply_shared_surface_snapshot(snapshot);
    state.diag.dns_leak_capture = Some(application);
    state.update(Message::RunDnsLeakProbe)
}
