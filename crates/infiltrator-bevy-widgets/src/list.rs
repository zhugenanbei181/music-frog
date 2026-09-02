//! List: virtual-window pure core over nav-vocabulary rows, and high-performance
//! Virtual List state machine for massive datasets with viewport calculation and
//! overscan preloading.
//!
//! **Pure core**: [`visible_window`] and [`visible_window_with_overscan`] are the
//! virtualization mathematics — the half-open item-index window `[start, end)`
//! that fits a viewport of `viewport_h` at row height `row_h` when scrolled to
//! `scroll_offset`. Independent implementation, same window mathematics
//! taskmanager's table established: partially visible rows count as visible, a
//! scroll offset past the end pins to the last full window (never an empty tail,
//! never an out-of-bounds range), and degenerate inputs (empty list, zero
//! viewport, zero/negative row height) yield an empty window instead of a panic.
//!
//! **State machine**: [`VirtualListState`] encapsulates high-performance O(1)
//! viewport geometry, scroll offset clamping, overscan preloading buffers, and
//! selection state over massive (1,000,000+) node collections.
//!
//! **Scene adapter**: [`list_scene`] is a clipped, gap-spaced column over
//! caller-composed rows. Rows reuse the nav vocabulary — [`list_row_scene`]
//! builds nav-style pills (same [`crate::nav`] tokens, markers and repaint
//! path), so selection repaint and theme reskin ride the existing
//! [`crate::nav`] machinery. The list itself owns one thing: the
//! [`ListSelection`] bit, which [`sync_list_selection`] projects onto the
//! rows' [`crate::nav::NavActive`] components in place — flipping the
//! selection never remounts anything.

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

/// The half-open visible window `[start, end)` of item indices.
///
/// Contract (unit-tested):
/// - empty list, non-positive viewport or non-positive row height →
///   `(0, 0)` — an honest empty window, never a fabricated range;
/// - the window covers the viewport including one partially visible row;
/// - a scroll offset past the content pins to the last full window
///   (`end == item_count`); a negative offset pins to the first window;
/// - `end - start <= item_count` always.
pub fn visible_window(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
) -> (usize, usize) {
    if item_count == 0 || viewport_h <= 0.0 || row_h <= 0.0 {
        return (0, 0);
    }
    // Rows the viewport holds, rounded up so a partially visible row is
    // visible (clipping hides the sliver, not the row).
    let visible = ((viewport_h / row_h).ceil() as usize).clamp(1, item_count);
    let scrolled = ((scroll_offset / row_h).floor().max(0.0)) as usize;
    let start = scrolled.min(item_count - visible);
    (start, start + visible)
}

/// Resolved virtual window geometry for recycled virtual lists.
///
/// Provides the active item range and the virtual spacer heights above and
/// below the active slice so the scroll container maintains the true total
/// scrollable height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualWindow {
    /// Starting index (inclusive) of the window including overscan buffer.
    pub start: usize,
    /// Ending index (exclusive) of the window including overscan buffer.
    pub end: usize,
    /// Number of items mounted in this window (`end - start`).
    pub visible_count: usize,
    /// Virtual spacer height (px) preceding the first mounted row.
    pub top_spacer_px: f32,
    /// Virtual spacer height (px) succeeding the last mounted row.
    pub bottom_spacer_px: f32,
    /// Total virtual height (px) of all `item_count` rows.
    pub total_height_px: f32,
}

/// Compute a virtual window with an overscan buffer for smooth scrolling.
///
/// Contract (unit-tested):
/// - Expands `[start, end)` by up to `overscan` items in both directions;
/// - Clamps `start >= 0` and `end <= item_count`;
/// - Calculates exact spacer heights `top_spacer_px = start * row_h` and
///   `bottom_spacer_px = (item_count - end) * row_h`;
/// - `top_spacer_px + (end - start) * row_h + bottom_spacer_px == total_height_px`.
pub fn visible_window_with_overscan(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
    overscan: usize,
) -> VirtualWindow {
    if item_count == 0 || viewport_h <= 0.0 || row_h <= 0.0 {
        return VirtualWindow {
            start: 0,
            end: 0,
            visible_count: 0,
            top_spacer_px: 0.0,
            bottom_spacer_px: 0.0,
            total_height_px: 0.0,
        };
    }
    let total_height_px = item_count as f32 * row_h;
    let visible_rows = ((viewport_h / row_h).ceil() as usize).clamp(1, item_count);
    let scrolled = ((scroll_offset / row_h).floor().max(0.0)) as usize;
    let base_start = scrolled.min(item_count.saturating_sub(visible_rows));
    let base_end = (base_start + visible_rows).min(item_count);

    let start = base_start.saturating_sub(overscan);
    let end = (base_end + overscan).min(item_count);
    let visible_count = end.saturating_sub(start);
    let top_spacer_px = start as f32 * row_h;
    let bottom_spacer_px = (item_count.saturating_sub(end)) as f32 * row_h;

    VirtualWindow {
        start,
        end,
        visible_count,
        top_spacer_px,
        bottom_spacer_px,
        total_height_px,
    }
}

/// Clamp a raw scroll offset to valid bounds for a virtual list.
pub fn clamp_scroll_offset(
    scroll_offset: f32,
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
) -> f32 {
    let max_scroll = (item_count as f32 * row_h - viewport_h).max(0.0);
    scroll_offset.clamp(0.0, max_scroll)
}

/// High-performance Virtual List state machine.
///
/// Designed to support massive item collections (e.g. 1,000,000+ items) with
/// constant-time O(1) viewport calculations, smooth scrolling bounds, overscan
/// preloading buffers, hit testing, and selection tracking.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualListState {
    item_count: usize,
    row_height_px: f32,
    viewport_height_px: f32,
    scroll_offset_px: f32,
    overscan: usize,
    selected_index: Option<usize>,
}

impl Default for VirtualListState {
    fn default() -> Self {
        Self {
            item_count: 0,
            row_height_px: 36.0,
            viewport_height_px: 0.0,
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
        }
    }
}

impl VirtualListState {
    /// Create a new virtual list state with given item count, row height, and viewport height.
    pub fn new(item_count: usize, row_height_px: f32, viewport_height_px: f32) -> Self {
        let mut state = Self {
            item_count,
            row_height_px: row_height_px.max(1.0),
            viewport_height_px: viewport_height_px.max(0.0),
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
        };
        state.clamp_scroll();
        state
    }

    /// Set overscan buffer size in items.
    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Set initial scroll offset (automatically clamped to valid bounds).
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

    /// Height of each individual row in pixels.
    pub fn row_height_px(&self) -> f32 {
        self.row_height_px
    }

    /// Visible height of the viewport container in pixels.
    pub fn viewport_height_px(&self) -> f32 {
        self.viewport_height_px
    }

    /// Current scroll offset in pixels from top.
    pub fn scroll_offset_px(&self) -> f32 {
        self.scroll_offset_px
    }

    /// Number of overscan buffer items prepended and appended to visible window.
    pub fn overscan(&self) -> usize {
        self.overscan
    }

    /// Current selected item index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Total height of all items combined in pixels. O(1).
    pub fn total_height_px(&self) -> f32 {
        self.item_count as f32 * self.row_height_px
    }

    /// Maximum valid scroll offset in pixels. O(1).
    pub fn max_scroll_offset(&self) -> f32 {
        (self.total_height_px() - self.viewport_height_px).max(0.0)
    }

    /// Whether the list is scrolled to the top boundary.
    pub fn is_at_top(&self) -> bool {
        self.scroll_offset_px <= 0.0
    }

    /// Whether the list is scrolled to the bottom boundary.
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset_px >= self.max_scroll_offset()
    }

    /// Scroll progress fraction from 0.0 (top) to 1.0 (bottom).
    pub fn scroll_progress(&self) -> f32 {
        let max = self.max_scroll_offset();
        if max <= 0.0 {
            0.0
        } else {
            (self.scroll_offset_px / max).clamp(0.0, 1.0)
        }
    }

    /// The half-open item range `[start, end)` directly visible in the viewport without overscan.
    pub fn visible_range(&self) -> (usize, usize) {
        visible_window(
            self.item_count,
            self.viewport_height_px,
            self.row_height_px,
            self.scroll_offset_px,
        )
    }

    /// The resolved virtual window including overscan buffer and spacer heights. O(1).
    pub fn window(&self) -> VirtualWindow {
        visible_window_with_overscan(
            self.item_count,
            self.viewport_height_px,
            self.row_height_px,
            self.scroll_offset_px,
            self.overscan,
        )
    }

    /// Find item index at given vertical offset in list content space. O(1).
    pub fn item_at_offset(&self, offset_y_px: f32) -> Option<usize> {
        if offset_y_px < 0.0 || self.item_count == 0 || self.row_height_px <= 0.0 {
            return None;
        }
        let index = (offset_y_px / self.row_height_px).floor() as usize;
        if index < self.item_count {
            Some(index)
        } else {
            None
        }
    }

    /// Top coordinate (px) in content space of the item at `index`. O(1).
    pub fn item_offset_y(&self, index: usize) -> Option<f32> {
        if index < self.item_count {
            Some(index as f32 * self.row_height_px)
        } else {
            None
        }
    }

    /// Item vertical bounds `(top_y, bottom_y)` in content space. O(1).
    pub fn item_rect_y(&self, index: usize) -> Option<(f32, f32)> {
        let top = self.item_offset_y(index)?;
        Some((top, top + self.row_height_px))
    }

    /// Whether the item at `index` intersects the visible viewport.
    pub fn is_item_visible(&self, index: usize) -> bool {
        let (start, end) = self.visible_range();
        index >= start && index < end
    }

    /// Whether the item at `index` is inside the mounted overscan window.
    pub fn is_item_mounted(&self, index: usize) -> bool {
        let win = self.window();
        index >= win.start && index < win.end
    }

    // --- State Mutations & Navigation ---

    fn clamp_scroll(&mut self) {
        self.scroll_offset_px = clamp_scroll_offset(
            self.scroll_offset_px,
            self.item_count,
            self.viewport_height_px,
            self.row_height_px,
        );
    }

    /// Update total item count and re-clamp scroll and selection.
    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
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

    /// Update row height and re-clamp scroll offset.
    pub fn set_row_height(&mut self, height_px: f32) {
        self.row_height_px = height_px.max(1.0);
        self.clamp_scroll();
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

    /// Scroll by a delta in pixels. Positive scrolls down, negative scrolls up.
    /// Returns true if offset changed.
    pub fn scroll_by(&mut self, delta_px: f32) -> bool {
        self.set_scroll_offset(self.scroll_offset_px + delta_px)
    }

    /// Scroll to make the given item visible in the viewport.
    /// If item is above viewport, scrolls to place it at the top.
    /// If item is below viewport, scrolls to place its bottom at viewport bottom.
    /// Returns true if scroll offset changed.
    pub fn scroll_to_index(&mut self, index: usize) -> bool {
        if index >= self.item_count {
            return false;
        }
        let item_top = index as f32 * self.row_height_px;
        let item_bottom = item_top + self.row_height_px;
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

    /// Scroll to place the item centered in the viewport.
    /// Returns true if scroll offset changed.
    pub fn scroll_to_index_centered(&mut self, index: usize) -> bool {
        if index >= self.item_count {
            return false;
        }
        let item_center = (index as f32 + 0.5) * self.row_height_px;
        let target_offset = item_center - (self.viewport_height_px * 0.5);
        self.set_scroll_offset(target_offset)
    }

    /// Scroll to top of list. Returns true if changed.
    pub fn scroll_to_top(&mut self) -> bool {
        self.set_scroll_offset(0.0)
    }

    /// Scroll to bottom of list. Returns true if changed.
    pub fn scroll_to_bottom(&mut self) -> bool {
        self.set_scroll_offset(self.max_scroll_offset())
    }

    /// Scroll down by one page (viewport height). Returns true if changed.
    pub fn page_down(&mut self) -> bool {
        self.scroll_by(self.viewport_height_px)
    }

    /// Scroll up by one page (viewport height). Returns true if changed.
    pub fn page_up(&mut self) -> bool {
        self.scroll_by(-self.viewport_height_px)
    }

    /// Set selection index. Out-of-bounds indices are rejected. Returns true if selection changed.
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

    /// Advance selection to next item. Returns true if changed.
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

    /// Move selection to previous item. Returns true if changed.
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

/// Component carrying virtual list state.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct VirtualList(pub VirtualListState);

/// Message requesting a scroll delta on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct VirtualListScroll(pub f32);

/// Message requesting a selection change on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualListSelect(pub Option<usize>);

/// Drive virtual lists from incoming scroll and select messages.
pub fn advance_virtual_lists(
    mut scrolls: MessageReader<VirtualListScroll>,
    mut selects: MessageReader<VirtualListSelect>,
    mut lists: Query<&mut VirtualList>,
) {
    for event in scrolls.read() {
        for mut list in &mut lists {
            list.0.scroll_by(event.0);
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
/// Spawned state; a later flip re-projects the rows in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListSelection(pub Option<usize>);

/// One list row: the nav-item pill vocabulary (accent fill + `on_accent` ink
/// when selected, elevated surface + ordinary ink otherwise) as a plain node
/// — a list row routes nowhere by itself, so unlike [`crate::nav::
/// nav_item_scene`] it is not the official `Button` and carries no
/// `flex_grow` (rows must keep their own height inside the column).
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

/// The list: a clipped, token-spaced column over caller-composed rows
/// (compose them with [`list_row_scene`] for the standard vocabulary). The
/// `selected` index rides [`ListSelection`] for the host to flip later;
/// the initial row bits are the caller's scenes' own state, and the first
/// sync pass converges both onto the same truth.
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

/// Repaint the list column's own fill from the live palette
/// (compare-and-set); rows repaint via [`crate::nav`].
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

/// Project the list's [`ListSelection`] onto its rows' [`NavActive`] bits,
/// compare-and-set: the selection flip is a component restamp, and the rows'
/// own repaint (fill + label ink, plus theme reskin) rides the existing
/// [`crate::nav`] sync system — this module never paints a row itself.
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
