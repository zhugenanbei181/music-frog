//! One localization boundary for typed product reasons and opaque adapter details.
use infiltrator_contract::error::{Failure, FailureReason};
use infiltrator_shared::i18n_interpolator::localize;

pub fn failure_message(failure: &Failure, locale: &str) -> String {
    let (key, mut values) = match &failure.reason {
        Some(FailureReason::YamlSyntax { line, column }) => (
            "failure_yaml_syntax",
            vec![("line", line.to_string()), ("column", column.to_string())],
        ),
        Some(FailureReason::MixinYaml) => ("failure_mixin_yaml", vec![]),
        Some(FailureReason::ActiveProfileInconsistent) => {
            ("failure_active_profile_inconsistent", vec![])
        }
        Some(FailureReason::QuotaSourceChanged) => ("failure_quota_source_changed", vec![]),
        Some(FailureReason::CurrentTimeUnavailable) => ("failure_current_time_unavailable", vec![]),
        Some(FailureReason::DownloadCanceled) => ("download_canceled", vec![]),
        None => return failure.message.clone(),
    };
    values.push(("detail", failure.message.clone()));
    localize(locale, key, &values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::error::ErrorCode;

    #[test]
    fn serialized_reason_changes_language_without_reinterpreting_diagnostic_data() {
        let failure = Failure::new(ErrorCode::Configuration, "user {line} {detail}", false)
            .with_reason(FailureReason::YamlSyntax { line: 7, column: 3 });
        let wire = serde_json::to_string(&failure).unwrap();
        let observed: Failure = serde_json::from_str(&wire).unwrap();
        assert_eq!(observed, failure);
        assert_eq!(
            failure_message(&observed, "en-US"),
            "YAML syntax error at line 7, column 3: user {line} {detail}"
        );
        assert_eq!(
            failure_message(&observed, "zh-CN"),
            "YAML 语法错误（第 7 行，第 3 列）：user {line} {detail}"
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
}
