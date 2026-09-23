use super::*;

#[test]
fn the_iced_surface_declares_the_typed_unsupported_boundary() {
    let support = touch_support();
    assert!(!support.is_hosted());
    assert!(!support.multi_touch());
    assert_eq!(
        support.unsupported_reason(),
        Some("iced-desktop-surface-has-no-touch-gesture-host")
    );
}
