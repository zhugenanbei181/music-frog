//! The official Bevy 0.20 scroll primitives for the shared list.
//!
//! Two official widgets own the list's scrolling:
//!
//! - the unstyled [`Scrollbar`] sits in the second column of the list's grid
//!   frame, beside the official [`ScrollArea`] container. The official
//!   [`bevy::ui_widgets::ScrollbarPlugin`] owns the thumb's size tracking and
//!   the drag behaviour; this module only declares the scene, binds the bar to
//!   its container and paints it with the product's theme tokens.
//! - the official [`ScrollIntoView`] event reveals a row: the viewport's own
//!   geometry decides how far the list scrolls, so the list keeps no second
//!   scroll offset of its own.
//!
//! The bar's target is the container entity, and only a mounted scene knows
//! that entity, so [`bind_list_scrollbars`] performs the binding once the
//! scene is in the world.

use crate::feathers_theme::Token;
use crate::list::{ListChangedFilter, ListSelection};
use crate::palette::rem;
use crate::theme::{REM_BASE, radius, space};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use bevy::scene::{Scene, bsn};
use bevy::text::RemSize;
use bevy::ui::prelude::{
    BorderRadius, Display, GridPlacement, Node, RepeatedGridTrack, UiRect, percent,
};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, ScrollIntoView, Scrollbar, ScrollbarThumb};

/// Marker on the list's official scroll container: the node that owns the
/// rows and carries the official [`ScrollArea`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListScrollContainer;

/// Marker on the official scrollbar that is bound to a
/// [`ListScrollContainer`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListScrollbar;

/// The official vertical scrollbar: a rem-sized track in the frame's second
/// column, with the official thumb inside it.
pub fn list_scrollbar_scene() -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_width: rem(space::S8),
                grid_row: GridPlacement::start(1),
                grid_column: GridPlacement::start(2),
            }
            ListScrollbar
            Children [
                ScrollbarThumb {
                    border_radius: BorderRadius::all(rem(radius::XS)),
                    border: UiRect::all(rem(space::S2)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                ThemeBorderColor({ Token::Border })
            ]
    }) as Box<dyn Scene>
}

/// The list's grid frame: the scroll container's cell plus the official
/// scrollbar in the frame's second column.
///
/// The frame is a grid because the bar must sit *beside* the viewport, not
/// inside it: a child of the scrolling container would travel with the
/// content.
pub fn list_scroll_frame(container: Box<dyn Scene>) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                display: Display::Grid,
                width: percent(100),
                grid_template_columns: vec![
                    RepeatedGridTrack::flex(1, 1.0),
                    RepeatedGridTrack::auto(1),
                ],
                grid_template_rows: vec![RepeatedGridTrack::flex(1, 1.0)],
                column_gap: rem(space::S2),
            }
            Children [
                @{ container }
                --
                @{ list_scrollbar_scene() }
            ]
    }) as Box<dyn Scene>
}

/// Bind every mounted list scrollbar to the list's official scroll container.
///
/// The bar and its container are siblings inside the list's grid frame, so the
/// shared parent identifies the pair. The official plugin then owns the
/// thumb's size and the drag behaviour.
pub fn bind_list_scrollbars(
    mut commands: Commands,
    rem_size: Res<RemSize>,
    bars: Query<(Entity, &ChildOf), (With<ListScrollbar>, Without<Scrollbar>)>,
    containers: Query<(Entity, &ChildOf), (With<ListScrollContainer>, With<ScrollArea>)>,
) {
    let min_thumb_length = space::S24 * rem_size.0 / REM_BASE;
    for (bar, bar_parent) in &bars {
        let Some((container, _)) = containers
            .iter()
            .find(|(_, container_parent)| container_parent.0 == bar_parent.0)
        else {
            continue;
        };
        commands.entity(bar).insert(Scrollbar::new(
            container,
            ControlOrientation::Vertical,
            min_thumb_length,
        ));
    }
}

/// Reveal the selected row of every list through the official
/// [`ScrollIntoView`] event.
///
/// The event carries no scroll arithmetic of its own: the official observer
/// measures the row against the viewport and moves the container's
/// [`bevy::ui::ScrollPosition`]. The list therefore keeps no second scroll
/// offset that could disagree with the viewport.
pub fn reveal_selected_list_rows(
    mut commands: Commands,
    mut lists: Query<(&ListSelection, &Children), ListChangedFilter>,
) {
    for (selection, children) in &mut lists {
        let Some(index) = selection.0 else {
            continue;
        };
        let Some(row) = children.get(index) else {
            continue;
        };
        commands.trigger(ScrollIntoView { entity: *row });
    }
}

/// Ask the official scroll-into-view mechanism to reveal one row.
pub fn scroll_item_into_view(commands: &mut Commands, item: Entity) {
    commands.trigger(ScrollIntoView { entity: item });
}
