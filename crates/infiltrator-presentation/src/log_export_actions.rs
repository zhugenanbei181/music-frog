//! Typed log-export workbench transitions shared by native surfaces.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{LogExportReceipt, LogExportSummary};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogExportPending {
    pub operation: u64,
    pub intent: CommandIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogExportContext {
    Review(LogExportSummary),
    Saved(LogExportReceipt),
}

impl LogExportContext {
    fn summary(&self) -> &LogExportSummary {
        match self {
            Self::Review(summary) => summary,
            Self::Saved(receipt) => &receipt.summary,
        }
    }

    fn identity(&self) -> infiltrator_contract::log_export::LogExportIdentity {
        self.summary().identity.clone()
    }

    fn receipt(&self) -> Option<&LogExportReceipt> {
        match self {
            Self::Review(_) => None,
            Self::Saved(receipt) => Some(receipt),
        }
    }
}

/// Every reachable modal phase carries exactly the data valid in that phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogExportPhase {
    Closed,
    Preparing {
        pending: LogExportPending,
    },
    Review {
        summary: LogExportSummary,
    },
    Saving {
        summary: LogExportSummary,
        pending: LogExportPending,
    },
    Saved {
        receipt: LogExportReceipt,
    },
    Cancelling {
        context: LogExportContext,
        pending: LogExportPending,
    },
    PrepareFailed {
        failure: Failure,
    },
    SaveFailed {
        summary: LogExportSummary,
        failure: Failure,
    },
    CancelFailed {
        context: LogExportContext,
        failure: Failure,
    },
}

impl Default for LogExportPhase {
    fn default() -> Self {
        Self::Closed
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogExportActions {
    phase: LogExportPhase,
    sequence: u64,
}

impl LogExportActions {
    pub fn phase(&self) -> &LogExportPhase {
        &self.phase
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.phase, LogExportPhase::Closed)
    }

    pub fn is_pending(&self) -> bool {
        self.pending().is_some()
    }

    pub fn pending(&self) -> Option<&LogExportPending> {
        match &self.phase {
            LogExportPhase::Preparing { pending }
            | LogExportPhase::Saving { pending, .. }
            | LogExportPhase::Cancelling { pending, .. } => Some(pending),
            _ => None,
        }
    }

    pub fn summary(&self) -> Option<&LogExportSummary> {
        match &self.phase {
            LogExportPhase::Review { summary }
            | LogExportPhase::Saving { summary, .. }
            | LogExportPhase::SaveFailed { summary, .. } => Some(summary),
            LogExportPhase::Saved { receipt } => Some(&receipt.summary),
            LogExportPhase::Cancelling { context, .. }
            | LogExportPhase::CancelFailed { context, .. } => Some(context.summary()),
            LogExportPhase::Closed
            | LogExportPhase::Preparing { .. }
            | LogExportPhase::PrepareFailed { .. } => None,
        }
    }

    pub fn receipt(&self) -> Option<&LogExportReceipt> {
        match &self.phase {
            LogExportPhase::Saved { receipt } => Some(receipt),
            LogExportPhase::Cancelling { context, .. }
            | LogExportPhase::CancelFailed { context, .. } => context.receipt(),
            _ => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            LogExportPhase::PrepareFailed { failure }
            | LogExportPhase::SaveFailed { failure, .. }
            | LogExportPhase::CancelFailed { failure, .. } => Some(failure),
            _ => None,
        }
    }

    pub fn can_confirm(&self) -> bool {
        matches!(self.phase, LogExportPhase::Review { .. })
    }

    pub fn can_close(&self) -> bool {
        self.is_open() && !self.is_pending()
    }

    pub fn show(&mut self) -> Option<LogExportPending> {
        if !matches!(self.phase, LogExportPhase::Closed) {
            return None;
        }
        let pending = self.begin(CommandIntent::PrepareLogExport);
        self.phase = LogExportPhase::Preparing {
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn confirm(&mut self) -> Option<LogExportPending> {
        let LogExportPhase::Review { summary } = self.phase.clone() else {
            return None;
        };
        let pending = self.begin(CommandIntent::SaveLogExport {
            identity: summary.identity.clone(),
        });
        self.phase = LogExportPhase::Saving {
            summary,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn cancel(&mut self) -> Option<LogExportPending> {
        let context = match self.phase.clone() {
            LogExportPhase::Review { summary } | LogExportPhase::SaveFailed { summary, .. } => {
                LogExportContext::Review(summary)
            }
            LogExportPhase::Saved { receipt } => LogExportContext::Saved(receipt),
            LogExportPhase::CancelFailed { context, .. } => context,
            LogExportPhase::PrepareFailed { .. } => {
                self.phase = LogExportPhase::Closed;
                return None;
            }
            LogExportPhase::Closed
            | LogExportPhase::Preparing { .. }
            | LogExportPhase::Saving { .. }
            | LogExportPhase::Cancelling { .. } => return None,
        };
        let pending = self.begin(CommandIntent::CancelLogExport {
            identity: context.identity(),
        });
        self.phase = LogExportPhase::Cancelling {
            context,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn can_retry(&self) -> bool {
        let failure = match &self.phase {
            LogExportPhase::PrepareFailed { failure }
            | LogExportPhase::SaveFailed { failure, .. }
            | LogExportPhase::CancelFailed { failure, .. } => failure,
            _ => return false,
        };
        failure.retryable || failure.code == ErrorCode::Permission
    }

    pub fn retry(&mut self) -> Option<LogExportPending> {
        if !self.can_retry() {
            return None;
        }
        let pending = match self.phase.clone() {
            LogExportPhase::PrepareFailed { .. } => {
                let pending = self.begin(CommandIntent::PrepareLogExport);
                self.phase = LogExportPhase::Preparing {
                    pending: pending.clone(),
                };
                pending
            }
            LogExportPhase::SaveFailed { summary, .. } => {
                let pending = self.begin(CommandIntent::SaveLogExport {
                    identity: summary.identity.clone(),
                });
                self.phase = LogExportPhase::Saving {
                    summary,
                    pending: pending.clone(),
                };
                pending
            }
            LogExportPhase::CancelFailed { context, .. } => {
                let pending = self.begin(CommandIntent::CancelLogExport {
                    identity: context.identity(),
                });
                self.phase = LogExportPhase::Cancelling {
                    context,
                    pending: pending.clone(),
                };
                pending
            }
            _ => return None,
        };
        Some(pending)
    }

    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) -> bool {
        let (pending, terminal) = match self.phase.clone() {
            LogExportPhase::Preparing { pending } => (pending, PendingLogExport::Prepare),
            LogExportPhase::Saving { summary, pending } => {
                (pending, PendingLogExport::Save(summary))
            }
            LogExportPhase::Cancelling { context, pending } => {
                (pending, PendingLogExport::Cancel(context))
            }
            _ => return false,
        };
        if pending.operation != operation {
            return false;
        }
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            if let (PendingLogExport::Save(summary), CommandOutput::LogExportSaved(receipt)) =
                (&terminal, &output)
            {
                receipt.validate(summary)?;
            }
            Ok(output)
        });
        self.phase = match (terminal, result) {
            (PendingLogExport::Prepare, Ok(CommandOutput::LogExportPrepared(summary))) => {
                LogExportPhase::Review { summary }
            }
            (PendingLogExport::Prepare, Err(failure)) => LogExportPhase::PrepareFailed { failure },
            (PendingLogExport::Save(_), Ok(CommandOutput::LogExportSaved(receipt))) => {
                LogExportPhase::Saved { receipt }
            }
            (PendingLogExport::Save(summary), Err(failure)) => {
                LogExportPhase::SaveFailed { summary, failure }
            }
            (PendingLogExport::Cancel(_), Ok(CommandOutput::Unit)) => LogExportPhase::Closed,
            (PendingLogExport::Cancel(context), Err(failure)) => {
                LogExportPhase::CancelFailed { context, failure }
            }
            (_, Ok(_)) => unreachable!("typed output validation rejected unrelated output"),
        };
        true
    }

    fn begin(&mut self, intent: CommandIntent) -> LogExportPending {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("log operation identity exhausted");
        LogExportPending {
            operation: self.sequence,
            intent,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingLogExport {
    Prepare,
    Save(LogExportSummary),
    Cancel(LogExportContext),
}
