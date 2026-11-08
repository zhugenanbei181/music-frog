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
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::Scene;
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, BorderRadius, FlexDirection, UiRect, percent, px,
};
use bevy::ui_render::ui_material::MaterialNode;
use zhuge_ui_kit_bevy::surface::{CardLayout, CardSurface, card_surface_scene};

type SurfaceVisualView<'a> = (
    &'a mut BackgroundColor,
    &'a mut BorderColor,
    Option<&'a MaterialNode<ModernSurfaceMaterial>>,
);

/// A token-backed card that accepts a dynamic scene list as its children.
///
/// The card geometry — the padded, bordered column — is owned by
/// `zhuge-ui-kit-bevy::surface::card_surface_scene`. This function keeps only
/// MusicFrog's token mapping, and the crate's own surface identity is the kit's
/// [`CardSurface`] marker: one marker for one surface, not a product-local
/// copy of it.
pub fn surface_scene(children: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
    let fill = palette.surface;
    let border = palette.border;
    card_surface_scene(
        CardLayout {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            direction: FlexDirection::Column,
            gap: rem(space::S8),
            padding: UiRect::all(rem(space::S16)),
            border: UiRect::all(px(palette.hairline_px)),
            radius: BorderRadius::all(rem(palette.card_radius_px)),
            shadow: Vec::new(),
        },
        fill,
        BorderColor {
            top: border,
            right: border,
            bottom: border,
            left: border,
        },
        children,
    )
}

/// Repaint every surface card from the live palette. Compare-and-set:
/// unchanged frames cost nothing.
pub fn sync_surface_visuals(
    palette: Res<UiPalette>,
    mut cards: Query<SurfaceVisualView, With<CardSurface>>,
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
