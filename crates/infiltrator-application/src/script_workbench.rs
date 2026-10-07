//! One native workbench state machine for execution and reviewed exports.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::ScriptExportSnapshot;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportReview};
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_domain::script_engine::ScriptEngine;

#[derive(Clone, Debug)]
pub struct ScriptPending {
    pub operation: u64,
    pub intent: CommandIntent,
}
#[derive(Clone, Debug, Default)]
pub struct ScriptWorkbench {
    pub script_code: String,
    pub input_yaml: String,
    pub selected_preset: Option<String>,
    pub snapshot: Option<ScriptSandboxSnapshot>,
    pub export: Option<ScriptExportSnapshot>,
    pub review: Option<ScriptExportReview>,
    pub export_visible: bool,
    pub failure: Option<Failure>,
    pub pending: Option<ScriptPending>,
    retry: Option<CommandIntent>,
    next_operation: u64,
    run_revision: u64,
}
impl ScriptWorkbench {
    pub fn has_local_actions(&self) -> bool {
        self.next_operation != 0
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn is_running(&self) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            matches!(
                pending.intent,
                CommandIntent::RunScriptSandbox { .. } | CommandIntent::ClearScriptSandbox { .. }
            )
        })
    }
    pub fn is_exporting(&self) -> bool {
        self.busy() && !self.is_running()
    }
    pub fn error_detail(&self) -> Option<&str> {
        self.failure
            .as_ref()
            .map(|failure| failure.message.as_str())
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.error_detail.as_deref())
            })
    }
    pub fn edit_script(&mut self, text: String) {
        if !self.busy() && !self.export_visible {
            self.script_code = text;
        }
    }
    pub fn edit_yaml(&mut self, text: String) {
        if !self.busy() && !self.export_visible {
            self.input_yaml = text;
        }
    }
    pub fn select_preset(&mut self, preset: &str) {
        if self.busy() || self.export_visible {
            return;
        }
        if let Some(definition) = ScriptEngine::find_preset(preset) {
            self.selected_preset = Some(preset.to_owned());
            self.script_code = definition.script_code.to_owned();
        }
    }
    pub fn run(&mut self) -> Option<ScriptPending> {
        if self.export_visible {
            return None;
        }
        let operation = self.next_operation.checked_add(1)?;
        self.begin(CommandIntent::RunScriptSandbox {
            request: ScriptRunRequest {
                operation: ScriptOperationId(operation),
                script_code: self.script_code.clone(),
                input_yaml: self.input_yaml.clone(),
                preset: self.selected_preset.clone(),
            },
        })
    }
    pub fn clear(&mut self) -> Option<ScriptPending> {
        if self.export_visible {
            return None;
        }
        let operation = self.next_operation.checked_add(1)?;
        self.begin(CommandIntent::ClearScriptSandbox {
            operation: ScriptOperationId(operation),
        })
    }
    pub fn prepare_export(&mut self, draft: ScriptExportDraft) -> Option<ScriptPending> {
        if self.busy() || self.export_visible {
            return None;
        }
        self.export_visible = true;
        self.review = None;
        self.export = None;
        self.begin(CommandIntent::PrepareScriptExport { draft })
    }
    pub fn confirm_export(&mut self) -> Option<ScriptPending> {
        if !self.export_visible {
            return None;
        }
        let identity = self.review.as_ref()?.identity.clone();
        self.begin(CommandIntent::SaveScriptExport { identity })
    }
    pub fn cancel_export(&mut self) -> Option<ScriptPending> {
        if self.busy() || !self.export_visible {
            return None;
        }
        let Some(review) = self.review.as_ref() else {
            self.export_visible = false;
            self.failure = None;
            return None;
        };
        self.begin(CommandIntent::CancelScriptExport {
            identity: review.identity.clone(),
        })
    }
    pub fn retry(&mut self) -> Option<ScriptPending> {
        if !self.can_retry() {
            return None;
        }
        let intent = self.retry.clone()?;
        self.begin(intent)
    }
    pub fn can_retry(&self) -> bool {
        !self.busy()
            && self.retry.is_some()
            && self.failure.as_ref().is_some_and(|failure| {
                !matches!(
                    failure.code,
                    ErrorCode::Unsupported | ErrorCode::InvalidInput | ErrorCode::Configuration
                )
            })
    }
    fn begin(&mut self, intent: CommandIntent) -> Option<ScriptPending> {
        if self.busy() {
            return None;
        }
        let operation = self.next_operation.checked_add(1)?;
        self.next_operation = operation;
        let request = ScriptPending { operation, intent };
        self.failure = None;
        self.retry = None;
        self.pending = Some(request.clone());
        Some(request)
    }
    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.operation == operation)
            .cloned()
        else {
            return;
        };
        self.pending = None;
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            Ok(output)
        });
        match result {
            Ok(CommandOutput::ScriptSandboxRun(result)) if result.revision > self.run_revision => {
                self.run_revision = result.revision;
                self.snapshot = Some(result.snapshot);
            }
            Ok(CommandOutput::ScriptSandboxCleared(receipt))
                if receipt.revision > self.run_revision =>
            {
                self.run_revision = receipt.revision;
                self.snapshot = None;
            }
            Ok(CommandOutput::ScriptExportPrepared(review)) => {
                self.export = Some(review.snapshot.clone());
                self.review = Some(*review);
            }
            Ok(CommandOutput::ScriptExportSaved(saved)) => {
                self.export = Some(saved.snapshot);
            }
            Ok(CommandOutput::Unit)
                if matches!(pending.intent, CommandIntent::CancelScriptExport { .. }) =>
            {
                self.review = None;
                self.export = None;
                self.export_visible = false;
            }
            Err(failure) => {
                self.failure = Some(failure);
                self.retry = Some(pending.intent);
            }
            Ok(_) => {
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "Stale or unrelated script terminal",
                    false,
                ));
            }
        }
    }
}
