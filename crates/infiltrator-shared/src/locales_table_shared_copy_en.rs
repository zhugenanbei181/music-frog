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
        _ => key.to_string().into(),
    }
}
