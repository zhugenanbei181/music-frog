//! One localization boundary for typed product reasons and opaque adapter details.
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureLocalizationKey {
    InvalidInput,
    InvalidState,
    NotReady,
    Unsupported,
    Network,
    Authentication,
    Configuration,
    Storage,
    Permission,
    Canceled,
    Internal,
    YamlSyntax,
    MixinYaml,
    ActiveProfileInconsistent,
    QuotaSourceChanged,
    CurrentTimeUnavailable,
    DownloadCanceled,
}

impl FailureLocalizationKey {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailureLocalizationProjection {
    pub key: FailureLocalizationKey,
    pub line: Option<usize>,
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

pub fn failure_message(failure: &Failure, locale: &str) -> String {
    if failure.reason.is_none() {
        return failure.message.clone();
    }
    let projection = project_failure(failure);
    let mut values = Vec::new();
    if let Some(line) = projection.line {
        values.push(("line", line.to_string()));
    }
    if let Some(column) = projection.column {
        values.push(("column", column.to_string()));
    }
    let title = localize(locale, projection.key.shared_key(), &values);
    if failure.message.trim().is_empty() {
        title
    } else {
        format!(
            "{title}\n{}",
            localize(
                locale,
                "failure_diagnostic_detail",
                &[("detail", failure.message.clone())],
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::active_profile_reference::{
        ActiveProfileRecoveryOutcome, CoreApplyFailureKind,
    };

    #[test]
    fn serialized_reason_changes_language_without_reinterpreting_diagnostic_data() {
        let failure = Failure::new(ErrorCode::Configuration, "user {line} {detail}", false)
            .with_reason(FailureReason::YamlSyntax { line: 7, column: 3 });
        let wire = serde_json::to_string(&failure).unwrap();
        let observed: Failure = serde_json::from_str(&wire).unwrap();
        assert_eq!(observed, failure);
        assert_eq!(
            failure_message(&observed, "en-US"),
            "YAML syntax error at line 7, column 3\nDiagnostic detail: user {line} {detail}"
        );
        assert_eq!(
            failure_message(&observed, "zh-CN"),
            "YAML 语法错误（第 7 行，第 3 列）\n诊断详情：user {line} {detail}"
        );
        assert_eq!(observed.code, ErrorCode::Configuration);
        assert!(!observed.retryable);
    }

    #[test]
    fn legacy_adapter_detail_stays_opaque_and_each_product_reason_uses_canonical_resources() {
        let legacy: Failure = serde_json::from_str(
            r#"{"code":"Permission","message":"opaque {detail}","retryable":false}"#,
        )
        .unwrap();
        assert!(legacy.reason.is_none());
        assert_eq!(failure_message(&legacy, "zh-CN"), "opaque {detail}");
        for reason in [
            FailureReason::MixinYaml,
            FailureReason::ActiveProfileInconsistent,
            FailureReason::QuotaSourceChanged,
            FailureReason::CurrentTimeUnavailable,
        ] {
            let failure = legacy.clone().with_reason(reason);
            let zh = failure_message(&failure, "zh-CN");
            let en = failure_message(&failure, "en-US");
            assert_ne!(zh, en);
            assert!(zh.contains("opaque {detail}"));
            assert!(en.contains("opaque {detail}"));
            assert!(!en.contains("failure_"));
        }
    }

    #[test]
    fn every_error_code_projects_to_a_closed_localization_key() {
        let cases = [
            (
                ErrorCode::InvalidInput,
                FailureLocalizationKey::InvalidInput,
            ),
            (
                ErrorCode::InvalidState,
                FailureLocalizationKey::InvalidState,
            ),
            (ErrorCode::NotReady, FailureLocalizationKey::NotReady),
            (ErrorCode::Unsupported, FailureLocalizationKey::Unsupported),
            (ErrorCode::Network, FailureLocalizationKey::Network),
            (
                ErrorCode::Authentication,
                FailureLocalizationKey::Authentication,
            ),
            (
                ErrorCode::Configuration,
                FailureLocalizationKey::Configuration,
            ),
            (ErrorCode::Storage, FailureLocalizationKey::Storage),
            (ErrorCode::Permission, FailureLocalizationKey::Permission),
            (ErrorCode::Canceled, FailureLocalizationKey::Canceled),
            (ErrorCode::Internal, FailureLocalizationKey::Internal),
        ];
        for (code, expected) in cases {
            let failure = Failure::new(code, "opaque platform diagnostic", false);
            let projection = project_failure(&failure);
            assert_eq!(projection.key, expected);
            assert_eq!(projection.line, None);
            assert_eq!(projection.column, None);
            assert_eq!(failure.message, "opaque platform diagnostic");
        }
    }

    #[test]
    fn new_active_profile_reasons_fall_back_to_their_typed_error_category() {
        let stale = Failure::new(ErrorCode::NotReady, "stale selection", true).with_reason(
            FailureReason::ActiveProfileReferenceStale {
                expected_revision: 1,
                observed_revision: 2,
            },
        );
        assert_eq!(
            project_failure(&stale).key,
            FailureLocalizationKey::NotReady
        );

        let apply = Failure::new(ErrorCode::Permission, "native apply denied", false).with_reason(
            FailureReason::CoreApplyFailure(CoreApplyFailureKind::Permission),
        );
        assert_eq!(
            project_failure(&apply).key,
            FailureLocalizationKey::Permission
        );

        let recovery = Failure::new(ErrorCode::Internal, "recovery failed", false).with_reason(
            FailureReason::ActiveProfileRecovery(ActiveProfileRecoveryOutcome::Superseded {
                expected_revision: 1,
                observed_revision: 2,
                apply_failure: CoreApplyFailureKind::Adapter,
            }),
        );
        assert_eq!(
            project_failure(&recovery).key,
            FailureLocalizationKey::Internal
        );
    }
}
