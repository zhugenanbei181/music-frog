//! Virtual-scrolling adapter core: shared fixed/variable geometry projection and the
//! zero-allocation entity recycling pool.

use zhuge_ui_core::virtual_list::{self, VariableExtentIndex, VirtualListConfig};

// ===========================================================================
// 1. Fixed-Height Virtual Window Math
// ===========================================================================

/// The half-open visible window `[start, end)` of item indices.
pub fn visible_window(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
) -> (usize, usize) {
    if item_count == 0 || viewport_h <= 0.0 {
        return (0, 0);
    }
    let window = virtual_list::VirtualWindow::visible_for_sticky_rows(
        item_count,
        scroll_offset,
        viewport_h,
        row_h,
    );
    (window.start, window.end)
}

/// Compute a virtual window with an overscan buffer for smooth scrolling.
///
/// The window is the shared owner's geometry: `start`/`end` are the
/// materialized row indices and `top`/`bottom` are the spacer extents that
/// keep the scrollable content the full length of the dataset.
pub fn visible_window_with_overscan(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
    overscan: usize,
) -> virtual_list::VirtualWindow {
    // An unmeasured viewport mounts nothing: the shared geometry falls back to a
    // one-row safe extent, but a surface that has not been measured yet must not
    // show a row it cannot place.
    if item_count == 0 || viewport_h <= 0.0 {
        return virtual_list::VirtualWindow::default();
    }
    VirtualListConfig::new(item_count, row_h, viewport_h)
        .with_scroll_offset(scroll_offset)
        .with_overscan(overscan)
        .compute_viewport()
}

/// Clamp a raw scroll offset to valid bounds for a virtual list.
pub fn clamp_scroll_offset(
    scroll_offset: f32,
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
) -> f32 {
    if item_count == 0 || viewport_h <= 0.0 {
        return 0.0;
    }
    VirtualListConfig::new(item_count, row_h, viewport_h)
        .with_scroll_offset(scroll_offset)
        .clamped_scroll_offset()
}

// ===========================================================================
// 2. Variable Extent Index
// ===========================================================================

// Prefix sums, extent updates, resize outcomes, and variable viewport geometry are
// owned by `zhuge-ui-core`; this crate keeps only the Bevy scroll adapter below.

// ===========================================================================
// 3. Slot Recycling
// ===========================================================================

// The recycle rule — which pooled slot shows which row, when a slot rebinds,
// and which generation rejects a stale receipt — is owned by
// `zhuge-ui-kit-bevy::control` (`sync_virtual_slots`, `VirtualScrollSlot`).
// This adapter keeps only the Bevy-side plumbing a page needs around it.

/// Scroll anchor bookmark remembering relative viewport position across route navigations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchorBookmark {
    pub item_index: usize,
    pub offset_in_item_px: f32,
    pub original_scroll_offset_px: f32,
}

impl ScrollAnchorBookmark {
    /// Create an anchor bookmark for the current scroll position using a variable extent index.
    pub fn create(scroll_offset_px: f32, index_engine: &VariableExtentIndex) -> Self {
        let item_index = index_engine.item_at_offset(scroll_offset_px).unwrap_or(0);
        let item_top = index_engine.item_offset(item_index).unwrap_or(0.0);
        let offset_in_item_px = (scroll_offset_px - item_top).max(0.0);

        Self {
            item_index,
            offset_in_item_px,
            original_scroll_offset_px: scroll_offset_px,
        }
    }

    /// Restore scroll offset from this bookmark, adapting to any intervening row height mutations.
    pub fn restore_scroll_offset(&self, index_engine: &VariableExtentIndex) -> f32 {
        if index_engine.item_count() == 0 {
            return 0.0;
        }
        let clamped_idx = self.item_index.min(index_engine.item_count() - 1);
        let item_top = index_engine.item_offset(clamped_idx).unwrap_or(0.0);
        let item_h = index_engine.item_extent(clamped_idx).unwrap_or(0.0);
        let intra = self.offset_in_item_px.min(item_h);
        item_top + intra
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zhuge_ui_core::virtual_list::ExtentUpdate;

    #[test]
    fn test_scroll_anchor_bookmark_restoration() {
        let mut index = VariableExtentIndex::new(20, 50.0);
        assert_eq!(index.total_extent(), 1000.0);

        // Scroll offset 225.0 -> item 4 (top=200, intra=25)
        let bookmark = ScrollAnchorBookmark::create(225.0, &index);
        assert_eq!(bookmark.item_index, 4);
        assert_eq!(bookmark.offset_in_item_px, 25.0);

        // Perfect restoration without mutations
        assert_eq!(bookmark.restore_scroll_offset(&index), 225.0);

        // Mutate preceding row height: item 1 grows from 50 to 100 (+50px)
        assert_eq!(
            index.update_extent(1, 100.0),
            ExtentUpdate::Updated {
                index: 1,
                previous_extent: 50.0,
                extent: 100.0,
            }
        );
        // Restored offset should shift by +50px -> 275.0
        assert_eq!(bookmark.restore_scroll_offset(&index), 275.0);
    }
}
