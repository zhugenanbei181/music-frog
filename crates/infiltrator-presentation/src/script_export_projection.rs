//! Shared export review copy and action gates for both native products.
use crate::script_workbench::{ScriptExportPhase, ScriptWorkbench};
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
    let export = model.export();
    let status = if let Some(failure) = model.failure() {
        failure.message.clone()
    } else if model.is_exporting() {
        lang.tr("script_export_busy").into_owned()
    } else {
        match export.map(|snapshot| &snapshot.outcome) {
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
    let (details, content, path) = export
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
                .review()
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
        confirm: matches!(model.export_phase(), ScriptExportPhase::Review { .. }),
        close: !model.is_exporting(),
        retry: model.can_retry(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_workbench::{ScriptExportPhase, ScriptWorkbench};
    use infiltrator_contract::script_export_review::{
        ScriptExportDraft, ScriptExportValidationScope,
    };
    use lurker_core_contract::error::Failure;

    fn draft() -> ScriptExportDraft {
        ScriptExportDraft {
            kind: ScriptExportKind::DirectiveDslScript,
            validation_scope: ScriptExportValidationScope::Unscoped,
            profile: None,
            base_yaml: String::new(),
            mixin_yaml: String::new(),
            script_code: "return;".into(),
            preset: None,
        }
    }

    #[test]
    fn hidden_export_does_not_offer_confirmation_and_preparation_is_explicitly_busy() {
        let mut model = ScriptWorkbench::default();
        assert!(!project_script_export(&model, "en-US").confirm);
        let pending = model.prepare_export(draft()).unwrap();
        assert!(matches!(
            model.export_phase(),
            ScriptExportPhase::Preparing { .. }
        ));
        assert!(model.is_exporting());
        assert!(!project_script_export(&model, "en-US").confirm);
        model.finish(pending.operation + 1, Err(Failure::unsupported("foreign")));
        assert!(model.is_exporting());
        assert!(!project_script_export(&model, "en-US").close);
    }
}
