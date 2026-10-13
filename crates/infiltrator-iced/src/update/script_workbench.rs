//! Native events submit the same session-owned command service as other peers.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportValidationScope};
use infiltrator_contract::script_run::ScriptEditorField;
use infiltrator_presentation::script_workbench::ScriptPending;

impl AppState {
    pub(crate) fn update_script_workbench(&mut self, message: Message) -> Task<Message> {
        if self.shell.ime.is_composing()
            && matches!(
                message,
                Message::Script(
                    ScriptAction::Run
                        | ScriptAction::Export(_)
                        | ScriptAction::SelectPreset(_)
                        | ScriptAction::Clear
                        | ScriptAction::Retry
                )
            )
        {
            return Task::none();
        }
        let request = match message {
            Message::Script(ScriptAction::Run) => self.editor.script_sandbox.run(),
            Message::Script(ScriptAction::Clear) => self.editor.script_sandbox.clear(),
            Message::Script(ScriptAction::Export(kind)) => {
                let (profile, validation_scope) = match kind {
                    ScriptExportKind::MixinOverlayYaml => {
                        match self.editor.document_session.source() {
                            Some(source) => (
                                Some(source.document.profile_id.as_str().to_owned()),
                                ScriptExportValidationScope::ProfileDocument(
                                    source.document.clone(),
                                ),
                            ),
                            None => (
                                self.editor_profile_name(),
                                ScriptExportValidationScope::Unscoped,
                            ),
                        }
                    }
                    ScriptExportKind::DirectiveDslScript
                    | ScriptExportKind::ExtensionPackageJson => (
                        self.editor_profile_name(),
                        ScriptExportValidationScope::Unscoped,
                    ),
                };
                let draft = ScriptExportDraft {
                    kind,
                    validation_scope,
                    profile,
                    base_yaml: self.editor.editor_content.text(),
                    mixin_yaml: self.editor.mixin_content.text(),
                    script_code: self.editor.script_sandbox.script_code().to_owned(),
                    preset: self
                        .editor
                        .script_sandbox
                        .selected_preset()
                        .map(str::to_owned),
                };
                self.editor.script_sandbox.prepare_export(draft)
            }
            Message::Script(ScriptAction::ConfirmExport) => {
                self.editor.script_sandbox.confirm_export()
            }
            Message::Script(ScriptAction::CancelExport) => {
                self.editor.script_sandbox.cancel_export()
            }
            Message::Script(ScriptAction::Retry) => self.editor.script_sandbox.retry(),
            Message::Script(ScriptAction::Finished { operation, result }) => {
                self.editor.script_sandbox.finish(operation, *result);
                return Task::none();
            }
            Message::Script(ScriptAction::SelectPreset(preset)) => {
                self.editor.script_sandbox.select_preset(&preset);
                if !self.editor.script_sandbox.busy()
                    && !self.editor.script_sandbox.is_export_visible()
                {
                    self.editor.script_code_content =
                        text_editor::Content::with_text(self.editor.script_sandbox.script_code());
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditCode(text)) => {
                self.editor.script_sandbox.edit_script(text);
                if !self.editor.script_sandbox.busy()
                    && !self.editor.script_sandbox.is_export_visible()
                {
                    self.editor.script_code_content =
                        text_editor::Content::with_text(self.editor.script_sandbox.script_code());
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditYaml(text)) => {
                self.editor.script_sandbox.edit_yaml(text);
                if !self.editor.script_sandbox.busy()
                    && !self.editor.script_sandbox.is_export_visible()
                {
                    self.editor.script_yaml_content =
                        text_editor::Content::with_text(self.editor.script_sandbox.input_yaml());
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditDocument { field, action }) => {
                if self.editor.script_sandbox.is_export_visible()
                    || (self.editor.script_sandbox.busy() && action.is_edit())
                {
                    return Task::none();
                }
                match field {
                    ScriptEditorField::Code => {
                        self.editor.script_code_content.perform(*action);
                        self.editor
                            .script_sandbox
                            .edit_script(self.editor.script_code_content.text());
                    }
                    ScriptEditorField::InputYaml => {
                        self.editor.script_yaml_content.perform(*action);
                        self.editor
                            .script_sandbox
                            .edit_yaml(self.editor.script_yaml_content.text());
                    }
                }
                return Task::none();
            }
            _ => unreachable!("script workbench message family"),
        };
        request
            .map(|request| self.submit_script_request(request))
            .unwrap_or_else(Task::none)
    }
    fn submit_script_request(&mut self, request: ScriptPending) -> Task<Message> {
        let operation = request.operation;
        let Some(commands) = self.command_ingress() else {
            self.editor.script_sandbox.finish(
                operation,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Script command service is unavailable",
                    true,
                )),
            );
            return Task::none();
        };
        Task::perform(
            async move { commands.execute(request.intent).await.into_output() },
            move |result| {
                Message::Script(ScriptAction::Finished {
                    operation,
                    result: Box::new(result),
                })
            },
        )
    }
}
