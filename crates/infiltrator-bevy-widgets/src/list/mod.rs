//! High-Performance Virtual List, Viewport Culling, Dynamic Height Indexing,
//! Entity Recycling Pool, and Smooth Inertial Scrolling Engine.
//!
//! **Pure core**:
//! - [`visible_window`] & [`visible_window_with_overscan`]: O(1) fixed-height windowing;
//! - [`DynamicHeightIndex`]: O(log N) binary search prefix-sum indexing and dynamic height probing;
//! - [`InertialScroller`]: Physics-driven exponential velocity decay and spring damping smooth scrolling;
//! - [`VirtualEntityPool`]: Zero-allocation slot recycling preventing entity churn over 10,000+ items;
//! - [`VirtualListState`]: Unified virtual list state machine for massive collections (1,000,000+ items).
//!
//! **Scene adapter**: [`list_scene`] and [`list_row_scene`] build clipped token-spaced columns
//! over nav-vocabulary rows with in-place component restamping on selection and theme flips.

pub mod scroll_core;
use scroll_core::*;

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Changed, With};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, Overflow, UiRect, Val, percent,
    px,
};
use bevy::ui::widget::Text;

use crate::nav::{NavActive, NavItem, NavLabel, nav_fill};
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

// ===========================================================================
// 5. Virtual List State Machine
// ===========================================================================

/// Row height calculation mode.
#[derive(Clone, Debug, PartialEq)]
pub enum RowHeightMode {
    /// Uniform fixed row height.
    Fixed(f32),
    /// Non-uniform dynamic row heights backed by binary search prefix sums.
    Dynamic(DynamicHeightIndex),
}

/// High-performance Virtual List state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualListState {
    item_count: usize,
    height_mode: RowHeightMode,
    viewport_height_px: f32,
    scroll_offset_px: f32,
    overscan: usize,
    selected_index: Option<usize>,
    scroller: InertialScroller,
}

impl Default for VirtualListState {
    fn default() -> Self {
        Self {
            item_count: 0,
            height_mode: RowHeightMode::Fixed(36.0),
            viewport_height_px: 0.0,
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
            scroller: InertialScroller::default(),
        }
    }
}

impl VirtualListState {
    /// Create a new virtual list state with uniform fixed row height.
    pub fn new(item_count: usize, row_height_px: f32, viewport_height_px: f32) -> Self {
        let mut state = Self {
            item_count,
            height_mode: RowHeightMode::Fixed(row_height_px.max(1.0)),
            viewport_height_px: viewport_height_px.max(0.0),
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
            scroller: InertialScroller::default(),
        };
        state.clamp_scroll();
        state
    }

    /// Create a new virtual list state with dynamic row height indexing.
    pub fn new_dynamic(item_count: usize, default_height_px: f32, viewport_height_px: f32) -> Self {
        let mut state = Self {
            item_count,
            height_mode: RowHeightMode::Dynamic(DynamicHeightIndex::new(
                item_count,
                default_height_px,
            )),
            viewport_height_px: viewport_height_px.max(0.0),
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
            scroller: InertialScroller::default(),
        };
        state.clamp_scroll();
        state
    }

    /// Set overscan buffer count.
    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Set initial scroll offset.
    pub fn with_scroll_offset(mut self, offset_px: f32) -> Self {
        self.scroll_offset_px = offset_px;
        self.clamp_scroll();
        self
    }

    /// Set initial selected index.
    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected_index = selected.filter(|&idx| idx < self.item_count);
        self
    }

    // --- Getters ---

    /// Number of total items in the virtual dataset.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Row height in pixels (fixed mode baseline).
    pub fn row_height_px(&self) -> f32 {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => *h,
            RowHeightMode::Dynamic(idx) => idx.default_height_px(),
        }
    }

    /// Height mode descriptor.
    pub fn height_mode(&self) -> &RowHeightMode {
        &self.height_mode
    }

    /// Mutable reference to height mode for dynamic updates.
    pub fn height_mode_mut(&mut self) -> &mut RowHeightMode {
        &mut self.height_mode
    }

    /// Visible height of the viewport container in pixels.
    pub fn viewport_height_px(&self) -> f32 {
        self.viewport_height_px
    }

    /// Current scroll offset in pixels from top.
    pub fn scroll_offset_px(&self) -> f32 {
        self.scroll_offset_px
    }

    /// Number of overscan buffer items.
    pub fn overscan(&self) -> usize {
        self.overscan
    }

    /// Current selected item index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Reference to internal inertial scroller.
    pub fn scroller(&self) -> &InertialScroller {
        &self.scroller
    }

    /// Mutable reference to internal inertial scroller.
    pub fn scroller_mut(&mut self) -> &mut InertialScroller {
        &mut self.scroller
    }

    /// Total height of all items combined in pixels. O(1).
    pub fn total_height_px(&self) -> f32 {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => self.item_count as f32 * *h,
            RowHeightMode::Dynamic(idx) => idx.total_height_px(),
        }
    }

    /// Maximum valid scroll offset in pixels. O(1).
    pub fn max_scroll_offset(&self) -> f32 {
        (self.total_height_px() - self.viewport_height_px).max(0.0)
    }

    /// Whether the list is scrolled to top boundary.
    pub fn is_at_top(&self) -> bool {
        self.scroll_offset_px <= 0.0
    }

    /// Whether the list is scrolled to bottom boundary.
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset_px >= self.max_scroll_offset()
    }

    /// Scroll progress fraction from 0.0 to 1.0.
    pub fn scroll_progress(&self) -> f32 {
        let max = self.max_scroll_offset();
        if max <= 0.0 {
            0.0
        } else {
            (self.scroll_offset_px / max).clamp(0.0, 1.0)
        }
    }

    /// The visible range `[start, end)` directly visible without overscan.
    pub fn visible_range(&self) -> (usize, usize) {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => visible_window(
                self.item_count,
                self.viewport_height_px,
                *h,
                self.scroll_offset_px,
            ),
            RowHeightMode::Dynamic(idx) => {
                let win = idx.compute_window(self.viewport_height_px, self.scroll_offset_px, 0);
                (win.start, win.end)
            }
        }
    }

    /// The resolved virtual window including overscan buffer and spacer heights.
    pub fn window(&self) -> VirtualWindow {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => visible_window_with_overscan(
                self.item_count,
                self.viewport_height_px,
                *h,
                self.scroll_offset_px,
                self.overscan,
            ),
            RowHeightMode::Dynamic(idx) => idx.compute_window(
                self.viewport_height_px,
                self.scroll_offset_px,
                self.overscan,
            ),
        }
    }

    /// Scroll to place the item centered in the viewport.
    pub fn scroll_to_index_centered(&mut self, index: usize) -> bool {
        if index >= self.item_count {
            return false;
        }
        let item_top = self.item_offset_y(index).unwrap_or(0.0);
        let item_h = self.item_height(index).unwrap_or(36.0);
        let item_center = item_top + item_h * 0.5;
        let target_offset = item_center - (self.viewport_height_px * 0.5);
        self.set_scroll_offset(target_offset)
    }

    /// Item vertical bounds (top_y, bottom_y) in content space.
    pub fn item_rect_y(&self, index: usize) -> Option<(f32, f32)> {
        let top = self.item_offset_y(index)?;
        let h = self.item_height(index)?;
        Some((top, top + h))
    }

    /// Whether the item at index intersects the visible viewport.
    pub fn is_item_visible(&self, index: usize) -> bool {
        let (start, end) = self.visible_range();
        index >= start && index < end
    }

    /// Whether the item at index is inside the mounted overscan window.
    pub fn is_item_mounted(&self, index: usize) -> bool {
        let win = self.window();
        index >= win.start && index < win.end
    }

    /// Find item index at given vertical offset in content space.
    pub fn item_at_offset(&self, offset_y_px: f32) -> Option<usize> {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => {
                if offset_y_px < 0.0 || self.item_count == 0 || *h <= 0.0 {
                    return None;
                }
                let index = (offset_y_px / *h).floor() as usize;
                if index < self.item_count {
                    Some(index)
                } else {
                    None
                }
            }
            RowHeightMode::Dynamic(idx) => idx.find_index_at_offset(offset_y_px),
        }
    }

    /// Top coordinate (px) in content space of the item at `index`.
    pub fn item_offset_y(&self, index: usize) -> Option<f32> {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => {
                if index < self.item_count {
                    Some(index as f32 * *h)
                } else {
                    None
                }
            }
            RowHeightMode::Dynamic(idx) => idx.item_offset_y(index),
        }
    }

    /// Height of item at `index`.
    pub fn item_height(&self, index: usize) -> Option<f32> {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => {
                if index < self.item_count {
                    Some(*h)
                } else {
                    None
                }
            }
            RowHeightMode::Dynamic(idx) => idx.item_height(index),
        }
    }

    // --- State Mutations ---

    fn clamp_scroll(&mut self) {
        let max_scroll = self.max_scroll_offset();
        self.scroll_offset_px = self.scroll_offset_px.clamp(0.0, max_scroll);
    }

    /// Update total item count and re-clamp scroll and selection.
    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
        if let RowHeightMode::Dynamic(idx) = &mut self.height_mode {
            idx.resize(count);
        }
        self.clamp_scroll();
        if let Some(sel) = self.selected_index
            && sel >= count
        {
            self.selected_index = count.checked_sub(1);
        }
    }

    /// Update viewport height and re-clamp scroll offset.
    pub fn set_viewport_height(&mut self, height_px: f32) {
        self.viewport_height_px = height_px.max(0.0);
        self.clamp_scroll();
    }

    /// Update row height (for fixed mode).
    pub fn set_row_height(&mut self, height_px: f32) {
        let h = height_px.max(1.0);
        match &mut self.height_mode {
            RowHeightMode::Fixed(cur) => *cur = h,
            RowHeightMode::Dynamic(idx) => *idx = DynamicHeightIndex::new(self.item_count, h),
        }
        self.clamp_scroll();
    }

    /// Update a measured dynamic row height.
    pub fn update_measured_row_height(&mut self, index: usize, height_px: f32) -> bool {
        if let RowHeightMode::Dynamic(idx) = &mut self.height_mode {
            let changed = idx.update_measured_height(index, height_px);
            if changed {
                self.clamp_scroll();
            }
            changed
        } else {
            false
        }
    }

    /// Set overscan count.
    pub fn set_overscan(&mut self, overscan: usize) {
        self.overscan = overscan;
    }

    /// Set scroll offset directly. Returns true if offset changed.
    pub fn set_scroll_offset(&mut self, offset_px: f32) -> bool {
        let old = self.scroll_offset_px;
        self.scroll_offset_px = offset_px;
        self.clamp_scroll();
        (self.scroll_offset_px - old).abs() > f32::EPSILON
    }

    /// Scroll by a delta in pixels.
    pub fn scroll_by(&mut self, delta_px: f32) -> bool {
        self.set_scroll_offset(self.scroll_offset_px + delta_px)
    }

    /// Scroll to make the given item visible in the viewport.
    pub fn scroll_to_index(&mut self, index: usize) -> bool {
        if index >= self.item_count {
            return false;
        }
        let item_top = self.item_offset_y(index).unwrap_or(0.0);
        let item_h = self.item_height(index).unwrap_or(36.0);
        let item_bottom = item_top + item_h;
        let view_top = self.scroll_offset_px;
        let view_bottom = view_top + self.viewport_height_px;

        if item_top < view_top {
            self.set_scroll_offset(item_top)
        } else if item_bottom > view_bottom {
            self.set_scroll_offset(item_bottom - self.viewport_height_px)
        } else {
            false
        }
    }

    /// Smooth scroll to index using spring animation.
    pub fn smooth_scroll_to_index(&mut self, index: usize) {
        if index >= self.item_count {
            return;
        }
        let item_top = self.item_offset_y(index).unwrap_or(0.0);
        let item_h = self.item_height(index).unwrap_or(36.0);
        let item_center = item_top + item_h * 0.5;
        let target =
            (item_center - self.viewport_height_px * 0.5).clamp(0.0, self.max_scroll_offset());
        self.scroller.scroll_to_smooth(target);
    }

    /// Advance inertial scroll physics simulation frame.
    pub fn tick_physics(&mut self, dt_secs: f32) -> bool {
        let max_offset = self.max_scroll_offset();
        let (new_offset, changed) = self
            .scroller
            .tick(dt_secs, self.scroll_offset_px, max_offset);
        if changed {
            self.scroll_offset_px = new_offset;
        }
        changed
    }

    /// Scroll to top.
    pub fn scroll_to_top(&mut self) -> bool {
        self.set_scroll_offset(0.0)
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) -> bool {
        self.set_scroll_offset(self.max_scroll_offset())
    }

    /// Page down.
    pub fn page_down(&mut self) -> bool {
        self.scroll_by(self.viewport_height_px)
    }

    /// Page up.
    pub fn page_up(&mut self) -> bool {
        self.scroll_by(-self.viewport_height_px)
    }

    /// Select an index.
    pub fn select(&mut self, index: Option<usize>) -> bool {
        if let Some(idx) = index
            && idx >= self.item_count
        {
            return false;
        }
        if self.selected_index != index {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    /// Select next item.
    pub fn select_next(&mut self) -> bool {
        if self.item_count == 0 {
            return false;
        }
        let next = match self.selected_index {
            Some(curr) => (curr + 1).min(self.item_count - 1),
            None => 0,
        };
        self.select(Some(next))
    }

    /// Select previous item.
    pub fn select_previous(&mut self) -> bool {
        if self.item_count == 0 {
            return false;
        }
        let prev = match self.selected_index {
            Some(curr) => curr.saturating_sub(1),
            None => 0,
        };
        self.select(Some(prev))
    }

    /// Select an item and scroll it into view.
    pub fn select_and_reveal(&mut self, index: usize) -> bool {
        let sel_changed = self.select(Some(index));
        let scroll_changed = self.scroll_to_index(index);
        sel_changed || scroll_changed
    }
}

// ===========================================================================
// 6. Bevy ECS Components & Messages
// ===========================================================================

/// Component carrying virtual list state.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct VirtualList(pub VirtualListState);

/// Message requesting a scroll delta on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct VirtualListScroll(pub f32);

/// Message requesting a fling gesture with initial velocity on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct VirtualListFling(pub f32);

/// Message requesting a selection change on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualListSelect(pub Option<usize>);

/// Drive virtual lists from incoming messages and step physics.
pub fn advance_virtual_lists(
    mut scrolls: MessageReader<VirtualListScroll>,
    mut flings: MessageReader<VirtualListFling>,
    mut selects: MessageReader<VirtualListSelect>,
    mut lists: Query<&mut VirtualList>,
) {
    for event in scrolls.read() {
        for mut list in &mut lists {
            list.0.scroll_by(event.0);
        }
    }
    for event in flings.read() {
        for mut list in &mut lists {
            list.0.scroller_mut().fling(event.0);
        }
    }
    for event in selects.read() {
        for mut list in &mut lists {
            list.0.select(event.0);
        }
    }
}

/// Marker on the list column root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct List;

/// The list-owned selection: the index of the selected row, or `None`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListSelection(pub Option<usize>);

/// One list row: the nav-item pill vocabulary.
pub fn list_row_scene(label: String, selected: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let fill = nav_fill(selected, palette);
    Box::new(bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ fill })
        NavItem
        NavActive({ selected })
        Children [
            ( Text(label) TextRole(Role::Body) NavLabel ),
        ]
    })
}

/// The list: a clipped, token-spaced column over caller-composed rows.
pub fn list_scene(
    rows: Vec<Box<dyn Scene>>,
    selected: Option<usize>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
            overflow: Overflow::scroll_y(),
        }
        BackgroundColor({ palette.surface })
        List
        ListSelection({ selected })
        Children [
            { rows },
        ]
    }
}

/// Repaint the list column's own fill from the live palette.
pub fn sync_list_visuals(
    palette: Res<UiPalette>,
    mut lists: Query<&mut BackgroundColor, With<List>>,
) {
    for mut fill in &mut lists {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
    }
}

/// Project the list's [`ListSelection`] onto its rows' [`NavActive`] bits.
#[allow(clippy::type_complexity)]
pub fn sync_list_selection(
    mut lists: Query<(&ListSelection, &Children), (With<List>, Changed<ListSelection>)>,
    mut rows: Query<&mut NavActive>,
) {
    for (selection, children) in &mut lists {
        for (index, child) in children.iter().enumerate() {
            if let Ok(mut active) = rows.get_mut(*child) {
                let target = NavActive(selection.0 == Some(index));
                if active.0 != target.0 {
                    *active = target;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_height_binary_search_and_prefix_sums() {
        let mut index = DynamicHeightIndex::new(5, 30.0);
        assert_eq!(index.total_height_px(), 150.0);
        assert_eq!(index.item_offset_y(0), Some(0.0));
        assert_eq!(index.item_offset_y(3), Some(90.0));

        // Update height of item 1 from 30px to 50px
        assert!(index.update_measured_height(1, 50.0));
        assert_eq!(index.total_height_px(), 170.0);
        assert_eq!(index.item_offset_y(0), Some(0.0));
        assert_eq!(index.item_offset_y(1), Some(30.0));
        assert_eq!(index.item_offset_y(2), Some(80.0));
        assert_eq!(index.item_offset_y(3), Some(110.0));

        // Binary search lookup
        assert_eq!(index.find_index_at_offset(0.0), Some(0));
        assert_eq!(index.find_index_at_offset(29.9), Some(0));
        assert_eq!(index.find_index_at_offset(30.0), Some(1));
        assert_eq!(index.find_index_at_offset(79.9), Some(1));
        assert_eq!(index.find_index_at_offset(80.0), Some(2));
        assert_eq!(index.find_index_at_offset(160.0), Some(4));
    }

    #[test]
    fn inertial_scroller_physics_simulation() {
        let mut scroller = InertialScroller::default();
        scroller.fling(1000.0); // 1000 px/s downwards

        let mut offset = 0.0;
        let max_offset = 5000.0;

        for _ in 0..60 {
            let (new_offset, changed) = scroller.tick(1.0 / 60.0, offset, max_offset);
            if changed {
                assert!(new_offset >= offset);
                offset = new_offset;
            }
        }

        // Velocity should have decayed significantly
        assert!(scroller.velocity() < 50.0);
        assert!(offset > 0.0);
    }
}
