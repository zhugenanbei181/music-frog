//! Native events submit the same session-owned command service as other peers.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_application::script_workbench::ScriptPending;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export_review::ScriptExportDraft;
use infiltrator_contract::script_run::ScriptEditorField;

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
                let draft = ScriptExportDraft {
                    kind,
                    profile: self.editor_profile_name(),
                    base_yaml: self.editor.editor_content.text(),
                    mixin_yaml: self.editor.mixin_content.text(),
                    script_code: self.editor.script_sandbox.script_code.clone(),
                    preset: self.editor.script_sandbox.selected_preset.clone(),
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
                if !self.editor.script_sandbox.busy() && !self.editor.script_sandbox.export_visible
                {
                    self.editor.script_code_content =
                        text_editor::Content::with_text(&self.editor.script_sandbox.script_code);
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditCode(text)) => {
                self.editor.script_sandbox.edit_script(text);
                if !self.editor.script_sandbox.busy() && !self.editor.script_sandbox.export_visible
                {
                    self.editor.script_code_content =
                        text_editor::Content::with_text(&self.editor.script_sandbox.script_code);
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditYaml(text)) => {
                self.editor.script_sandbox.edit_yaml(text);
                if !self.editor.script_sandbox.busy() && !self.editor.script_sandbox.export_visible
                {
                    self.editor.script_yaml_content =
                        text_editor::Content::with_text(&self.editor.script_sandbox.input_yaml);
                }
                return Task::none();
            }
            Message::Script(ScriptAction::EditDocument { field, action }) => {
                if self.editor.script_sandbox.export_visible
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
        let Some(commands) = self.commands.clone() else {
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
