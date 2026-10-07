//! Capture opens the actual restoration review through the production TEA command path.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::snapshot_restore_fixture::SnapshotRestoreFixture;
use infiltrator_composition::tokio_application_runtime;
use std::sync::Arc;
pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let fixture = SnapshotRestoreFixture::new();
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_snapshots(fixture.application),
    ));
    state.commands = Some(core);
    state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target,
    )))
}
