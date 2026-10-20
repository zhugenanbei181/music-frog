//! Config snapshot diff & rollback actions (DUAL-09-08/09/12).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` computed by the snapshot
//! application; this module owns only the surface state machine around it
//! (loading, layout, two-step rollback confirmation, protection unlock).

use crate::snapshot_commands::execute;
use crate::state::AppState;
use crate::state::editor::SnapshotDiffModalState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use lurker_core_contract::error::InfiltratorError;

impl AppState {
    pub(super) fn update_snapshot_diff(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSnapshotDiff(id) => {
                self.editor.snapshot_diff_modal = SnapshotDiffModalState::Loading {
                    snapshot_id: id.clone(),
                };
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
                    self.editor.snapshot_diff_modal = SnapshotDiffModalState::Failed {
                        snapshot_id: id.clone(),
                        reason: "no profile is open in the editor".to_string(),
                    };
                    return Task::none();
                };
                let commands = self.command_ingress();
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
                let snapshot_id = self
                    .editor
                    .snapshot_diff_modal
                    .snapshot_id()
                    .unwrap_or_default()
                    .to_owned();
                self.editor.snapshot_diff_modal = match result {
                    Ok(diff) => SnapshotDiffModalState::Loaded { snapshot_id, diff },
                    Err(error) => SnapshotDiffModalState::Failed {
                        snapshot_id,
                        reason: error.to_string(),
                    },
                };
                Task::none()
            }
            Message::SetSnapshotDiffMode(mode) => {
                self.editor.snapshot_diff_mode = mode;
                Task::none()
            }
            // DUAL-09-14: the same "recompute from the shared snapshot
            // application" action the Bevy card offers. Re-dispatching through
            // the open path keeps one loading/error state machine.
            Message::RefreshSnapshotDiff => {
                match self.editor.snapshot_diff_modal.snapshot_id().map(str::to_owned) {
                    Some(id) => self.update_snapshot_diff(Message::OpenSnapshotDiff(id)),
                    None => Task::none(),
                }
            }
            Message::SetProfileWriteAuthorization(authorization) => {
                self.editor.profile_write_authorization = authorization;
                Task::none()
            }
            Message::CloseSnapshotDiff => {
                self.editor.snapshot_diff_modal = SnapshotDiffModalState::Closed;
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
