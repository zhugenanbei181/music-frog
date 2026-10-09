//! Shared virtual-list scroll-offset derivation for page-level scroll areas.
//!
//! The Proxies recycler owns a scroll area whose content is *only* the virtual
//! container, so the raw [`ScrollPosition`] is the row offset. The Connections
//! and Logs pages scroll their header/search together with the rows, so the raw
//! offset leads the row offset by that fixed prefix. Bevy's UI layout folds the
//! parent scroll into each descendant's [`UiGlobalTransform`], so the rows
//! container's position relative to its scroll area is exactly
//! `prefix - scroll_offset`; negating it yields the rows' own scrolled amount.
//!
//! When layout has not run yet (headless tests), the transforms are absent and
//! the raw scroll position is the honest fallback.

use bevy::ui::UiGlobalTransform;
use bevy::ui::prelude::{ComputedNode, ScrollPosition};

/// The rows' own scrolled amount in logical pixels for a page-level scroll area
/// whose content precedes the virtual rows with a fixed header/search prefix.
pub(crate) fn virtual_window_offset_px(
    position: &ScrollPosition,
    scroll_global: Option<&UiGlobalTransform>,
    rows_global: Option<&UiGlobalTransform>,
    computed: Option<&ComputedNode>,
) -> f32 {
    let layout_ready = computed.is_some_and(|node| node.size().y > 0.0);
    if layout_ready && let (Some(scroll), Some(rows)) = (scroll_global, rows_global) {
        let scale = computed
            .map(|node| node.inverse_scale_factor())
            .unwrap_or(1.0);
        let relative = (rows.translation.y - scroll.translation.y) * scale;
        return (-relative).max(0.0);
    }
    position.0.y.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec2;

    fn computed(height: f32) -> ComputedNode {
        ComputedNode {
            size: Vec2::new(100.0, height),
            ..Default::default()
        }
    }

    #[test]
    fn subtracts_the_header_prefix_from_a_page_scroll() {
        // The rows container sits `prefix` below the scroll area top; the
        // scroll area's own transform folds in the scroll offset.
        let position = ScrollPosition(Vec2::new(0.0, 300.0));
        let scroll = UiGlobalTransform::from_xy(0.0, 100.0);
        // rows top viewport-relative = prefix - scroll = 200 - 300 = -100.
        let rows = UiGlobalTransform::from_xy(0.0, 0.0);
        let offset = virtual_window_offset_px(
            &position,
            Some(&scroll),
            Some(&rows),
            Some(&computed(720.0)),
        );
        assert!((offset - 100.0).abs() < 0.001, "got {offset}");
    }

    #[test]
    fn stays_at_zero_while_the_header_scrolls_away() {
        let position = ScrollPosition(Vec2::new(0.0, 120.0));
        let scroll = UiGlobalTransform::from_xy(0.0, 0.0);
        // rows still below the viewport top: prefix 200 - scroll 120 = +80.
        let rows = UiGlobalTransform::from_xy(0.0, 80.0);
        let offset = virtual_window_offset_px(
            &position,
            Some(&scroll),
            Some(&rows),
            Some(&computed(720.0)),
        );
        assert_eq!(offset, 0.0);
    }

    #[test]
    fn falls_back_to_the_raw_offset_before_layout() {
        let position = ScrollPosition(Vec2::new(0.0, 512.0));
        let offset = virtual_window_offset_px(&position, None, None, None);
        assert_eq!(offset, 512.0);
        // A zero-height computed node also means layout has not run.
        let offset = virtual_window_offset_px(
            &position,
            Some(&UiGlobalTransform::from_xy(0.0, 0.0)),
            Some(&UiGlobalTransform::from_xy(0.0, 0.0)),
            Some(&computed(0.0)),
        );
        assert_eq!(offset, 512.0);
    }
}
