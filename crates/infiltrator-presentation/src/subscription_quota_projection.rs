//! One native-independent presentation of optional quota observations for both peer products.
use crate::failure_projection::failure_message;
use infiltrator_contract::subscription_quota::{
    SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
};
use infiltrator_shared::byte_units::format_bytes;
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
#[path = "subscription_quota_projection_tests.rs"]
mod tests;
