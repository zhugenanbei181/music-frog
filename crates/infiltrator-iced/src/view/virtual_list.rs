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
#[path = "../../tests/gui/view_virtual_list_tests.rs"]
mod tests;
