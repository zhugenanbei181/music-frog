//! Capture actual stale-zero telemetry through production reader and update paths.
use crate::demo::capabilities::install_demo_log_lifecycle_capability;
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::telemetry_observation_fixtures::{
    TelemetryObservationReader, stale_zero_snapshot,
};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_test_support::log_capture_fixtures::LogCaptureProcess;
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
    // The capture starts a real core, so the owner has to publish the
    // capability matrix that admits the shared start command. Without it the
    // capability gate refuses the start and the capture never observes the
    // stale-zero readback it is meant to prove.
    install_demo_log_lifecycle_capability(&core);
    attach_capture_owner(state, core.clone());
    let reader = ProductSurfaceReader::new(
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
