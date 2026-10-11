//! Capture executes the actual diagnostic TEA task and observes its failed reader state.
use super::capabilities::DemoHostCapabilityProvider;
use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::doctor_capture_fixtures::DoctorCapturePort;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let runtime = tokio_application_runtime().expect("isolated capture runtime");
    let doctor = DoctorApplication::new(Arc::new(DoctorCapturePort::default()));
    let initial = doctor.clone();
    runtime.block_on(Box::pin(async move {
        initial.run(None).await.expect("observed initial diagnosis");
    }));
    let application = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    );
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_host_capabilities(Arc::new(DemoHostCapabilityProvider::new(
                Capability::CoreLifecycle,
            )))
            .with_doctor(doctor.clone()),
    ));
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "doctor capture is preparing", true),
    );
    snapshot.pages.doctor = doctor.page();
    attach_capture_owner_with_snapshot(state, application, snapshot);
    state.diag.doctor.capture_observation = Some(doctor);
    state.update(Message::RunDoctor)
}
