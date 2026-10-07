//! Dedicated logs activation composes the shared buffer and real typed reader.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{
    LOG_QUERY, LogCaptureProcess, append_follow_records, populate_logs,
};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_application::log_export_capture::ReadOnlyLogExportCapture;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().expect("isolated log executor"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_logs(core.log_application())
            .with_log_export(LogExportApplication::new(
                core.log_application(),
                Some(Arc::new(ReadOnlyLogExportCapture)),
                vec![],
            )),
    ));
    state.commands = Some(core.clone());
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let revision = state.surface.revision() + 1;
    let continuing = core.clone();
    let later_reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let task = Task::perform(
        async move {
            populate_logs(&core)
                .await
                .expect("actual isolated log lifecycle");
            let mut snapshot = reader.read().await.expect("actual isolated log reader");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    );
    if feature == FeatureId::LogsRedactedExport {
        task.chain(Task::done(Message::ExportRedactedLogs))
    } else if feature == FeatureId::LogsScrollLock {
        task.chain(Task::done(Message::ToggleLogFollow))
            .chain(Task::perform(
                async move {
                    append_follow_records(&continuing).expect("continued actual log facts");
                    let mut snapshot = later_reader.read().await.expect("actual continued logs");
                    snapshot.origin = SurfaceOrigin::Demo;
                    snapshot.revision = revision + 1;
                    snapshot
                },
                |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
            ))
    } else {
        task.chain(Task::done(Message::UpdateLogRegexFilter(LOG_QUERY.into())))
    }
}
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let prepared = matches!(
        &message,
        Message::LogExportFinished {
            result: Ok(CommandOutput::LogExportPrepared(_)),
            ..
        }
    );
    let task = state.update(message);
    if prepared && state.diag.log_export.summary.is_some() {
        task.chain(Task::done(Message::ConfirmLogExport))
    } else {
        task
    }
}

#[cfg(test)]
#[path = "../../tests/gui/log_capture_tests.rs"]
mod tests;
