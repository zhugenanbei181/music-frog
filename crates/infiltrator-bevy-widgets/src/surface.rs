//! Surface panels: the token-backed card chrome scenes compose into.
//!
//! Callers pass already-composed `Scene` values as children; the module
//! never creates children through commands or a builder API (charter law:
//! static structure composes declaratively with `bsn!`).

use crate::palette::UiPalette;
use crate::palette::rem;
use crate::shader_fx::ModernSurfaceMaterial;
use crate::theme::space;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{BackgroundColor, BorderRadius, FlexDirection, UiRect, percent, px};
use bevy::ui_render::ui_material::MaterialNode;
use zhuge_ui_kit_bevy::surface::{self, SurfaceLayout};

/// Marker on the surface card root: the fill [`sync_surface_visuals`]
/// re-projects from the live palette (compare-and-set — a theme switch
/// repaints cards with no switch-specific hook).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfacePanel;

/// A token-backed card that accepts a dynamic scene list as its children.
pub fn surface_scene(children: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
    let scene = surface::surface_scene(
        SurfaceLayout {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            direction: FlexDirection::Column,
            gap: rem(space::S8),
            padding: UiRect::all(rem(space::S16)),
            radius: BorderRadius::all(rem(palette.card_radius_px)),
        },
        palette.surface,
        children,
    );
    bsn! { @{ scene } SurfacePanel }
}

/// Repaint every surface card from the live palette. Compare-and-set:
/// unchanged frames cost nothing.
pub fn sync_surface_visuals(
    palette: Res<UiPalette>,
    mut cards: Query<
        (
            &mut BackgroundColor,
            Option<&MaterialNode<ModernSurfaceMaterial>>,
        ),
        With<SurfacePanel>,
    >,
) {
    for (mut fill, material) in &mut cards {
        let color = if material.is_some() {
            Color::NONE
        } else {
            palette.surface
        };
        if fill.0 != color {
            fill.0 = color;
        }
    }
}
