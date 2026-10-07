//! TEA adapter submits the same typed restoration state machine as the Bevy observer adapter.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::Task;
use infiltrator_application::snapshot_restore_workbench::RestorePending;
use infiltrator_contract::error::{ErrorCode, Failure};
impl AppState {
    pub(crate) fn update_snapshot_restore(&mut self, action: RestoreAction) -> Task<Message> {
        let pending = match action {
            RestoreAction::Open(target) => self.editor.snapshot_restore.open(target),
            RestoreAction::Confirm => self.editor.snapshot_restore.confirm(),
            RestoreAction::Cancel => self.editor.snapshot_restore.cancel(),
            RestoreAction::Retry => self.editor.snapshot_restore.retry(),
            RestoreAction::Finished { operation, result } => {
                self.editor.snapshot_restore.finish(operation, *result);
                return Task::none();
            }
        };
        pending
            .map(|pending| self.submit_snapshot_restore(pending))
            .unwrap_or_else(Task::none)
    }
    fn submit_snapshot_restore(&mut self, pending: RestorePending) -> Task<Message> {
        let Some(commands) = self.commands.clone() else {
            self.editor.snapshot_restore.finish(
                pending.operation,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Snapshot command service is unavailable",
                    true,
                )),
            );
            return Task::none();
        };
        Task::perform(
            async move { commands.execute(pending.intent).await.into_output() },
            move |result| {
                Message::SnapshotRestore(RestoreAction::Finished {
                    operation: pending.operation,
                    result: Box::new(result),
                })
            },
        )
    }
}
