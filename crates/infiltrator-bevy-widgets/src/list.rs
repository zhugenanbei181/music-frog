//! List: virtual-window pure core over nav-vocabulary rows.
//!
//! **Pure core**: [`visible_window`] is the whole virtualization contract —
//! the half-open item-index window `[start, end)` that fits a viewport of
//! `viewport_h` at row height `row_h` when scrolled to `scroll_offset`.
//! Independent implementation, same window mathematics taskmanager's table
//! established: partially visible rows count as visible, a scroll offset
//! past the end pins to the last full window (never an empty tail, never an
//! out-of-bounds range), and degenerate inputs (empty list, zero viewport,
//! zero/negative row height) yield an empty window instead of a panic.
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
