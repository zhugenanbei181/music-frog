//! Surface panels: the token-backed card chrome scenes compose into.
//!
//! Callers pass already-composed `Scene` values as children; the module
//! never creates children through commands or a builder API (charter law:
//! static structure composes declaratively with `bsn!`).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, percent, px,
};

use crate::palette::UiPalette;
use crate::theme::space;

/// Marker on the surface card root: the fill [`sync_surface_visuals`]
/// re-projects from the live palette (compare-and-set — a theme switch
/// repaints cards with no switch-specific hook).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfacePanel;

/// A token-backed card that accepts a dynamic scene list as its children.
pub fn surface_scene(children: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S8),
            padding: UiRect::all(Val::Px(space::S16)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        SurfacePanel
        Children [
            { children },
        ]
    }
}

/// Repaint every surface card from the live palette. Compare-and-set:
/// unchanged frames cost nothing.
pub fn sync_surface_visuals(
    palette: Res<UiPalette>,
    mut cards: Query<&mut BackgroundColor, With<SurfacePanel>>,
) {
    for mut fill in &mut cards {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
    }
}
