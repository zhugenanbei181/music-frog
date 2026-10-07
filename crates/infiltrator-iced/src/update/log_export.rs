//! Native confirmation submits exact typed commands; only an actual receipt shows a saved path.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::log_export_actions::LogExportPending;
use infiltrator_contract::error::{ErrorCode, Failure};
impl AppState {
    pub(super) fn update_log_export(&mut self, message: Message) -> Task<Message> {
        let request = match message {
            Message::ExportRedactedLogs => self.diag.log_export.show(),
            Message::ConfirmLogExport => self.diag.log_export.confirm(),
            Message::RetryLogExport => self.diag.log_export.retry(),
            Message::CancelLogExport => self.diag.log_export.cancel(),
            Message::LogExportFinished { operation, result } => {
                self.diag.log_export.finish(operation, result);
                return Task::none();
            }
            _ => unreachable!("log export message family"),
        };
        request
            .map(|request| self.submit_log_export(request))
            .unwrap_or_else(Task::none)
    }
    fn submit_log_export(&mut self, request: LogExportPending) -> Task<Message> {
        let operation = request.operation;
        let Some(core) = self.commands.clone() else {
            self.diag.log_export.finish(
                operation,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Log export command service is unavailable",
                    true,
                )),
            );
            return Task::none();
        };
        Task::perform(
            async move { core.execute(request.intent).await.into_output() },
            move |result| Message::LogExportFinished { operation, result },
        )
    }
}
