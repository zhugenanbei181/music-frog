//! DUAL-10-12: the Iced export dispatch — one entry point for every kind.
//!
//! The surface owns no export policy: it composes nothing and decides nothing
//! about the destination. It hands the live drafts to the shared
//! [`ScriptExportApplication`] together with the host save-file port (or no
//! port at all, which the application reports as typed unsupported), then the
//! published [`ScriptExportSnapshot`] is rendered by both surfaces.

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::script_export::ScriptExportKind;

impl AppState {
    /// Export the open draft through the shared use-case and the host port.
    pub(crate) fn export_script_draft(&mut self, kind: ScriptExportKind) -> Task<Message> {
        if self.editor.script_sandbox.is_exporting {
            return Task::none();
        }
        // The host port is the only destination decision; `None` is a real,
        // typed "this host has no file dialog or export directory" answer.
        let port = self
            .runtime
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.script_export_port());
        let profile = self.editor_profile_name();
        let base_yaml = self.editor.editor_content.text();
        let mixin_yaml = self.editor.mixin_content.text();
        let script_code = self.editor.script_sandbox.script_code.clone();
        let preset = self.editor.script_sandbox.selected_preset.clone();

        self.editor.script_sandbox.is_exporting = true;
        Task::perform(
            async move {
                let application = ScriptExportApplication::new(port);
                let snapshot = match kind {
                    ScriptExportKind::MixinOverlayYaml => {
                        let profile = profile.ok_or_else(|| {
                            InfiltratorError::Config(
                                "没有打开任何配置，Mixin 覆写未导出".to_string(),
                            )
                        })?;
                        application.export_mixin_overlay(&profile, &base_yaml, &mixin_yaml)
                    }
                    ScriptExportKind::DirectiveDslScript => application.export_directive_dsl(
                        preset.as_deref(),
                        &script_code,
                        preset.as_deref(),
                    ),
                    ScriptExportKind::ExtensionPackageJson => application.export_draft_package(
                        script_code_package_name(profile.as_deref(), preset.as_deref()).as_str(),
                        profile.as_deref(),
                        &script_code,
                        Some(&mixin_yaml),
                        preset.as_deref(),
                    ),
                };
                snapshot.map_err(|failure| InfiltratorError::Config(failure.message))
            },
            Message::ScriptExportFinished,
        )
    }
}

/// The package name a surface export gets: the preset id when one is loaded,
/// otherwise the profile, otherwise a stable default. The application keeps it
/// in the artifact file name after sanitization.
fn script_code_package_name(profile: Option<&str>, preset: Option<&str>) -> String {
    preset
        .or(profile)
        .map(str::to_string)
        .unwrap_or_else(|| "music-frog-extension".to_string())
}
