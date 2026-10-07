//! Script workbench native actions have one owner and one TEA entry.
use iced::widget::text_editor;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_run::ScriptEditorField;

#[derive(Clone, Debug)]
pub enum ScriptAction {
    Run,
    Clear,
    SelectPreset(String),
    EditCode(String),
    EditYaml(String),
    EditDocument {
        field: ScriptEditorField,
        action: Box<text_editor::Action>,
    },
    Export(ScriptExportKind),
    ConfirmExport,
    CancelExport,
    Retry,
    Finished {
        operation: u64,
        result: Box<Result<CommandOutput, Failure>>,
    },
}
