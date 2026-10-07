//! One presentation of observed profile traffic and update schedules for peer products.
use crate::byte_format::format_bytes;
use crate::subscription_status_projection::schedule_status;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageGrade {
    Unknown,
    Normal,
    Warning,
    High,
    Exhausted,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficProjection {
    pub upload: String,
    pub download: String,
    pub used: String,
    pub total: String,
    pub fraction: Option<f32>,
    pub grade: UsageGrade,
}
pub fn traffic(
    upload: Option<u64>,
    download: Option<u64>,
    total: Option<u64>,
) -> TrafficProjection {
    let used = upload
        .zip(download)
        .and_then(|(upload, download)| upload.checked_add(download));
    let fraction = used
        .zip(total)
        .filter(|(_, total)| *total > 0)
        .map(|(used, total)| (used as f32 / total as f32).clamp(0.0, 1.0));
    let grade = match fraction {
        None => UsageGrade::Unknown,
        Some(value) if value >= 1.0 => UsageGrade::Exhausted,
        Some(value) if value >= 0.9 => UsageGrade::High,
        Some(value) if value >= 0.8 => UsageGrade::Warning,
        Some(_) => UsageGrade::Normal,
    };
    let display = |value: Option<u64>| value.map(format_bytes).unwrap_or_else(|| "—".into());
    TrafficProjection {
        upload: display(upload),
        download: display(download),
        used: display(used),
        total: display(total),
        fraction,
        grade,
    }
}
pub fn traffic_caption(facts: &TrafficProjection, locale: &str) -> String {
    let percent = facts
        .fraction
        .map(|value| format!(" ({:.1}%)", value * 100.0))
        .unwrap_or_default();
    localize(
        locale,
        "profile_observed_traffic",
        &[
            ("upload", facts.upload.clone()),
            ("download", facts.download.clone()),
            ("used", facts.used.clone()),
            ("total", facts.total.clone()),
            ("percent", percent),
        ],
    )
}
pub fn active_name(value: Option<&str>, locale: &str) -> String {
    value
        .map(str::to_owned)
        .unwrap_or_else(|| Lang(locale).tr("profile_no_active").into_owned())
}
pub fn summary(count: usize, active: Option<&str>, locale: &str) -> String {
    localize(
        locale,
        "profile_list_summary",
        &[
            ("count", count.to_string()),
            ("active", active_name(active, locale)),
        ],
    )
}
pub fn updated(value: &str, locale: &str) -> String {
    localize(locale, "profile_updated_time", &[("time", value.into())])
}
pub fn active_status(active: bool, locale: &str) -> String {
    Lang(locale)
        .tr(if active {
            "profile_status_active"
        } else {
            "profile_status_activate"
        })
        .into_owned()
}
pub fn schedule(
    enabled: bool,
    cron: Option<&str>,
    hours: Option<u32>,
    next: Option<&str>,
    locale: &str,
) -> String {
    let cadence = schedule_status(Some(enabled), cron, hours).render(locale);
    match next.filter(|next| !next.trim().is_empty()) {
        Some(next) => localize(
            locale,
            "profile_schedule_next",
            &[("cadence", cadence), ("next", next.into())],
        ),
        None => cadence,
    }
}
pub fn schedule_overview<'a>(
    values: impl IntoIterator<Item = (bool, Option<&'a str>, Option<u32>)>,
    locale: &str,
) -> String {
    let enabled = values
        .into_iter()
        .filter(|(enabled, _, _)| *enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return Lang(locale).tr("profile_schedule_disabled").into_owned();
    }
    let interval = enabled
        .iter()
        .filter_map(|(_, _, hours)| *hours)
        .filter(|hours| *hours > 0)
        .min();
    let missing = enabled
        .iter()
        .filter(|(_, cron, hours)| {
            hours.is_none() && cron.is_none_or(|cron| cron.trim().is_empty())
        })
        .count();
    let key = if missing > 0 {
        "profile_schedule_missing"
    } else if interval.is_some() {
        "profile_schedule_interval_summary"
    } else {
        "profile_schedule_cron_summary"
    };
    localize(
        locale,
        key,
        &[
            ("count", enabled.len().to_string()),
            ("missing", missing.to_string()),
            (
                "hours",
                interval
                    .map(|hours| hours.to_string())
                    .unwrap_or_else(|| "—".into()),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_quota_overflow_and_real_zero_never_become_fabricated_usage_or_unlimited() {
        let partial = traffic(None, Some(0), None);
        assert_eq!(
            (
                partial.upload.as_str(),
                partial.download.as_str(),
                partial.used.as_str(),
                partial.total.as_str()
            ),
            ("—", "0 B", "—", "—")
        );
        assert_eq!(partial.fraction, None);
        let zero = traffic(Some(0), Some(0), Some(0));
        assert_eq!((zero.used.as_str(), zero.total.as_str()), ("0 B", "0 B"));
        assert_eq!(zero.grade, UsageGrade::Unknown);
        assert_eq!(traffic(Some(u64::MAX), Some(1), Some(u64::MAX)).used, "—");
        assert_eq!(
            traffic(Some(0), Some(90), Some(100)).grade,
            UsageGrade::High
        );
        assert!(traffic_caption(&partial, "en-US").contains("Used: — / Total: —"));
    }
    #[test]
    fn missing_schedule_is_not_cron_and_opaque_names_and_next_times_are_not_reinterpreted() {
        assert_eq!(
            schedule_overview([(true, None, None)], "en-US"),
            "Auto update: 1 enabled; 1 schedules not observed"
        );
        assert_eq!(
            schedule_overview([(true, Some("0 0 * * *"), None)], "en-US"),
            "Auto update: 1 enabled; Cron schedules"
        );
        assert_eq!(
            summary(1, Some("profile {count}/中文🙂"), "en-US"),
            "Profiles · 1 profiles (active: profile {count}/中文🙂)"
        );
        assert!(
            schedule(true, None, Some(2), Some("{cadence}/🙂"), "en-US")
                .ends_with("Next update {cadence}/🙂")
        );
    }
}
