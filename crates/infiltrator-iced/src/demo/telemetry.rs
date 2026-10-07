//! Capture actual stale-zero telemetry through production reader and update paths.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::LogCaptureProcess;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::telemetry_observation_fixtures::{
    TelemetryObservationReader, stale_zero_snapshot,
};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let process = Arc::new(LogCaptureProcess::default());
    let telemetry = Arc::new(TelemetryObservationReader::default());
    let core = CoreApplication::new_with_overview(
        process.clone(),
        process,
        telemetry.clone(),
        tokio_application_runtime().expect("isolated telemetry executor"),
    );
    state.commands = Some(core.clone());
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let revision = state.surface.revision() + 1;
    Task::perform(
        async move {
            let mut snapshot = stale_zero_snapshot(&core, &reader, &telemetry)
                .await
                .expect("actual telemetry transitions");
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
}
