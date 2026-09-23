//! DUAL-15-07 Iced-side evidence: the desktop surface has no touch-gesture
//! host. Bevy hosts the real `TouchInput` consumer and the widget recognizer;
//! Iced must state the typed-unsupported boundary instead of faking parity.

use crate::gesture;
use infiltrator_contract::shell_gesture::TouchGestureSupport;

#[test]
fn a_desktop_surface_reports_no_touch_gesture_host() {
    assert_eq!(
        gesture::touch_support(),
        TouchGestureSupport::Unsupported {
            reason: "iced-desktop-surface-has-no-touch-gesture-host",
        }
    );
    assert!(!gesture::touch_support().is_hosted());
    assert_eq!(
        gesture::touch_support().unsupported_reason(),
        Some("iced-desktop-surface-has-no-touch-gesture-host")
    );
}
