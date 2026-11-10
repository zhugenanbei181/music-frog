//! Canonical localization keys for user-facing copy shared by every UI.
//!
//! Surfaces resolve these keys through `infiltrator_shared::locales` or the
//! product `.po` catalog. They never own a second copy of the text.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SharedCopyKey {
    AppName,
    AppDisplayName,
    AboutBrandNote,
    ConfigurationSaving,
    ConfigurationSaved,
    ConfigurationFailed,
    PerformanceSnapshot,
    PerformanceNavigateToFirstPaint,
    CommonClose,
    CommonRetry,
    DismissProbeBatch,
    LayoutDensity,
    A11yValuePrefix,
    CapabilityNotRegistered,
    RelativeTimeJustNow,
    RelativeTimeMinutesAgo,
    RelativeTimeHoursAgo,
    RelativeTimeDaysAgo,
    RelativeTimeMonthsAgo,
    RelativeTimeYearsAgo,
    RelativeTimeCompactJustNow,
    RelativeTimeCompactMinutesAgo,
    RelativeTimeCompactHoursAgo,
    RelativeTimeCompactDaysAgo,
    RelativeTimeCompactMonthsAgo,
    RelativeTimeCompactYearsAgo,
    RelativeTimeDetailedPrefix,
    ProfileSourceLocal,
    ProfileSourceSubscription,
    ProfileSourceImported,
    ProfileSourceSynchronized,
    ProfileSourceUnobserved,
}

impl SharedCopyKey {
    pub const ALL: [Self; 32] = [
        Self::AppName,
        Self::AppDisplayName,
        Self::AboutBrandNote,
        Self::ConfigurationSaving,
        Self::ConfigurationSaved,
        Self::ConfigurationFailed,
        Self::PerformanceSnapshot,
        Self::PerformanceNavigateToFirstPaint,
        Self::CommonClose,
        Self::CommonRetry,
        Self::DismissProbeBatch,
        Self::LayoutDensity,
        Self::A11yValuePrefix,
        Self::CapabilityNotRegistered,
        Self::RelativeTimeJustNow,
        Self::RelativeTimeMinutesAgo,
        Self::RelativeTimeHoursAgo,
        Self::RelativeTimeDaysAgo,
        Self::RelativeTimeMonthsAgo,
        Self::RelativeTimeYearsAgo,
        Self::RelativeTimeCompactJustNow,
        Self::RelativeTimeCompactMinutesAgo,
        Self::RelativeTimeCompactHoursAgo,
        Self::RelativeTimeCompactDaysAgo,
        Self::RelativeTimeCompactMonthsAgo,
        Self::RelativeTimeCompactYearsAgo,
        Self::RelativeTimeDetailedPrefix,
        Self::ProfileSourceLocal,
        Self::ProfileSourceSubscription,
        Self::ProfileSourceImported,
        Self::ProfileSourceSynchronized,
        Self::ProfileSourceUnobserved,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::AppName => "brand_name",
            Self::AppDisplayName => "app_display_name",
            Self::AboutBrandNote => "about_brand_note",
            Self::ConfigurationSaving => "configuration_saving",
            Self::ConfigurationSaved => "configuration_saved",
            Self::ConfigurationFailed => "configuration_failed",
            Self::PerformanceSnapshot => "performance_snapshot",
            Self::PerformanceNavigateToFirstPaint => "performance_navigate_to_first_paint",
            Self::CommonClose => "common_close",
            Self::CommonRetry => "common_retry",
            Self::DismissProbeBatch => "dismiss_probe_batch",
            Self::LayoutDensity => "layout_density",
            Self::A11yValuePrefix => "a11y_value_prefix",
            Self::CapabilityNotRegistered => "capability_not_registered",
            Self::RelativeTimeJustNow => "relative_time_just_now",
            Self::RelativeTimeMinutesAgo => "relative_time_minutes_ago",
            Self::RelativeTimeHoursAgo => "relative_time_hours_ago",
            Self::RelativeTimeDaysAgo => "relative_time_days_ago",
            Self::RelativeTimeMonthsAgo => "relative_time_months_ago",
            Self::RelativeTimeYearsAgo => "relative_time_years_ago",
            Self::RelativeTimeCompactJustNow => "relative_time_compact_just_now",
            Self::RelativeTimeCompactMinutesAgo => "relative_time_compact_minutes_ago",
            Self::RelativeTimeCompactHoursAgo => "relative_time_compact_hours_ago",
            Self::RelativeTimeCompactDaysAgo => "relative_time_compact_days_ago",
            Self::RelativeTimeCompactMonthsAgo => "relative_time_compact_months_ago",
            Self::RelativeTimeCompactYearsAgo => "relative_time_compact_years_ago",
            Self::RelativeTimeDetailedPrefix => "relative_time_detailed_prefix",
            Self::ProfileSourceLocal => "profile_source_local",
            Self::ProfileSourceSubscription => "profile_source_subscription",
            Self::ProfileSourceImported => "profile_source_imported",
            Self::ProfileSourceSynchronized => "profile_source_synchronized",
            Self::ProfileSourceUnobserved => "profile_source_unobserved",
        }
    }
}

pub const SHARED_COPY_KEYS: [&str; SharedCopyKey::ALL.len()] = [
    SharedCopyKey::AppName.key(),
    SharedCopyKey::AppDisplayName.key(),
    SharedCopyKey::AboutBrandNote.key(),
    SharedCopyKey::ConfigurationSaving.key(),
    SharedCopyKey::ConfigurationSaved.key(),
    SharedCopyKey::ConfigurationFailed.key(),
    SharedCopyKey::PerformanceSnapshot.key(),
    SharedCopyKey::PerformanceNavigateToFirstPaint.key(),
    SharedCopyKey::CommonClose.key(),
    SharedCopyKey::CommonRetry.key(),
    SharedCopyKey::DismissProbeBatch.key(),
    SharedCopyKey::LayoutDensity.key(),
    SharedCopyKey::A11yValuePrefix.key(),
    SharedCopyKey::CapabilityNotRegistered.key(),
    SharedCopyKey::RelativeTimeJustNow.key(),
    SharedCopyKey::RelativeTimeMinutesAgo.key(),
    SharedCopyKey::RelativeTimeHoursAgo.key(),
    SharedCopyKey::RelativeTimeDaysAgo.key(),
    SharedCopyKey::RelativeTimeMonthsAgo.key(),
    SharedCopyKey::RelativeTimeYearsAgo.key(),
    SharedCopyKey::RelativeTimeCompactJustNow.key(),
    SharedCopyKey::RelativeTimeCompactMinutesAgo.key(),
    SharedCopyKey::RelativeTimeCompactHoursAgo.key(),
    SharedCopyKey::RelativeTimeCompactDaysAgo.key(),
    SharedCopyKey::RelativeTimeCompactMonthsAgo.key(),
    SharedCopyKey::RelativeTimeCompactYearsAgo.key(),
    SharedCopyKey::RelativeTimeDetailedPrefix.key(),
    SharedCopyKey::ProfileSourceLocal.key(),
    SharedCopyKey::ProfileSourceSubscription.key(),
    SharedCopyKey::ProfileSourceImported.key(),
    SharedCopyKey::ProfileSourceSynchronized.key(),
    SharedCopyKey::ProfileSourceUnobserved.key(),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn shared_copy_keys_are_unique_and_stable() {
        let keys: BTreeSet<&str> = SharedCopyKey::ALL.iter().map(|key| key.key()).collect();
        assert_eq!(keys.len(), SharedCopyKey::ALL.len());
        assert_eq!(SHARED_COPY_KEYS.len(), SharedCopyKey::ALL.len());
        assert!(SHARED_COPY_KEYS.contains(&"brand_name"));
        assert!(SHARED_COPY_KEYS.contains(&"a11y_value_prefix"));
        assert!(SHARED_COPY_KEYS.contains(&"capability_not_registered"));
        assert!(SHARED_COPY_KEYS.contains(&"relative_time_just_now"));
        assert!(SHARED_COPY_KEYS.contains(&"relative_time_compact_just_now"));
        assert!(SHARED_COPY_KEYS.contains(&"relative_time_detailed_prefix"));
        assert!(SHARED_COPY_KEYS.contains(&"profile_source_subscription"));
        assert!(SHARED_COPY_KEYS.contains(&"profile_source_unobserved"));
    }
}
