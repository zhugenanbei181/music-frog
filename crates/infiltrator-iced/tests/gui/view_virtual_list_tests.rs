use super::*;

#[test]
fn test_virtual_viewport_empty() {
    let cfg = VirtualListConfig::new(0, 32.0, 400.0);
    let vp = cfg.compute_viewport();
    assert_eq!(vp.start_index, 0);
    assert_eq!(vp.end_index, 0);
    assert_eq!(vp.top_spacer_height, 0.0);
    assert_eq!(vp.bottom_spacer_height, 0.0);
    assert_eq!(vp.total_content_height, 0.0);
}

#[test]
fn test_virtual_viewport_start_with_overscan() {
    let cfg = VirtualListConfig::new(1000, 40.0, 400.0).with_overscan(3);
    let vp = cfg.compute_viewport();

    // At scroll_offset = 0: first_visible = 0, visible_count = 11.
    // start_index with overscan = 0, end_index = (0 + 11 + 3).min(1000) = 14
    assert_eq!(vp.start_index, 0);
    assert_eq!(vp.end_index, 14);
    assert_eq!(vp.top_spacer_height, 0.0);
    assert_eq!(vp.bottom_spacer_height, (1000 - 14) as f32 * 40.0);
    assert_eq!(vp.total_content_height, 40000.0);
}

#[test]
fn test_virtual_viewport_scrolled_mid() {
    // Scrolled to 800px: first_visible = 800 / 40 = 20
    // visible_count = ceil(400 / 40) + 1 = 11
    // overscan = 5 -> start = 20 - 5 = 15, end = (20 + 11 + 5) = 36
    let cfg = VirtualListConfig::new(1000, 40.0, 400.0)
        .with_scroll_offset(800.0)
        .with_overscan(5);
    let vp = cfg.compute_viewport();

    assert_eq!(vp.start_index, 15);
    assert_eq!(vp.end_index, 36);
    assert_eq!(vp.top_spacer_height, 15.0 * 40.0);
    assert_eq!(vp.bottom_spacer_height, (1000 - 36) as f32 * 40.0);
}

#[test]
fn test_virtual_viewport_clamped_end() {
    // Scrolled beyond end
    let cfg = VirtualListConfig::new(100, 30.0, 300.0)
        .with_scroll_offset(5000.0)
        .with_overscan(2);
    let vp = cfg.compute_viewport();

    assert_eq!(vp.end_index, 100);
    assert_eq!(vp.bottom_spacer_height, 0.0);
    assert_eq!(vp.total_content_height, 3000.0);
}
