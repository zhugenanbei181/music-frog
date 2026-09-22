//! Config snapshot diff & rollback actions (DUAL-09-08/09/12).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` computed by the snapshot
//! application; this module owns only the surface state machine around it
//! (loading, layout, two-step rollback confirmation, protection unlock).

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::error::InfiltratorError;

impl AppState {
    pub(super) fn update_snapshot_diff(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSnapshotDiff(id) => {
                self.editor.snapshot_diff_modal_open = true;
                self.editor.snapshot_diff_selected_id = Some(id.clone());
                self.editor.snapshot_diff_loading = true;
                self.editor.snapshot_diff_error = None;
                self.editor.snapshot_diff_rollback_armed = false;
                // DUAL-09-08: the modal renders the shared application's real
                // Myers diff; it never fabricates rows.
                let Some(profile) = self
                    .editor
                    .editor_path
                    .as_ref()
                    .and_then(|path| path.file_stem())
                    .and_then(|stem| stem.to_str())
                    .map(str::to_string)
                else {
                    self.editor.snapshot_diff_loading = false;
                    self.editor.snapshot_diff_error =
                        Some("no profile is open in the editor".to_string());
                    return Task::none();
                };
                Task::perform(
                    async move {
                        crate::snapshot_application::application()
                            .await?
                            .diff_snapshot(&profile, std::path::Path::new(&id))
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::SnapshotDiffLoaded,
                )
            }
            Message::SnapshotDiffLoaded(result) => {
                self.editor.snapshot_diff_loading = false;
                match result {
                    Ok(diff) => {
                        self.editor.snapshot_diff = Some(diff);
                        self.editor.snapshot_diff_error = None;
                    }
                    Err(error) => {
                        self.editor.snapshot_diff = None;
                        self.editor.snapshot_diff_error = Some(error.to_string());
                    }
                }
                Task::none()
            }
            Message::SetSnapshotDiffMode(mode) => {
                self.editor.snapshot_diff_mode = mode;
                Task::none()
            }
            Message::ArmSnapshotRollback => {
                self.editor.snapshot_diff_rollback_armed = true;
                Task::none()
            }
            Message::CancelSnapshotRollback => {
                self.editor.snapshot_diff_rollback_armed = false;
                Task::none()
            }
            Message::SetProfileProtectionOverride(allow) => {
                self.editor.profile_protection_override = allow;
                Task::none()
            }
            Message::CloseSnapshotDiff => {
                self.editor.snapshot_diff_modal_open = false;
                self.editor.snapshot_diff_selected_id = None;
                self.editor.snapshot_diff = None;
                self.editor.snapshot_diff_error = None;
                self.editor.snapshot_diff_rollback_armed = false;
                Task::none()
            }
            Message::RollbackToSnapshot(id) => {
                // DUAL-09-09: the rollback only executes once the two-step
                // confirmation has been armed; the first click never applies.
                if !self.editor.snapshot_diff_rollback_armed {
                    self.editor.snapshot_diff_rollback_armed = true;
                    return Task::none();
                }
                self.editor.snapshot_diff_modal_open = false;
                self.editor.snapshot_diff_rollback_armed = false;
                // The shared restore executor requires the same confirmation
                // the modal just collected.
                self.editor.pending_restore_snapshot = Some(id.clone().into());
                Task::done(Message::RestoreProfileSnapshot(id.into()))
            }
            _ => Task::none(),
        }
    }
}
