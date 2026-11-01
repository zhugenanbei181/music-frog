/// Title bar specification and live traffic capsule projection (FLPARITY-018).
use crate::chart::format_rate_standard;
use infiltrator_contract::window_chrome::{
    DesktopPlatformKind, DesktopTitleBarSpec, SnapLayoutSupport, TitleBarInsets,
    TitleBarTrafficCapsule,
};

/// Project a live traffic rate pair into a desktop title bar speed capsule.
pub fn project_title_bar_traffic_capsule(
    upload_bps: f64,
    download_bps: f64,
    is_active: bool,
) -> TitleBarTrafficCapsule {
    let upload_text = format_rate_standard(upload_bps);
    let download_text = format_rate_standard(download_bps);
    let up_bytes = if upload_bps.is_finite() && upload_bps > 0.0 {
        upload_bps as u64
    } else {
        0
    };
    let down_bytes = if download_bps.is_finite() && download_bps > 0.0 {
        download_bps as u64
    } else {
        0
    };
    TitleBarTrafficCapsule::new(up_bytes, down_bytes, upload_text, download_text, is_active)
}

/// Build a unified title bar spec for the frameless desktop shell (FLPARITY-018).
pub fn project_desktop_title_bar(
    title: impl Into<String>,
    upload_bps: f64,
    download_bps: f64,
    is_active: bool,
    platform: DesktopPlatformKind,
    snap_layout: SnapLayoutSupport,
) -> DesktopTitleBarSpec {
    let traffic_capsule = project_title_bar_traffic_capsule(upload_bps, download_bps, is_active);
    let controls_placement = platform.controls_placement();
    let insets = TitleBarInsets::for_platform(platform);
    DesktopTitleBarSpec {
        title: title.into(),
        show_traffic_capsule: true,
        traffic_capsule,
        controls_placement,
        insets,
        snap_layout,
        double_click_maximizes: true,
    }
}

#[cfg(test)]
#[path = "title_bar_projection_tests.rs"]
mod tests;
