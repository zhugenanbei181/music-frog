//! Both peers replay the same frozen content, destination and action gates.
use crate::script_workbench::ScriptWorkbench;
use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct ScriptExportPresentation {
    pub status: String,
    pub details: String,
    pub content: String,
    pub path: String,
    pub confirm: bool,
    pub close: bool,
    pub retry: bool,
}
pub fn project_script_export(model: &ScriptWorkbench, language: &str) -> ScriptExportPresentation {
    let lang = Lang(language);
    let saved = model
        .export
        .as_ref()
        .is_some_and(|export| export.outcome.is_saved());
    let status = if let Some(failure) = &model.failure {
        failure.message.clone()
    } else if model.busy() {
        lang.tr("script_export_busy").into_owned()
    } else {
        match model.export.as_ref().map(|export| &export.outcome) {
            Some(ScriptExportOutcome::Saved { .. }) => {
                lang.tr("script_export_toast_saved").into_owned()
            }
            Some(ScriptExportOutcome::Unsupported { reason }) => {
                format!("{} {reason}", lang.tr("script_export_unsupported"))
            }
            Some(ScriptExportOutcome::Failed { reason }) => {
                format!("{} {reason}", lang.tr("script_export_toast_failed"))
            }
            Some(ScriptExportOutcome::Prepared) => {
                lang.tr("script_export_review_instructions").into_owned()
            }
            None => lang.tr("script_export_unobserved").into_owned(),
        }
    };
    let (details, content, path) = model
        .export
        .as_ref()
        .map(|export| {
            let file = localize(
                language,
                "script_export_file_value",
                &[
                    ("name", export.file_name.clone()),
                    ("bytes", export.content.len().to_string()),
                ],
            );
            let note = match export.kind {
                ScriptExportKind::DirectiveDslScript => "script_export_directive_note",
                ScriptExportKind::MixinOverlayYaml => "script_export_overlay_note",
                ScriptExportKind::ExtensionPackageJson => "script_export_package_note",
            };
            let hash = model
                .review
                .as_ref()
                .map(|review| review.identity.sha256.as_str())
                .unwrap_or("");
            let details = format!(
                "{} · {file}\n{} {hash}\n{}",
                lang.tr(export.kind.label_key()),
                lang.tr("script_export_checksum"),
                lang.tr(note)
            );
            let path = match &export.outcome {
                ScriptExportOutcome::Saved { path, .. } => path.clone(),
                _ => String::new(),
            };
            (details, export.content_preview(1600), path)
        })
        .unwrap_or_default();
    ScriptExportPresentation {
        status,
        details,
        content,
        path,
        confirm: model.export_visible
            && !model.busy()
            && model.review.is_some()
            && !saved
            && model.failure.is_none(),
        close: !model.busy(),
        retry: model.can_retry(),
    }
}
