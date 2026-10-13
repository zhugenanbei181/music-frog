//! Shared user copy and action availability, replayed by native modal renderers.
use crate::log_export_actions::{LogExportActions, LogExportPhase};
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
    let key = match model.phase() {
        LogExportPhase::Preparing { .. } => "logs_export_preparing",
        LogExportPhase::Saving { .. } => "logs_export_saving",
        LogExportPhase::Cancelling { .. } => "logs_export_cancelling",
        LogExportPhase::PrepareFailed { .. }
        | LogExportPhase::SaveFailed { .. }
        | LogExportPhase::CancelFailed { .. } => "logs_export_failed",
        LogExportPhase::Saved { .. } => "logs_export_saved",
        LogExportPhase::Review { .. } => "logs_export_review",
        LogExportPhase::Closed => "logs_export_review",
    };
    let mut status = lang.tr(key).into_owned();
    if let Some(failure) = model.failure() {
        status.push_str(": ");
        status.push_str(&failure.message);
    }
    let details = model
        .summary()
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
            .receipt()
            .map(|receipt| receipt.path.clone())
            .unwrap_or_default(),
        confirm: model.can_confirm(),
        retry: model.can_retry(),
        close: model.can_close(),
        error: model.failure().is_some(),
    }
}
