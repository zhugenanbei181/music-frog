//! Runtime-neutral script editor and reviewed-export transitions for both peers.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::script_export::ScriptExportSnapshot;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportReview};
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_domain::script_engine::ScriptEngine;
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptPending {
    pub operation: u64,
    pub intent: CommandIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptExecutionPhase {
    Idle,
    Running {
        pending: ScriptPending,
    },
    Clearing {
        pending: ScriptPending,
    },
    Failed {
        failure: Failure,
        retry: ScriptRetry,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptRetry {
    Run(ScriptRunRequest),
    Clear(ScriptOperationId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptExportPhase {
    Hidden,
    Preparing {
        draft: ScriptExportDraft,
        pending: ScriptPending,
    },
    Review {
        review: ScriptExportReview,
    },
    Saving {
        review: ScriptExportReview,
        pending: ScriptPending,
    },
    Saved {
        review: ScriptExportReview,
        snapshot: ScriptExportSnapshot,
    },
    Cancelling {
        review: ScriptExportReview,
        pending: ScriptPending,
    },
    PrepareFailed {
        draft: ScriptExportDraft,
        failure: Failure,
    },
    SaveFailed {
        review: ScriptExportReview,
        failure: Failure,
    },
    CancelFailed {
        review: ScriptExportReview,
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptWorkbench {
    script_code: String,
    input_yaml: String,
    selected_preset: Option<String>,
    snapshot: Option<ScriptSandboxSnapshot>,
    observed_export: Option<ScriptExportSnapshot>,
    execution: ScriptExecutionPhase,
    export_phase: ScriptExportPhase,
    next_operation: u64,
    run_revision: u64,
}

impl Default for ScriptExecutionPhase {
    fn default() -> Self {
        Self::Idle
    }
}

impl Default for ScriptExportPhase {
    fn default() -> Self {
        Self::Hidden
    }
}

impl ScriptWorkbench {
    pub fn execution_phase(&self) -> &ScriptExecutionPhase {
        &self.execution
    }

    pub fn export_phase(&self) -> &ScriptExportPhase {
        &self.export_phase
    }

    pub fn script_code(&self) -> &str {
        &self.script_code
    }

    pub fn input_yaml(&self) -> &str {
        &self.input_yaml
    }

    pub fn selected_preset(&self) -> Option<&str> {
        self.selected_preset.as_deref()
    }

    pub fn snapshot(&self) -> Option<&ScriptSandboxSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn export(&self) -> Option<&ScriptExportSnapshot> {
        match &self.export_phase {
            ScriptExportPhase::Review { review }
            | ScriptExportPhase::Saving { review, .. }
            | ScriptExportPhase::Cancelling { review, .. }
            | ScriptExportPhase::SaveFailed { review, .. }
            | ScriptExportPhase::CancelFailed { review, .. } => Some(&review.snapshot),
            ScriptExportPhase::Saved { snapshot, .. } => Some(snapshot),
            ScriptExportPhase::Hidden
            | ScriptExportPhase::Preparing { .. }
            | ScriptExportPhase::PrepareFailed { .. } => self.observed_export.as_ref(),
        }
    }

    pub fn review(&self) -> Option<&ScriptExportReview> {
        match &self.export_phase {
            ScriptExportPhase::Review { review }
            | ScriptExportPhase::Saving { review, .. }
            | ScriptExportPhase::Saved { review, .. }
            | ScriptExportPhase::Cancelling { review, .. }
            | ScriptExportPhase::SaveFailed { review, .. }
            | ScriptExportPhase::CancelFailed { review, .. } => Some(review),
            ScriptExportPhase::Hidden
            | ScriptExportPhase::Preparing { .. }
            | ScriptExportPhase::PrepareFailed { .. } => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match (&self.execution, &self.export_phase) {
            (ScriptExecutionPhase::Failed { failure, .. }, _) => Some(failure),
            (_, ScriptExportPhase::PrepareFailed { failure, .. })
            | (_, ScriptExportPhase::SaveFailed { failure, .. })
            | (_, ScriptExportPhase::CancelFailed { failure, .. }) => Some(failure),
            _ => None,
        }
    }

    pub fn pending(&self) -> Option<&ScriptPending> {
        match (&self.execution, &self.export_phase) {
            (ScriptExecutionPhase::Running { pending }, _)
            | (ScriptExecutionPhase::Clearing { pending }, _) => Some(pending),
            (_, ScriptExportPhase::Preparing { pending, .. })
            | (_, ScriptExportPhase::Saving { pending, .. })
            | (_, ScriptExportPhase::Cancelling { pending, .. }) => Some(pending),
            _ => None,
        }
    }

    pub fn is_export_visible(&self) -> bool {
        !matches!(self.export_phase, ScriptExportPhase::Hidden)
    }

    pub fn has_local_actions(&self) -> bool {
        self.next_operation != 0
    }

    pub fn busy(&self) -> bool {
        self.pending().is_some()
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.execution,
            ScriptExecutionPhase::Running { .. } | ScriptExecutionPhase::Clearing { .. }
        )
    }

    pub fn is_exporting(&self) -> bool {
        matches!(
            self.export_phase,
            ScriptExportPhase::Preparing { .. }
                | ScriptExportPhase::Saving { .. }
                | ScriptExportPhase::Cancelling { .. }
        )
    }

    pub fn error_detail(&self) -> Option<&str> {
        self.failure()
            .map(|failure| failure.message.as_str())
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.error_detail.as_deref())
            })
    }

    pub fn set_script_code(&mut self, text: String) {
        if !self.busy() && !self.is_export_visible() {
            self.script_code = text;
        }
    }

    pub fn set_input_yaml(&mut self, text: String) {
        if !self.busy() && !self.is_export_visible() {
            self.input_yaml = text;
        }
    }

    pub fn edit_script(&mut self, text: String) {
        self.set_script_code(text);
    }

    pub fn edit_yaml(&mut self, text: String) {
        self.set_input_yaml(text);
    }

    pub fn select_preset(&mut self, preset: &str) {
        if self.busy() || self.is_export_visible() {
            return;
        }
        if let Some(definition) = ScriptEngine::find_preset(preset) {
            self.selected_preset = Some(preset.to_owned());
            self.script_code = definition.script_code.to_owned();
        }
    }

    pub fn observe_snapshot(&mut self, snapshot: Option<ScriptSandboxSnapshot>) {
        self.snapshot = snapshot;
    }

    pub fn observe_export(&mut self, export: Option<ScriptExportSnapshot>) {
        self.observed_export = export;
    }

    pub fn run(&mut self) -> Option<ScriptPending> {
        if self.is_export_visible() || self.busy() {
            return None;
        }
        let operation = self.next_operation.checked_add(1)?;
        let request = ScriptRunRequest {
            operation: ScriptOperationId(operation),
            script_code: self.script_code.clone(),
            input_yaml: self.input_yaml.clone(),
            preset: self.selected_preset.clone(),
        };
        let pending = self.begin(CommandIntent::RunScriptSandbox {
            request: request.clone(),
        })?;
        self.execution = ScriptExecutionPhase::Running {
            pending: pending.clone(),
        };
        self.export_phase = ScriptExportPhase::Hidden;
        Some(pending)
    }

    pub fn clear(&mut self) -> Option<ScriptPending> {
        if self.is_export_visible() || self.busy() {
            return None;
        }
        let operation = self.next_operation.checked_add(1)?;
        let typed_operation = ScriptOperationId(operation);
        let pending = self.begin(CommandIntent::ClearScriptSandbox {
            operation: typed_operation,
        })?;
        self.execution = ScriptExecutionPhase::Clearing {
            pending: pending.clone(),
        };
        self.export_phase = ScriptExportPhase::Hidden;
        Some(pending)
    }

    pub fn prepare_export(&mut self, draft: ScriptExportDraft) -> Option<ScriptPending> {
        if self.busy() || self.is_export_visible() {
            return None;
        }
        self.observed_export = None;
        let pending = self.begin(CommandIntent::PrepareScriptExport {
            draft: draft.clone(),
        })?;
        self.execution = ScriptExecutionPhase::Idle;
        self.export_phase = ScriptExportPhase::Preparing {
            draft,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn confirm_export(&mut self) -> Option<ScriptPending> {
        let ScriptExportPhase::Review { review } = self.export_phase.clone() else {
            return None;
        };
        let pending = self.begin(CommandIntent::SaveScriptExport {
            identity: review.identity.clone(),
        })?;
        self.export_phase = ScriptExportPhase::Saving {
            review,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn cancel_export(&mut self) -> Option<ScriptPending> {
        let review = match self.export_phase.clone() {
            ScriptExportPhase::Review { review }
            | ScriptExportPhase::Saved { review, .. }
            | ScriptExportPhase::SaveFailed { review, .. }
            | ScriptExportPhase::CancelFailed { review, .. } => review,
            ScriptExportPhase::PrepareFailed { .. } => {
                self.observed_export = None;
                self.export_phase = ScriptExportPhase::Hidden;
                return None;
            }
            ScriptExportPhase::Hidden
            | ScriptExportPhase::Preparing { .. }
            | ScriptExportPhase::Saving { .. }
            | ScriptExportPhase::Cancelling { .. } => return None,
        };
        let pending = self.begin(CommandIntent::CancelScriptExport {
            identity: review.identity.clone(),
        })?;
        self.export_phase = ScriptExportPhase::Cancelling {
            review,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn can_retry(&self) -> bool {
        let failure = match (&self.execution, &self.export_phase) {
            (ScriptExecutionPhase::Failed { failure, .. }, _) => failure,
            (_, ScriptExportPhase::PrepareFailed { failure, .. })
            | (_, ScriptExportPhase::SaveFailed { failure, .. })
            | (_, ScriptExportPhase::CancelFailed { failure, .. }) => failure,
            _ => return false,
        };
        !matches!(
            failure.code,
            ErrorCode::Unsupported | ErrorCode::InvalidInput | ErrorCode::Configuration
        )
    }

    pub fn retry(&mut self) -> Option<ScriptPending> {
        if !self.can_retry() {
            return None;
        }
        match (self.execution.clone(), self.export_phase.clone()) {
            (
                ScriptExecutionPhase::Failed {
                    retry: ScriptRetry::Run(request),
                    ..
                },
                _,
            ) => {
                let pending = self.begin(CommandIntent::RunScriptSandbox {
                    request: request.clone(),
                })?;
                self.execution = ScriptExecutionPhase::Running {
                    pending: pending.clone(),
                };
                Some(pending)
            }
            (
                ScriptExecutionPhase::Failed {
                    retry: ScriptRetry::Clear(operation),
                    ..
                },
                _,
            ) => {
                let pending = self.begin(CommandIntent::ClearScriptSandbox { operation })?;
                self.execution = ScriptExecutionPhase::Clearing {
                    pending: pending.clone(),
                };
                Some(pending)
            }
            (_, ScriptExportPhase::PrepareFailed { draft, .. }) => {
                let pending = self.begin(CommandIntent::PrepareScriptExport {
                    draft: draft.clone(),
                })?;
                self.export_phase = ScriptExportPhase::Preparing {
                    draft,
                    pending: pending.clone(),
                };
                Some(pending)
            }
            (_, ScriptExportPhase::SaveFailed { review, .. }) => {
                let pending = self.begin(CommandIntent::SaveScriptExport {
                    identity: review.identity.clone(),
                })?;
                self.export_phase = ScriptExportPhase::Saving {
                    review,
                    pending: pending.clone(),
                };
                Some(pending)
            }
            (_, ScriptExportPhase::CancelFailed { review, .. }) => {
                let pending = self.begin(CommandIntent::CancelScriptExport {
                    identity: review.identity.clone(),
                })?;
                self.export_phase = ScriptExportPhase::Cancelling {
                    review,
                    pending: pending.clone(),
                };
                Some(pending)
            }
            _ => None,
        }
    }

    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) {
        match self.execution.clone() {
            ScriptExecutionPhase::Running { pending } if pending.operation == operation => {
                self.finish_run(pending, result);
            }
            ScriptExecutionPhase::Clearing { pending } if pending.operation == operation => {
                self.finish_clear(pending, result);
            }
            _ => self.finish_export(operation, result),
        }
    }

    fn finish_run(&mut self, pending: ScriptPending, result: Result<CommandOutput, Failure>) {
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            match output {
                CommandOutput::ScriptSandboxRun(result) if result.revision > self.run_revision => {
                    Ok(result)
                }
                _ => Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Stale or unrelated script run terminal",
                    false,
                )),
            }
        });
        match result {
            Ok(result) => {
                self.run_revision = result.revision;
                self.snapshot = Some(result.snapshot);
                self.execution = ScriptExecutionPhase::Idle;
            }
            Err(failure) => {
                let retry = match pending.intent {
                    CommandIntent::RunScriptSandbox { request } => ScriptRetry::Run(request),
                    _ => unreachable!("running phase only admits run commands"),
                };
                self.execution = ScriptExecutionPhase::Failed { failure, retry };
            }
        }
    }

    fn finish_clear(&mut self, pending: ScriptPending, result: Result<CommandOutput, Failure>) {
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            match output {
                CommandOutput::ScriptSandboxCleared(receipt)
                    if receipt.revision > self.run_revision =>
                {
                    Ok(receipt)
                }
                _ => Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Stale or unrelated script clear terminal",
                    false,
                )),
            }
        });
        match result {
            Ok(receipt) => {
                self.run_revision = receipt.revision;
                self.snapshot = None;
                self.execution = ScriptExecutionPhase::Idle;
            }
            Err(failure) => {
                let retry = match pending.intent {
                    CommandIntent::ClearScriptSandbox { operation } => {
                        ScriptRetry::Clear(operation)
                    }
                    _ => unreachable!("clearing phase only admits clear commands"),
                };
                self.execution = ScriptExecutionPhase::Failed { failure, retry };
            }
        }
    }

    fn finish_export(&mut self, operation: u64, result: Result<CommandOutput, Failure>) {
        let (pending, phase) = match self.export_phase.clone() {
            ScriptExportPhase::Preparing { draft, pending } => {
                (pending, PendingScriptExport::Prepare(draft))
            }
            ScriptExportPhase::Saving { review, pending } => {
                (pending, PendingScriptExport::Save(review))
            }
            ScriptExportPhase::Cancelling { review, pending } => {
                (pending, PendingScriptExport::Cancel(review))
            }
            _ => return,
        };
        if pending.operation != operation {
            return;
        }
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            Ok(output)
        });
        self.export_phase = match (phase, result) {
            (PendingScriptExport::Prepare(_), Ok(CommandOutput::ScriptExportPrepared(review))) => {
                self.observed_export = Some(review.snapshot.clone());
                ScriptExportPhase::Review { review: *review }
            }
            (PendingScriptExport::Prepare(draft), Err(failure)) => {
                ScriptExportPhase::PrepareFailed { draft, failure }
            }
            (PendingScriptExport::Save(review), Ok(CommandOutput::ScriptExportSaved(saved))) => {
                let snapshot = saved.snapshot;
                self.observed_export = Some(snapshot.clone());
                ScriptExportPhase::Saved { review, snapshot }
            }
            (PendingScriptExport::Save(review), Err(failure)) => {
                ScriptExportPhase::SaveFailed { review, failure }
            }
            (PendingScriptExport::Cancel(_), Ok(CommandOutput::Unit)) => {
                self.observed_export = None;
                ScriptExportPhase::Hidden
            }
            (PendingScriptExport::Cancel(review), Err(failure)) => {
                ScriptExportPhase::CancelFailed { review, failure }
            }
            (_, Ok(_)) => unreachable!("typed output validation rejected unrelated output"),
        };
    }

    fn begin(&mut self, intent: CommandIntent) -> Option<ScriptPending> {
        if self.busy() {
            return None;
        }
        self.next_operation = self.next_operation.checked_add(1)?;
        let pending = ScriptPending {
            operation: self.next_operation,
            intent,
        };
        Some(pending)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingScriptExport {
    Prepare(ScriptExportDraft),
    Save(ScriptExportReview),
    Cancel(ScriptExportReview),
}
