//! Shared-copy catalog for the English product locale.
use infiltrator_contract::surface::SurfaceKind;
use std::borrow::Cow;

pub(super) fn translate_shared_copy_en(key: &str) -> Cow<'static, str> {
    match key {
        "brand_name" => SurfaceKind::IcedDesktop.product_name().into(),
        "app_display_name" => SurfaceKind::IcedDesktop.product_name().into(),
        "about_brand_note" => "The frog musician in the icon is the internal MusicFrog mark; product names use only their own names.".into(),
        "configuration_saving" => "Saving configuration".into(),
        "configuration_saved" => "Saved".into(),
        "configuration_failed" => "Save failed".into(),
        "performance_snapshot" => "Performance snapshot".into(),
        "performance_navigate_to_first_paint" => "Navigate → first paint: {value}".into(),
        "common_close" => "Close".into(),
        "common_retry" => "Retry".into(),
        "dismiss_probe_batch" => "Close".into(),
        "layout_density" => "Layout density".into(),
        "a11y_value_prefix" => "{label}: {value}".into(),
        "capability_not_registered" => "This capability is not registered for the current host".into(),
        "relative_time_just_now" => "Just now".into(),
        "relative_time_minutes_ago" => "{count} minutes ago".into(),
        "relative_time_hours_ago" => "{count} hours ago".into(),
        "relative_time_days_ago" => "{count} days ago".into(),
        "relative_time_months_ago" => "{count} months ago".into(),
        "relative_time_years_ago" => "{count} years ago".into(),
        "relative_time_compact_just_now" => "Just now".into(),
        "relative_time_compact_minutes_ago" => "{count}m".into(),
        "relative_time_compact_hours_ago" => "{count}h".into(),
        "relative_time_compact_days_ago" => "{count}d".into(),
        "relative_time_compact_months_ago" => "{count}mo".into(),
        "relative_time_compact_years_ago" => "{count}y".into(),
        "relative_time_detailed_prefix" => "Updated {time}".into(),
        "profile_source_local" => "Local".into(),
        "profile_source_subscription" => "Remote".into(),
        "profile_source_imported" => "Imported".into(),
        "profile_source_synchronized" => "Synced".into(),
        "profile_source_unobserved" => "Source unknown".into(),
        _ => key.to_string().into(),
    }
}
