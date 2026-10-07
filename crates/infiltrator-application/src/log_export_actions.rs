//! One confirmation, failure and retry state machine for both native peers.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{LogExportReceipt, LogExportSummary};

#[derive(Clone, Debug)]
pub struct LogExportPending {
    pub operation: u64,
    pub intent: CommandIntent,
}
#[derive(Clone, Debug, Default)]
pub struct LogExportActions {
    pub open: bool,
    pub summary: Option<LogExportSummary>,
    pub receipt: Option<LogExportReceipt>,
    pub failure: Option<Failure>,
    pub pending: Option<LogExportPending>,
    retry: Option<CommandIntent>,
    sequence: u64,
}
impl LogExportActions {
    pub fn show(&mut self) -> Option<LogExportPending> {
        if self.open {
            return None;
        }
        self.open = true;
        self.summary = None;
        self.receipt = None;
        self.begin(CommandIntent::PrepareLogExport)
    }
    pub fn confirm(&mut self) -> Option<LogExportPending> {
        if !self.open || self.receipt.is_some() || self.failure.is_some() {
            return None;
        }
        let identity = self.summary.as_ref()?.identity.clone();
        self.begin(CommandIntent::SaveLogExport { identity })
    }
    pub fn cancel(&mut self) -> Option<LogExportPending> {
        if !self.open || self.pending.is_some() {
            return None;
        }
        if let Some(summary) = &self.summary {
            self.begin(CommandIntent::CancelLogExport {
                identity: summary.identity.clone(),
            })
        } else {
            self.close();
            None
        }
    }
    pub fn can_retry(&self) -> bool {
        self.open
            && self.pending.is_none()
            && self.retry.is_some()
            && self
                .failure
                .as_ref()
                .is_some_and(|failure| failure.retryable || failure.code == ErrorCode::Permission)
    }
    pub fn retry(&mut self) -> Option<LogExportPending> {
        if !self.can_retry() {
            return None;
        }
        self.begin(self.retry.clone()?)
    }
    fn begin(&mut self, intent: CommandIntent) -> Option<LogExportPending> {
        if self.pending.is_some() {
            return None;
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("log operation identity exhausted");
        let pending = LogExportPending {
            operation: self.sequence,
            intent,
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        self.retry = None;
        Some(pending)
    }
    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) -> bool {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.operation == operation)
            .cloned()
        else {
            return false;
        };
        self.pending = None;
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            if let CommandOutput::LogExportSaved(receipt) = &output {
                let summary = self.summary.as_ref().ok_or_else(|| {
                    Failure::new(
                        ErrorCode::InvalidState,
                        "Prepared log export source is no longer available",
                        false,
                    )
                })?;
                receipt.validate(summary)?;
            }
            Ok(output)
        });
        match result {
            Ok(CommandOutput::LogExportPrepared(summary)) => self.summary = Some(summary),
            Ok(CommandOutput::LogExportSaved(receipt)) => self.receipt = Some(receipt),
            Ok(CommandOutput::Unit) => self.close(),
            Ok(_) => unreachable!("typed output validation rejected unrelated output"),
            Err(failure) => {
                self.failure = Some(failure);
                self.retry = Some(pending.intent);
            }
        }
        true
    }
    fn close(&mut self) {
        self.open = false;
        self.summary = None;
        self.receipt = None;
        self.failure = None;
        self.retry = None;
    }
}

#[cfg(test)]
#[path = "log_export_actions_tests.rs"]
mod tests;
