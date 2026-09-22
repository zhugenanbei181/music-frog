//! DUAL-11-08: the Iced rules list's virtual window.
//!
//! The list renders the *filtered* rule list through the shared
//! [`infiltrator_domain::rules::view::rule_window`] reduction: fixed-height
//! rows at [`infiltrator_domain::rules::view::RULE_ROW_HEIGHT_PX`], only the
//! visible band plus a fixed overscan, and two spacer elements that keep the
//! scrollable content exactly `total * row_height` tall. Scrolling the
//! scrollable publishes its viewport, which drives the window; the window
//! never depends on the list length, so a 50,000-entry profile renders the
//! same bounded number of rows as a 5-entry one.
//!
//! This module owns the pure part (which rows, which spacers, which page the
//! viewport is on) so it is headless-testable; `rules.rs` owns the widgets.

use crate::state::AppState;
use infiltrator_domain::rules::view;

/// Scrollable id the paging buttons target with `scroll_to`.
pub const RULES_LIST_SCROLL_ID: &str = "rules_list_scroll";

/// The window the rules list renders for the current scroll offset, measured
/// viewport height and filtered length.
pub fn rules_window(state: &AppState) -> view::RuleWindow {
    view::rule_window(
        state.editor.rules_scroll_offset_px,
        state.editor.rules_viewport_px,
        state.editor.rules_filtered_indices.len(),
    )
}

/// Cache indices (into `state.editor.rules_render_cache`) the view renders, in
/// list order.
///
/// This is exactly the list the row loop iterates: its length is the number of
/// rule row elements the surface builds for the frame, bounded by
/// [`view::rendered_row_bound`] for any profile size.
pub fn visible_rule_items(state: &AppState) -> Vec<usize> {
    let window = rules_window(state);
    let cache_len = state.editor.rules_render_cache.len();
    (window.start..window.end)
        .filter(|cache_index| *cache_index < cache_len)
        .collect()
}

/// DUAL-11-08: number of rule rows the view builds for the current window
/// (one row element per rendered rule, no matter how long the profile is).
pub fn rendered_rule_rows(state: &AppState) -> usize {
    visible_rule_items(state).len()
}

/// `(top, bottom)` spacer heights that keep the scrollable content the full
/// filtered-list height while only the window is rendered.
pub fn rules_window_spacers(state: &AppState) -> (f32, f32) {
    let window = rules_window(state);
    (window.top_spacer_px, window.bottom_spacer_px)
}

/// First and last rendered filtered positions, 1-based and inclusive for the
/// "showing N–M of T" caption. `(0, 0)` for an empty list.
pub fn rules_window_range(state: &AppState) -> (usize, usize) {
    let window = rules_window(state);
    if window.is_empty() {
        (0, 0)
    } else {
        (window.start + 1, window.end)
    }
}

/// The page the viewport currently shows. The paging indicator follows the
/// scroll offset — specifically the page of the topmost *visible* row, which
/// the overscan above the band does not shift — instead of a second cursor.
pub fn rules_window_page(state: &AppState) -> usize {
    let total = state.editor.rules_filtered_indices.len();
    if total == 0 {
        return 0;
    }
    let first_visible =
        view::rule_index_at_scroll_offset(state.editor.rules_scroll_offset_px).min(total - 1);
    view::page_for_rule_index(first_visible, state.editor.rules_page_size)
}

/// Scroll offset that brings the shared page's first row to the top.
pub fn page_scroll_offset(state: &AppState, page: usize) -> f32 {
    let page = view::clamp_page(
        page,
        state.editor.rules_filtered_indices.len(),
        state.editor.rules_page_size,
    );
    let (start, _) = view::page_bounds(
        page,
        state.editor.rules_filtered_indices.len(),
        state.editor.rules_page_size,
    );
    view::rule_scroll_offset_for_index(start)
}

/// Paging bounds over the filtered list for the current page size.
pub fn rules_page_bounds(state: &AppState) -> (usize, usize) {
    view::page_bounds(
        rules_window_page(state),
        state.editor.rules_filtered_indices.len(),
        state.editor.rules_page_size,
    )
}
