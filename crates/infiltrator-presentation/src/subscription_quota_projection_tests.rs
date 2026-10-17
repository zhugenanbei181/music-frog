use super::*;
use infiltrator_shared::locales::Lang;
#[test]
fn unknown_and_reported_zero_keep_distinct_values_and_user_identifiers_are_not_templates() {
    let mut snapshot = SubscriptionQuotaSnapshot::default();
    let unknown = project_quota(&snapshot, &Lang("en-US"));
    assert_eq!(unknown.used, "—");
    assert_eq!(unknown.total, "—");
    assert_eq!(unknown.fraction, None);
    assert_eq!(unknown.status, "Quota not observed");
    snapshot.status = SubscriptionQuotaStatus::Ready;
    snapshot.profile_name = Some("用户 {usage} {reason}".into());
    snapshot.used_bytes = Some(0);
    snapshot.total_bytes = Some(0);
    snapshot.usage_percent = Some(0.0);
    let zero = project_quota(&snapshot, &Lang("en-US"));
    assert_eq!(zero.used, "0 B");
    assert_eq!(zero.total, "0 B");
    assert_eq!(zero.usage, "0.0%");
    assert_eq!(zero.fraction, Some(0.0));
    assert_eq!(zero.profile, "用户 {usage} {reason}");
    assert_eq!(zero.status, "Provider metadata observed");
    snapshot.usage_percent = Some(f64::NAN);
    assert_eq!(project_quota(&snapshot, &Lang("zh-CN")).fraction, None);
}
#[test]
fn failure_and_unsupported_are_localized_without_inventing_success_or_translating_backend_reason() {
    let failed = SubscriptionQuotaSnapshot::failed(2, 3, "opaque {reason} cause");
    let en = project_quota(&failed, &Lang("en-US"));
    let zh = project_quota(&failed, &Lang("zh-CN"));
    assert_eq!(en.status, "Quota read failed: opaque {reason} cause");
    assert_eq!(zh.status, "配额读取失败：opaque {reason} cause");
    assert_eq!(en.grade, QuotaGrade::Danger);
    let unsupported = project_quota(
        &SubscriptionQuotaSnapshot::unsupported(1, 2, "missing host"),
        &Lang("en-US"),
    );
    assert_eq!(unsupported.status, "Quota unavailable: missing host");
    assert_eq!(unsupported.grade, QuotaGrade::Unknown);
    assert_eq!(unsupported.fraction, None);
}

#[cfg(test)]
mod retained_tests {
    use super::*;
    use lurker_core_contract::error::{ErrorCode, Failure};
    #[test]
    fn retained_zero_keeps_geometry_and_is_visibly_stale_in_both_locales() {
        let snapshot = SubscriptionQuotaSnapshot {
            status: SubscriptionQuotaStatus::Failed,
            failure: Some(Failure::new(ErrorCode::Permission, "opaque {reason}", true)),
            retained: true,
            profile_name: Some("user {state}".into()),
            used_bytes: Some(0),
            total_bytes: Some(100),
            usage_percent: Some(0.0),
            ..Default::default()
        };
        let en = project_quota(&snapshot, &Lang("en-US"));
        let zh = project_quota(&snapshot, &Lang("zh-CN"));
        assert_eq!(en.used, "0 B");
        assert_eq!(en.fraction, Some(0.0));
        assert_eq!(en.profile, "user {state}");
        assert_eq!(
            en.status,
            "Last observed quota retained · Quota read failed: opaque {reason}"
        );
        assert_eq!(
            zh.status,
            "保留上次实际配额（已失效）· 配额读取失败：opaque {reason}"
        );
        assert_eq!(en.grade, QuotaGrade::Danger);
    }
}
