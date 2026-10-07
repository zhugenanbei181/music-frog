//! Capture executes the actual diagnostic TEA task and observes its failed reader state.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::doctor_capture_fixtures::DoctorCapturePort;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
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
        CommandApplication::new().with_doctor(doctor.clone()),
    ));
    state.commands = Some(application);
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated capture host services"),
        )
    });
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.revision += 1;
    snapshot.pages.doctor = doctor.page();
    state.apply_shared_surface_snapshot(snapshot);
    state.diag.doctor.capture_observation = Some(doctor);
    state.update(Message::RunDoctor)
}
