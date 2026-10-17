use super::*;
use lurker_core_contract::owner_switch::{ActiveProfileRecoveryOutcome, CoreApplyFailureKind};
use std::borrow::Cow;

/// Synthetic catalog: the neutral kernel is verified against the interpolation
/// contract only. Real-en/zh product copy is asserted by the product-side
/// projection tests that own the catalog.
struct StubCatalog;

impl TextCatalog for StubCatalog {
    fn text(&self, key: &str) -> Cow<'static, str> {
        match key {
            "failure_yaml_syntax" => "[yaml line={line} column={column}]".into(),
            "failure_diagnostic_detail" => "[detail {detail}]".into(),
            other => format!("[{other}]").into(),
        }
    }
}

/// Proves the catalog is never consulted for a reasonless failure.
struct UnusedCatalog;

impl TextCatalog for UnusedCatalog {
    fn text(&self, _key: &str) -> Cow<'static, str> {
        panic!("legacy adapter detail must not be used as a localization key")
    }
}

#[test]
fn serialized_reason_keeps_its_key_and_leaves_diagnostic_data_opaque() {
    let failure = Failure::new(ErrorCode::Configuration, "user {line} {detail}", false)
        .with_reason(FailureReason::YamlSyntax { line: 7, column: 3 });
    let wire = serde_json::to_string(&failure).unwrap();
    let observed: Failure = serde_json::from_str(&wire).unwrap();
    assert_eq!(observed, failure);
    assert_eq!(
        failure_message(&observed, &StubCatalog),
        "[yaml line=7 column=3]\n[detail user {line} {detail}]"
    );
    assert_eq!(observed.code, ErrorCode::Configuration);
    assert!(!observed.retryable);
    assert_eq!(
        project_failure(&observed).key,
        FailureLocalizationKey::YamlSyntax
    );
    assert_eq!(project_failure(&observed).line, Some(7));
    assert_eq!(project_failure(&observed).column, Some(3));
}

#[test]
fn legacy_adapter_detail_stays_opaque_and_each_product_reason_uses_one_canonical_key() {
    let legacy: Failure = serde_json::from_str(
        r#"{"code":"Permission","message":"opaque {detail}","retryable":false}"#,
    )
    .unwrap();
    assert!(legacy.reason.is_none());
    assert_eq!(failure_message(&legacy, &UnusedCatalog), "opaque {detail}");
    let mut keys = Vec::new();
    for reason in [
        FailureReason::MixinYaml,
        FailureReason::ActiveProfileInconsistent,
        FailureReason::QuotaSourceChanged,
        FailureReason::CurrentTimeUnavailable,
    ] {
        let failure = legacy.clone().with_reason(reason);
        let projection = project_failure(&failure);
        let message = failure_message(&failure, &StubCatalog);
        assert!(
            message.starts_with(&format!("[{}]", projection.key.shared_key())),
            "{message}"
        );
        assert!(message.contains("opaque {detail}"), "{message}");
        keys.push(projection.key);
    }
    keys.sort_by_key(|key| key.shared_key());
    keys.dedup();
    assert_eq!(keys.len(), 4);
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
