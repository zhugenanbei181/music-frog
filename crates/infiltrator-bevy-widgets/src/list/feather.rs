//! The list's edge feather: the transparent gradient that softens the two
//! scroll edges of a list viewport (EFFECTS-017 #62).
//!
//! The feather is a 16px overlay that dissolves the list's content into
//! the surface behind it, so a row is never cut off by the viewport edge.
//! It appears only where there is something to dissolve: the top feather
//! fades in as the list scrolls away from its first row, and the bottom
//! feather as the list approaches its last row.
//!
//! The fade is a *gradient*, not a stack of translucent strips: a strip
//! stack bands, and the bands' edges are exactly the hard cut the feather
//! exists to remove. The gradient is the official Bevy 0.20 UI gradient
//! ([`BackgroundGradient`]).
//!
//! The mask is a grid item of the list's frame, placed in the scroll
//! container's own cell: the frame's grid is what keeps the feather over
//! the scrolling content, and the container's own children stay exactly
//! the rows.

use crate::list::ListScrollContainer;
use crate::list::scrollbar::list_scroll_frame_with_overlays;
use crate::palette::UiPalette;
use crate::palette::rem;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::picking::Pickable;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignSelf, BackgroundGradient, ColorStop, ComputedNode, Gradient, GridPlacement,
    LinearGradient, Node, ScrollPosition, percent,
};
use bevy::ui_widgets::ScrollArea;

/// The feather's height in baseline pixels: the fade the two scroll edges of
/// a list wear.
pub const FEATHER_HEIGHT_PX: f32 = 16.0;

/// Which scroll edge of the list a feather mask sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListFeatherEdge {
    /// The top edge of the viewport: where the list's first rows leave.
    #[default]
    Top,
    /// The bottom edge of the viewport: where the list's last rows arrive.
    Bottom,
}

/// The feather mask over one scroll edge of a list.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListFeatherMask {
    /// The scroll edge the mask softens.
    pub edge: ListFeatherEdge,
}

/// The opacity the feather wears at one edge of a scrolled viewport.
///
/// The feather only appears where there is content to dissolve: at the top
/// edge that is the distance the list has scrolled past its first row, at
/// the bottom edge the distance it still has to go. Both are measured
/// against the feather's own height, so the fade completes inside the
/// feather itself.
pub fn feather_opacity(
    scroll_offset_px: f32,
    max_scroll_px: f32,
    edge: ListFeatherEdge,
    feather_px: f32,
) -> f32 {
    if feather_px <= 0.0 {
        return 0.0;
    }
    let remaining = match edge {
        ListFeatherEdge::Top => scroll_offset_px,
        ListFeatherEdge::Bottom => max_scroll_px - scroll_offset_px,
    };
    (remaining / feather_px).clamp(0.0, 1.0)
}

/// The feather's gradient: the surface behind the list, fading out over the
/// mask's height.
fn feather_gradient(surface: Color, opacity: f32, edge: ListFeatherEdge) -> Vec<Gradient> {
    let ink = surface.with_alpha(opacity);
    let clear = ink.with_alpha(0.0);
    let stops = vec![
        ColorStop::percent(ink, 0.0),
        ColorStop::percent(clear, 100.0),
    ];
    match edge {
        ListFeatherEdge::Top => vec![Gradient::Linear(LinearGradient::to_bottom(stops))],
        ListFeatherEdge::Bottom => vec![Gradient::Linear(LinearGradient::to_top(stops))],
    }
}

/// The opacity the feather's gradient currently paints with.
fn gradient_opacity(gradient: &BackgroundGradient) -> Option<f32> {
    gradient.0.first().and_then(|gradient| match gradient {
        Gradient::Linear(linear) => linear.stops.first().map(|stop| stop.color.alpha()),
        Gradient::Radial(_) | Gradient::Conic(_) => None,
    })
}

/// The feather mask over one scroll edge of the list.
///
/// The mask sits in the scroll container's own grid cell, aligned to the
/// edge it softens, and it never blocks the pointer: the wheel keeps
/// scrolling the list underneath it.
pub fn list_feather_mask_scene(edge: ListFeatherEdge, palette: &UiPalette) -> Box<dyn Scene> {
    let gradient = feather_gradient(palette.surface, 0.0, edge);
    let align = match edge {
        ListFeatherEdge::Top => AlignSelf::Start,
        ListFeatherEdge::Bottom => AlignSelf::End,
    };
    Box::new(bsn! {
        Node {
            grid_row: GridPlacement::start(1),
            grid_column: GridPlacement::start(1),
            align_self: { align },
            width: percent(100),
            height: rem(FEATHER_HEIGHT_PX),
        }
        BackgroundGradient({ gradient })
        Pickable { should_block_lower: false, is_hoverable: false }
        ListFeatherMask { edge: { edge } }
    })
}

/// The list's grid frame with the scroll container, the scrollbar and the
/// two edge feathers.
///
/// The feathers are the frame's overlays: they sit in the content cell, the
/// same cell the scroll container occupies, so they float over the
/// scrolling rows without becoming part of them.
pub fn list_scroll_frame_with_feathers(
    container: Box<dyn Scene>,
    scrollbar: Box<dyn Scene>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    list_scroll_frame_with_overlays(
        container,
        scrollbar,
        vec![
            list_feather_mask_scene(ListFeatherEdge::Top, palette),
            list_feather_mask_scene(ListFeatherEdge::Bottom, palette),
        ],
    )
}

/// Repaint every list's edge feathers from the live scroll position.
///
/// The feather's opacity follows the scroll: the top feather is invisible
/// while the list sits at its first row and opaque once the list has
/// scrolled a feather's height, and the bottom feather does the same at
/// the other end. A list that is shorter than its viewport has nothing to
/// dissolve at either edge, so neither feather appears.
pub fn sync_list_feather_masks(
    palette: Res<UiPalette>,
    parents: Query<&ChildOf>,
    containers: Query<(Entity, &ChildOf), With<ListScrollContainer>>,
    viewports: Query<(&ComputedNode, &ScrollPosition), With<ScrollArea>>,
    mut masks: Query<(Entity, &ListFeatherMask, &mut BackgroundGradient)>,
) {
    for (entity, mask, mut gradient) in &mut masks {
        let Ok(parent) = parents.get(entity) else {
            continue;
        };
        // The mask and the scroll container are siblings in the list's
        // frame, so the shared parent identifies the pair.
        let Some((container, _)) = containers
            .iter()
            .find(|(_, container_parent)| container_parent.0 == parent.0)
        else {
            continue;
        };
        let Ok((computed, position)) = viewports.get(container) else {
            continue;
        };
        let visible = computed.size().y * computed.inverse_scale_factor;
        let content = computed.content_size().y * computed.inverse_scale_factor;
        let max_scroll = (content - visible).max(0.0);
        let opacity = feather_opacity(position.y, max_scroll, mask.edge, FEATHER_HEIGHT_PX);
        if gradient_opacity(&gradient).is_some_and(|current| (current - opacity).abs() < 1.0e-4) {
            continue;
        }
        *gradient = BackgroundGradient(feather_gradient(palette.surface, opacity, mask.edge));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_feather_only_appears_once_the_list_has_scrolled() {
        assert_eq!(
            feather_opacity(0.0, 200.0, ListFeatherEdge::Top, FEATHER_HEIGHT_PX),
            0.0
        );
        assert_eq!(
            feather_opacity(8.0, 200.0, ListFeatherEdge::Top, FEATHER_HEIGHT_PX),
            0.5
        );
        assert_eq!(
            feather_opacity(40.0, 200.0, ListFeatherEdge::Top, FEATHER_HEIGHT_PX),
            1.0
        );
    }

    #[test]
    fn the_bottom_feather_only_appears_before_the_list_ends() {
        assert_eq!(
            feather_opacity(200.0, 200.0, ListFeatherEdge::Bottom, FEATHER_HEIGHT_PX),
            0.0
        );
        assert_eq!(
            feather_opacity(192.0, 200.0, ListFeatherEdge::Bottom, FEATHER_HEIGHT_PX),
            0.5
        );
        assert_eq!(
            feather_opacity(160.0, 200.0, ListFeatherEdge::Bottom, FEATHER_HEIGHT_PX),
            1.0
        );
    }

    #[test]
    fn a_list_that_does_not_scroll_has_no_feather_at_either_edge() {
        assert_eq!(
            feather_opacity(0.0, 0.0, ListFeatherEdge::Top, FEATHER_HEIGHT_PX),
            0.0
        );
        assert_eq!(
            feather_opacity(0.0, 0.0, ListFeatherEdge::Bottom, FEATHER_HEIGHT_PX),
            0.0
        );
    }

    #[test]
    fn a_feather_without_height_never_fades() {
        assert_eq!(feather_opacity(40.0, 200.0, ListFeatherEdge::Top, 0.0), 0.0);
    }

    #[test]
    fn the_top_feather_is_opaque_at_its_own_edge_and_clear_at_the_other() {
        let gradient = feather_gradient(Color::WHITE, 1.0, ListFeatherEdge::Top);
        let Gradient::Linear(linear) = &gradient[0] else {
            panic!("the feather paints a linear gradient");
        };
        assert_eq!(linear.stops.len(), 2);
        assert_eq!(linear.stops[0].color.alpha(), 1.0);
        assert_eq!(linear.stops[1].color.alpha(), 0.0);
    }

    #[test]
    fn the_bottom_feather_runs_the_other_way() {
        let gradient = feather_gradient(Color::WHITE, 1.0, ListFeatherEdge::Bottom);
        let Gradient::Linear(linear) = &gradient[0] else {
            panic!("the feather paints a linear gradient");
        };
        assert_eq!(linear.stops[0].color.alpha(), 1.0);
        assert_eq!(linear.stops[1].color.alpha(), 0.0);
        assert_ne!(linear.angle, LinearGradient::TO_BOTTOM);
    }
}
