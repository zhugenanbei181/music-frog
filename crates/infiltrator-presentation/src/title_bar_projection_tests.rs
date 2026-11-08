use super::*;
use infiltrator_contract::window_chrome::{CHROME_DRAG_STRIP_HEIGHT_PX, TitleBarControlsPlacement};

#[test]
fn traffic_capsule_projection_handles_zeros_and_activity() {
    let idle = project_title_bar_traffic_capsule(0.0, 0.0, false);
    assert_eq!(idle.upload_text, "0 B/s");
    assert_eq!(idle.download_text, "0 B/s");
    assert_eq!(idle.compact_text, "↑ 0 B/s  ↓ 0 B/s");
    assert!(!idle.is_active);

    let active = project_title_bar_traffic_capsule(15_360.0, 1_048_576.0, true);
    assert_eq!(active.upload_text, "15.4 KB/s");
    assert_eq!(active.download_text, "1.0 MB/s");
    assert_eq!(active.compact_text, "↑ 15.4 KB/s  ↓ 1.0 MB/s");
    assert!(active.is_active);
}

#[test]
fn traffic_capsule_projection_clamps_negative_and_nan() {
    let degraded = project_title_bar_traffic_capsule(-100.0, f64::NAN, false);
    assert_eq!(degraded.upload_text, "0 B/s");
    assert_eq!(degraded.download_text, "0 B/s");
    assert_eq!(degraded.upload_bytes_per_sec, 0);
    assert_eq!(degraded.download_bytes_per_sec, 0);
    assert_eq!(degraded.compact_text, "↑ 0 B/s  ↓ 0 B/s");
}

#[test]
fn desktop_title_bar_projection_aligns_macos_and_windows() {
    let mac = project_desktop_title_bar(
        "Ice",
        0.0,
        0.0,
        false,
        DesktopPlatformKind::MacOs,
        SnapLayoutSupport::Unsupported {
            reason: "non-windows-platform".to_string(),
        },
    );
    assert_eq!(mac.controls_placement, TitleBarControlsPlacement::Left);
    assert_eq!(mac.insets.left_px, 72);
    assert_eq!(mac.insets.height_px, CHROME_DRAG_STRIP_HEIGHT_PX as u16);
    assert!(!mac.snap_layout.is_supported());

    let win = project_desktop_title_bar(
        "Ice",
        1024.0,
        2048.0,
        true,
        DesktopPlatformKind::Windows,
        SnapLayoutSupport::Supported,
    );
    assert_eq!(win.controls_placement, TitleBarControlsPlacement::Right);
    assert_eq!(win.insets.right_px, 138);
    assert!(win.snap_layout.is_supported());
    assert!(win.traffic_capsule.is_active);
}
