//! One localization boundary for typed product reasons and opaque adapter details.
//!
//! The projection never interprets the raw adapter message: only the stable
//! [`ErrorCode`] and the typed [`FailureReason`] decide the localization key.
//! The adapter text stays a separate diagnostic detail.
use crate::copy::{TextCatalog, interpolate};
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};

/// The localization key of one projected failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureLocalizationKey {
    /// The caller supplied an invalid argument.
    InvalidInput,
    /// The operation is not legal in the current state.
    InvalidState,
    /// The target is not ready yet.
    NotReady,
    /// The capability is not supported.
    Unsupported,
    /// A network operation failed.
    Network,
    /// Authentication or authorisation failed.
    Authentication,
    /// The configuration was rejected.
    Configuration,
    /// A storage operation failed.
    Storage,
    /// The process lacks the permission the operation needs.
    Permission,
    /// The operation was cancelled.
    Canceled,
    /// An unexpected internal failure occurred.
    Internal,
    /// The YAML document could not be parsed.
    YamlSyntax,
    /// The YAML mixin could not be applied.
    MixinYaml,
    /// The persisted active-profile record is inconsistent.
    ActiveProfileInconsistent,
    /// The quota source changed while the request was in flight.
    QuotaSourceChanged,
    /// The current time could not be determined.
    CurrentTimeUnavailable,
    /// The download was cancelled by the caller.
    DownloadCanceled,
}

impl FailureLocalizationKey {
    /// Returns the stable catalog key of this localization key.
    pub const fn shared_key(self) -> &'static str {
        match self {
            Self::InvalidInput => "failure_invalid_input",
            Self::InvalidState => "failure_invalid_state",
            Self::NotReady => "failure_not_ready",
            Self::Unsupported => "failure_unsupported",
            Self::Network => "failure_network",
            Self::Authentication => "failure_authentication",
            Self::Configuration => "failure_configuration",
            Self::Storage => "failure_storage",
            Self::Permission => "failure_permission",
            Self::Canceled => "failure_canceled",
            Self::Internal => "failure_internal",
            Self::YamlSyntax => "failure_yaml_syntax",
            Self::MixinYaml => "failure_mixin_yaml",
            Self::ActiveProfileInconsistent => "failure_active_profile_inconsistent",
            Self::QuotaSourceChanged => "failure_quota_source_changed",
            Self::CurrentTimeUnavailable => "failure_current_time_unavailable",
            Self::DownloadCanceled => "download_canceled",
        }
    }
}

/// The localization projection of one typed failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailureLocalizationProjection {
    /// The localization key of the failure.
    pub key: FailureLocalizationKey,
    /// The line the failure points at, when the codec reported one.
    pub line: Option<usize>,
    /// The column the failure points at, when the codec reported one.
    pub column: Option<usize>,
}

/// Project only stable contract categories and typed product reasons. The raw
/// adapter message remains a separate diagnostic and is never interpreted as
/// a localization key or parameter.
pub fn project_failure(failure: &Failure) -> FailureLocalizationProjection {
    let key = match &failure.reason {
        Some(FailureReason::YamlSyntax { .. }) => FailureLocalizationKey::YamlSyntax,
        Some(FailureReason::MixinYaml) => FailureLocalizationKey::MixinYaml,
        Some(FailureReason::ActiveProfileInconsistent) => {
            FailureLocalizationKey::ActiveProfileInconsistent
        }
        Some(FailureReason::QuotaSourceChanged) => FailureLocalizationKey::QuotaSourceChanged,
        Some(FailureReason::CurrentTimeUnavailable) => {
            FailureLocalizationKey::CurrentTimeUnavailable
        }
        Some(FailureReason::DownloadCanceled) => FailureLocalizationKey::DownloadCanceled,
        Some(FailureReason::ActiveProfileReferenceStale { .. }) => FailureLocalizationKey::NotReady,
        Some(FailureReason::CoreApplyFailure(_))
        | Some(FailureReason::ActiveProfileRecovery(_))
        | None => localization_key_for_error_code(failure.code.clone()),
    };
    let (line, column) = match &failure.reason {
        Some(FailureReason::YamlSyntax { line, column }) => (Some(*line), Some(*column)),
        _ => (None, None),
    };
    FailureLocalizationProjection { key, line, column }
}

fn localization_key_for_error_code(code: ErrorCode) -> FailureLocalizationKey {
    match code {
        ErrorCode::InvalidInput => FailureLocalizationKey::InvalidInput,
        ErrorCode::InvalidState => FailureLocalizationKey::InvalidState,
        ErrorCode::NotReady => FailureLocalizationKey::NotReady,
        ErrorCode::Unsupported => FailureLocalizationKey::Unsupported,
        ErrorCode::Network => FailureLocalizationKey::Network,
        ErrorCode::Authentication => FailureLocalizationKey::Authentication,
        ErrorCode::Configuration => FailureLocalizationKey::Configuration,
        ErrorCode::Storage => FailureLocalizationKey::Storage,
        ErrorCode::Permission => FailureLocalizationKey::Permission,
        ErrorCode::Canceled => FailureLocalizationKey::Canceled,
        ErrorCode::Internal => FailureLocalizationKey::Internal,
    }
}

/// Renders the localized message of one typed failure.
///
/// A failure without a typed reason keeps its raw message; a failure with a
/// reason renders the localized title plus the opaque diagnostic detail.
pub fn failure_message(failure: &Failure, catalog: &dyn TextCatalog) -> String {
    if failure.reason.is_none() {
        return failure.message.clone();
    }
    let projection = project_failure(failure);
    let mut values: Vec<(&str, String)> = Vec::new();
    if let Some(line) = projection.line {
        values.push(("line", line.to_string()));
    }
    if let Some(column) = projection.column {
        values.push(("column", column.to_string()));
    }
    let values: Vec<(&str, &str)> = values
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let title = interpolate(catalog.text(projection.key.shared_key()).as_ref(), &values);
    if failure.message.trim().is_empty() {
        title
    } else {
        format!(
            "{title}\n{}",
            interpolate(
                catalog.text("failure_diagnostic_detail").as_ref(),
                &[("detail", failure.message.as_str())],
            )
        )
    }
}

#[cfg(test)]
#[path = "failure_projection_tests.rs"]
mod tests;
