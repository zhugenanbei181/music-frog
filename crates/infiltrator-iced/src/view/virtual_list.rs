//! High-performance Virtual Viewport Scrolling Engine for massive lists.
//!
//! Provides O(1) viewport index slicing and top/bottom spacer height calculation,
//! preventing UI thread stalling and bounded memory footprint even with 50,000+
//! rules or connection logs.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualListConfig {
    pub total_items: usize,
    pub item_height: f32,
    pub viewport_height: f32,
    pub scroll_offset: f32,
    pub overscan: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualViewport {
    pub start_index: usize,
    pub end_index: usize,
    pub top_spacer_height: f32,
    pub bottom_spacer_height: f32,
    pub total_content_height: f32,
}

impl VirtualListConfig {
    pub fn new(total_items: usize, item_height: f32, viewport_height: f32) -> Self {
        Self {
            total_items,
            item_height: item_height.max(1.0),
            viewport_height: viewport_height.max(1.0),
            scroll_offset: 0.0,
            overscan: 5,
        }
    }

    pub fn with_scroll_offset(mut self, offset: f32) -> Self {
        self.scroll_offset = offset.max(0.0);
        self
    }

    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub fn compute_viewport(&self) -> VirtualViewport {
        if self.total_items == 0 {
            return VirtualViewport {
                start_index: 0,
                end_index: 0,
                top_spacer_height: 0.0,
                bottom_spacer_height: 0.0,
                total_content_height: 0.0,
            };
        }

        let total_content_height = self.total_items as f32 * self.item_height;
        let clamped_scroll = self.scroll_offset.clamp(0.0, total_content_height);
        let first_visible = (clamped_scroll / self.item_height).floor() as usize;
        let visible_count = (self.viewport_height / self.item_height).ceil() as usize + 1;

        let start_index = first_visible.saturating_sub(self.overscan).min(self.total_items);
        let end_index = (first_visible
            .saturating_add(visible_count)
            .saturating_add(self.overscan))
        .min(self.total_items);

        let top_spacer_height = start_index as f32 * self.item_height;
        let bottom_spacer_height =
            (self.total_items.saturating_sub(end_index)) as f32 * self.item_height;

        VirtualViewport {
            start_index,
            end_index,
            top_spacer_height,
            bottom_spacer_height,
            total_content_height,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
