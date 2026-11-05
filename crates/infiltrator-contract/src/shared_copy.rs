//! Canonical localization keys for user-facing copy shared by every UI.
//!
//! Surfaces resolve these keys through `infiltrator_shared::locales` or the
//! product `.po` catalog. They never own a second copy of the text.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SharedCopyKey {
    AppName,
    AppDisplayName,
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
}

impl SharedCopyKey {
    pub const ALL: [Self; 12] = [
        Self::AppName,
        Self::AppDisplayName,
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
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::AppName => "brand_name",
            Self::AppDisplayName => "app_display_name",
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
        }
    }
}

pub const SHARED_COPY_KEYS: [&str; SharedCopyKey::ALL.len()] = [
    SharedCopyKey::AppName.key(),
    SharedCopyKey::AppDisplayName.key(),
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
    }
}
