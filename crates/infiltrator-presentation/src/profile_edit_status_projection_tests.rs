use super::*;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};

#[test]
fn editor_failure_copy_localizes_typed_reason_and_keeps_raw_diagnostic() {
    let fact = ProfileEditStatusFact::ReadFailed(
        Failure::new(ErrorCode::Configuration, "opaque {line}", false)
            .with_reason(FailureReason::YamlSyntax { line: 7, column: 3 }),
    );
    assert_eq!(
        status(&fact, "en-US"),
        "Editor read failed: YAML syntax error at line 7, column 3\nDiagnostic detail: opaque {line}"
    );
    assert_eq!(
        status(&fact, "zh-CN"),
        "编辑器读取失败：YAML 语法错误（第 7 行，第 3 列）\n诊断详情：opaque {line}"
    );
}

#[test]
fn ready_has_no_extra_status_copy_and_source_change_is_explicit() {
    assert_eq!(status(&ProfileEditStatusFact::Ready, "en-US"), "");
    assert_eq!(
        status(&ProfileEditStatusFact::SourceChanged, "en-US"),
        "The profile or options changed; discard this draft to inspect the current source"
    );
}
