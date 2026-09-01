//! Scroll area: the official unstyled `bevy_ui_widgets` [`ScrollArea`]
//! behavior wrapped in token container chrome. The official widget owns the
//! wheel/trackpad scroll semantics (driven by pointer scroll events on the
//! windowed composition); this module owns the clipped viewport geometry and
//! the card surface it sits on.
//!
//! [`clamp_scroll`] is the pure projection the official observer applies
//! (delta applied, then clamped into the over-scroll range); it is kept
//! here as the headless-testable statement of that contract — the viewport
//! never scrolls past its content, and short content never scrolls at all.

use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderRadius, FlexDirection, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui_widgets::ScrollArea;

use crate::palette::UiPalette;

/// Clamp a scroll offset into the legal range for a viewport: at most
/// `content - viewport` (never less than zero), and zero entirely when the
/// content fits. Mirrors the official `ScrollArea` observer's clamp.
pub fn clamp_scroll(position: f32, content: f32, viewport: f32) -> f32 {
    position.clamp(0.0, (content - viewport).max(0.0))
}

/// A token card hosting one scrollable content scene. The viewport is
/// vertically scrollable and clips at the card's rounded edge; the caller
/// owns the viewport height (their layout decides) and the content scene.
pub fn scrollarea_scene(
    content: Box<dyn Scene>,
    viewport_height_px: f32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(viewport_height_px),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                    ( { content } ),
                ]
            ),
        ]
    }
}
