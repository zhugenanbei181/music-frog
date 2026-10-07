//! Shared user copy and action availability, replayed by native modal renderers.
use crate::log_export_actions::LogExportActions;
use infiltrator_contract::command::CommandIntent;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct LogExportView {
    pub status: String,
    pub details: String,
    pub path: String,
    pub confirm: bool,
    pub retry: bool,
    pub close: bool,
    pub error: bool,
}
pub fn project_log_export(model: &LogExportActions, language: &str) -> LogExportView {
    let lang = Lang(language);
    let key = if let Some(pending) = &model.pending {
        match pending.intent {
            CommandIntent::PrepareLogExport => "logs_export_preparing",
            CommandIntent::SaveLogExport { .. } => "logs_export_saving",
            _ => "logs_export_cancelling",
        }
    } else if model.failure.is_some() {
        "logs_export_failed"
    } else if model.receipt.is_some() {
        "logs_export_saved"
    } else {
        "logs_export_review"
    };
    let mut status = lang.tr(key).into_owned();
    if let Some(failure) = &model.failure {
        status.push_str(": ");
        status.push_str(&failure.message);
    }
    let details = model
        .summary
        .as_ref()
        .map(|summary| {
            interpolate(
                lang.tr("logs_export_details").as_ref(),
                &[
                    ("records", &summary.records.to_string()),
                    ("bytes", &summary.bytes.to_string()),
                ],
            )
        })
        .unwrap_or_default();
    LogExportView {
        status,
        details,
        path: model
            .receipt
            .as_ref()
            .map(|receipt| receipt.path.clone())
            .unwrap_or_default(),
        confirm: model.open
            && model.summary.is_some()
            && model.pending.is_none()
            && model.failure.is_none()
            && model.receipt.is_none(),
        retry: model.can_retry(),
        close: model.open && model.pending.is_none(),
        error: model.failure.is_some(),
    }
}
