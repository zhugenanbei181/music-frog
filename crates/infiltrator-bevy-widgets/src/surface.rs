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
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::Node;
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, BorderRadius, FlexDirection, UiRect, percent, px,
};
use bevy::ui_render::ui_material::MaterialNode;
/// Marker on the surface card root: the fill [`sync_surface_visuals`]
/// re-projects from the live palette (compare-and-set — a theme switch
/// repaints cards with no switch-specific hook).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfacePanel;

type SurfaceVisualView<'a> = (
    &'a mut BackgroundColor,
    &'a mut BorderColor,
    Option<&'a MaterialNode<ModernSurfaceMaterial>>,
);

/// A token-backed card that accepts a dynamic scene list as its children.
pub fn surface_scene(children: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
    let fill = palette.surface;
    let border = palette.border;
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S8),
                padding: UiRect::all(rem(space::S16)),
                border: UiRect::all(px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
            }
            BackgroundColor({ fill })
            BorderColor { top: border, right: border, bottom: border, left: border }
            SurfacePanel
            Children [
                { children }
            ]
    }
}

/// Repaint every surface card from the live palette. Compare-and-set:
/// unchanged frames cost nothing.
pub fn sync_surface_visuals(
    palette: Res<UiPalette>,
    mut cards: Query<SurfaceVisualView, With<SurfacePanel>>,
) {
    let border_color = palette.border;
    for (mut fill, mut border, material) in &mut cards {
        let color = if material.is_some() {
            Color::NONE
        } else {
            palette.surface
        };
        if fill.0 != color {
            fill.0 = color;
        }
        if border.top != border_color
            || border.right != border_color
            || border.bottom != border_color
            || border.left != border_color
        {
            border.top = border_color;
            border.right = border_color;
            border.bottom = border_color;
            border.left = border_color;
        }
    }
}
