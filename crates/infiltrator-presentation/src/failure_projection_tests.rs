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
