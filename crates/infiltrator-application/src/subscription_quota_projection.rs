//! One native-independent presentation of optional quota observations for both peer products.
use crate::byte_format::format_bytes;
use crate::failure_projection::failure_message;
use infiltrator_contract::subscription_quota::{
    SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaGrade {
    Unknown,
    Observed,
    Warning,
    Danger,
}
pub struct QuotaPresentation {
    pub profile: String,
    pub expiry: String,
    pub used: String,
    pub total: String,
    pub remaining: String,
    pub usage: String,
    pub remaining_percent: String,
    pub reset: String,
    pub status: String,
    pub metrics: String,
    pub grade: QuotaGrade,
    pub fraction: Option<f32>,
}
pub fn project_quota(snapshot: &SubscriptionQuotaSnapshot, locale: &str) -> QuotaPresentation {
    let lang = Lang(locale);
    let amount = |value: Option<u64>| value.map(format_bytes).unwrap_or_else(|| "—".into());
    let percent = |value: Option<f64>| {
        value
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| format!("{value:.1}%"))
            .unwrap_or_else(|| lang.tr("quota_value_unobserved").into_owned())
    };
    let used = amount(snapshot.used_bytes);
    let total = amount(snapshot.total_bytes);
    let usage = percent(snapshot.usage_percent);
    let remaining = amount(snapshot.remaining_bytes);
    let remaining_percent = percent(snapshot.remaining_percent);
    let expiry = snapshot
        .expires_at_label
        .as_ref()
        .map(|date| match snapshot.remaining_days {
            Some(days) => localize(
                locale,
                "quota_expiry_days",
                &[("date", date.clone()), ("days", days.to_string())],
            ),
            None => date.clone(),
        })
        .unwrap_or_else(|| {
            lang.tr("overview_subscription_quota_expiry_unknown")
                .into_owned()
        });
    let reset = snapshot
        .reset_days
        .map(|days| localize(locale, "quota_reset_days", &[("days", days.to_string())]))
        .unwrap_or_else(|| {
            lang.tr("overview_subscription_quota_reset_unknown")
                .into_owned()
        });
    let (key, grade) = match snapshot.status {
        SubscriptionQuotaStatus::Unknown => ("quota_status_unobserved", QuotaGrade::Unknown),
        SubscriptionQuotaStatus::Ready => ("quota_status_observed", QuotaGrade::Observed),
        SubscriptionQuotaStatus::Empty => ("quota_status_empty", QuotaGrade::Unknown),
        SubscriptionQuotaStatus::Warning => {
            ("overview_subscription_quota_warning", QuotaGrade::Warning)
        }
        SubscriptionQuotaStatus::Critical => {
            ("overview_subscription_quota_critical", QuotaGrade::Danger)
        }
        SubscriptionQuotaStatus::Exhausted => {
            ("overview_subscription_quota_exhausted", QuotaGrade::Danger)
        }
        SubscriptionQuotaStatus::Expired => {
            ("overview_subscription_quota_expired", QuotaGrade::Danger)
        }
        SubscriptionQuotaStatus::ExpiringSoon => {
            ("overview_subscription_quota_expiring", QuotaGrade::Warning)
        }
        SubscriptionQuotaStatus::Unsupported => ("quota_status_unsupported", QuotaGrade::Unknown),
        SubscriptionQuotaStatus::Failed => ("quota_status_failed", QuotaGrade::Danger),
    };
    let mut status = if matches!(
        snapshot.status,
        SubscriptionQuotaStatus::Unsupported | SubscriptionQuotaStatus::Failed
    ) {
        localize(
            locale,
            key,
            &[(
                "reason",
                snapshot
                    .failure
                    .as_ref()
                    .map(|failure| failure_message(failure, locale))
                    .unwrap_or_else(|| lang.tr("quota_reason_unreported").into_owned()),
            )],
        )
    } else {
        lang.tr(key).into_owned()
    };
    if snapshot.retained {
        status = localize(locale, "quota_retained_observation", &[("state", status)]);
    }
    QuotaPresentation {
        profile: snapshot.profile_name.clone().unwrap_or_else(|| {
            lang.tr("overview_subscription_quota_no_profile")
                .into_owned()
        }),
        expiry,
        remaining: remaining.clone(),
        remaining_percent: remaining_percent.clone(),
        reset,
        status,
        metrics: localize(
            locale,
            "quota_usage_summary",
            &[
                ("used", used.clone()),
                ("total", total.clone()),
                ("usage", usage.clone()),
                ("remaining", remaining),
                ("remaining_percent", remaining_percent),
            ],
        ),
        used,
        total,
        usage,
        grade,
        fraction: snapshot
            .usage_percent
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| (value / 100.0).clamp(0.0, 1.0) as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_and_reported_zero_keep_distinct_values_and_user_identifiers_are_not_templates() {
        let mut snapshot = SubscriptionQuotaSnapshot::default();
        let unknown = project_quota(&snapshot, "en-US");
        assert_eq!(unknown.used, "—");
        assert_eq!(unknown.total, "—");
        assert_eq!(unknown.fraction, None);
        assert_eq!(unknown.status, "Quota not observed");
        snapshot.status = SubscriptionQuotaStatus::Ready;
        snapshot.profile_name = Some("用户 {usage} {reason}".into());
        snapshot.used_bytes = Some(0);
        snapshot.total_bytes = Some(0);
        snapshot.usage_percent = Some(0.0);
        let zero = project_quota(&snapshot, "en-US");
        assert_eq!(zero.used, "0 B");
        assert_eq!(zero.total, "0 B");
        assert_eq!(zero.usage, "0.0%");
        assert_eq!(zero.fraction, Some(0.0));
        assert_eq!(zero.profile, "用户 {usage} {reason}");
        assert_eq!(zero.status, "Provider metadata observed");
        snapshot.usage_percent = Some(f64::NAN);
        assert_eq!(project_quota(&snapshot, "zh-CN").fraction, None);
    }
    #[test]
    fn failure_and_unsupported_are_localized_without_inventing_success_or_translating_backend_reason()
     {
        let failed = SubscriptionQuotaSnapshot::failed(2, 3, "opaque {reason} cause");
        let en = project_quota(&failed, "en-US");
        let zh = project_quota(&failed, "zh-CN");
        assert_eq!(en.status, "Quota read failed: opaque {reason} cause");
        assert_eq!(zh.status, "配额读取失败：opaque {reason} cause");
        assert_eq!(en.grade, QuotaGrade::Danger);
        let unsupported = project_quota(
            &SubscriptionQuotaSnapshot::unsupported(1, 2, "missing host"),
            "en-US",
        );
        assert_eq!(unsupported.status, "Quota unavailable: missing host");
        assert_eq!(unsupported.grade, QuotaGrade::Unknown);
        assert_eq!(unsupported.fraction, None);
    }
}

#[cfg(test)]
mod retained_tests {
    use super::*;
    use infiltrator_contract::error::{ErrorCode, Failure};
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
        let en = project_quota(&snapshot, "en-US");
        let zh = project_quota(&snapshot, "zh-CN");
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
