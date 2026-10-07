//! Config snapshot diff & rollback actions (DUAL-09-08/09/12).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` computed by the snapshot
//! application; this module owns only the surface state machine around it
//! (loading, layout, two-step rollback confirmation, protection unlock).

use crate::snapshot_commands::execute;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;

impl AppState {
    pub(super) fn update_snapshot_diff(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSnapshotDiff(id) => {
                self.editor.snapshot_diff_modal_open = true;
                self.editor.snapshot_diff_selected_id = Some(id.clone());
                self.editor.snapshot_diff_loading = true;
                self.editor.snapshot_diff_error = None;
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
                let commands = self.commands.clone();
                Task::perform(
                    async move {
                        execute(
                            commands,
                            CommandIntent::LoadSnapshotDiff {
                                profile: Some(profile),
                                snapshot_id: Some(id),
                            },
                        )
                        .await?
                        .into_snapshot_diff()
                        .map_err(|failure| InfiltratorError::Config(failure.message))?
                        .ok_or_else(|| {
                            InfiltratorError::Config("No stored snapshot is available".into())
                        })
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
            // DUAL-09-14: the same "recompute from the shared snapshot
            // application" action the Bevy card offers. Re-dispatching through
            // the open path keeps one loading/error state machine.
            Message::RefreshSnapshotDiff => match self.editor.snapshot_diff_selected_id.clone() {
                Some(id) => self.update_snapshot_diff(Message::OpenSnapshotDiff(id)),
                None => Task::none(),
            },
            Message::SetProfileProtectionOverride(allow) => {
                self.editor.profile_protection_override = allow;
                Task::none()
            }
            Message::CloseSnapshotDiff => {
                self.editor.snapshot_diff_modal_open = false;
                self.editor.snapshot_diff_selected_id = None;
                self.editor.snapshot_diff = None;
                self.editor.snapshot_diff_error = None;
                Task::none()
            }
            Message::RollbackToSnapshot(id) => {
                let Some(profile) = self.editor_profile_name() else {
                    return Task::none();
                };
                self.update_snapshot_restore(RestoreAction::Open(SnapshotRestoreTarget {
                    profile,
                    snapshot_id: id,
                }))
            }
            _ => Task::none(),
        }
    }
}
