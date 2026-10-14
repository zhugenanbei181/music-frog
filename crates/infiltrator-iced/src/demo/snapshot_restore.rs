//! Capture opens the actual restoration review through the production TEA command path.
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::snapshot_restore_fixture::SnapshotRestoreFixture;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
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
    attach_capture_owner(state, core);
    state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target,
    )))
}
