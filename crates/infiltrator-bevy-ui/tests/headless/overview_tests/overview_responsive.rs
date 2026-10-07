//! Behavior cases for overview responsive.
//! test-intent: behavior

#[test]
fn test_overview_responsive_four_tier_viewport_parity() {
    use infiltrator_contract::responsive_viewport::{ResponsiveViewportSnapshot, ViewportTier};

    let mobile = ResponsiveViewportSnapshot::from_dimensions(390.0, 844.0);
    assert_eq!(mobile.tier, ViewportTier::Compact);
    assert_eq!(mobile.card_columns, 1);
    assert_eq!(mobile.metrics_columns, 2);

    let tablet = ResponsiveViewportSnapshot::from_dimensions(768.0, 1024.0);
    assert_eq!(tablet.tier, ViewportTier::Medium);
    assert_eq!(tablet.card_columns, 2);
    assert_eq!(tablet.metrics_columns, 3);

    let desktop = ResponsiveViewportSnapshot::from_dimensions(1180.0, 780.0);
    assert_eq!(desktop.tier, ViewportTier::Expanded);
    assert_eq!(desktop.card_columns, 2);
    assert_eq!(desktop.metrics_columns, 6);

    let wide = ResponsiveViewportSnapshot::from_dimensions(1920.0, 1080.0);
    assert_eq!(wide.tier, ViewportTier::Ultra);
    assert_eq!(wide.card_columns, 3);
    assert_eq!(wide.metrics_columns, 6);
}
