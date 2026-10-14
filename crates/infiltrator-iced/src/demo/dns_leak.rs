//! Actual composed command activation with an isolated echo host and shared report readback.
use super::capabilities::DemoHostCapabilityProvider;
use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_test_support::dns_leak_fixtures::IsolatedEcho;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
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
        CommandApplication::new()
            .with_host_capabilities(Arc::new(DemoHostCapabilityProvider::new(Capability::Dns)))
            .with_dns_leak(application.clone()),
    ));
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "DNS leak capture is preparing", true),
    );
    snapshot.dns_leak = application.last_report();
    attach_capture_owner_with_snapshot(state, core, snapshot);
    state.diag.dns_leak_capture = Some(application);
    state.update(Message::RunDnsLeakProbe)
}
