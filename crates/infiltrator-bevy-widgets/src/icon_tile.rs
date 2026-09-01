//! Icon tile: the recessed accent square semantic icons sit on (the
//! stat-chip / identity-block visual anchor).
//!
//! Same contract as the other controls: pure function core
//! ([`icon_tile_fill`] / [`icon_tile_tint`]), a `bsn!` scene adapter
//! ([`icon_tile_scene`]) and a compare-and-set repaint system
//! ([`sync_icon_tile_visuals`], the checkbox idiom) so a palette swap
//! re-projects the tile fill and its icon's tint with no switch-specific
//! hook and no tree rebuild.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, Val, px};

use crate::icon::{IconId, IconTint, icon_scene};
use crate::palette::UiPalette;
use crate::theme::radius;

/// Marker on the tile root; [`sync_icon_tile_visuals`] re-projects its fill
/// and its icon child's tint from the live palette.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IconTile;

/// The tile fill: the recessed accent token (an accent-tinted translucent
/// wash that composites over whatever surface the tile sits on). Pure
/// function — headless-testable without any app.
pub fn icon_tile_fill(palette: &UiPalette) -> Color {
    palette.icon_tile
}

/// The tint the tile's icon draws with: the plain accent. Pure function.
pub fn icon_tile_tint(palette: &UiPalette) -> Color {
    palette.accent
}

/// One icon tile: a rounded square (control radius) centering the semantic
/// icon at the classic ~55% plate scale. `size_px` is the outer square; the
/// icon inside scales with it.
pub fn icon_tile_scene(icon: IconId, size_px: f32, palette: &UiPalette) -> impl Scene + use<> {
    let tint = icon_tile_tint(palette);
    bsn! {
        Node {
            width: px(size_px),
            height: px(size_px),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(radius::CONTROL)),
        }
        BackgroundColor({ icon_tile_fill(palette) })
        IconTile
        Children [
            ( { icon_scene(icon, size_px * 0.55, tint) } ),
        ]
    }
}

/// Repaint icon tiles from the live palette: the root fill and the icon
/// child's tint. Compare-and-set; the icon's drawn image color follows its
/// tint through [`crate::icon::sync_icon_tints`].
#[allow(clippy::type_complexity)]
pub fn sync_icon_tile_visuals(
    palette: Res<UiPalette>,
    tiles: Query<(Entity, &IconTile, &Children)>,
    mut fills: Query<&mut BackgroundColor>,
    mut tints: Query<&mut IconTint>,
) {
    let fill = icon_tile_fill(&palette);
    let tint = icon_tile_tint(&palette);
    for (entity, _tile, children) in &tiles {
        if let Ok(mut bg) = fills.get_mut(entity)
            && bg.0 != fill
        {
            bg.0 = fill;
        }
        for child in children.iter() {
            if let Ok(mut icon_tint) = tints.get_mut(*child)
                && icon_tint.0 != tint
            {
                icon_tint.0 = tint;
            }
        }
    }
}
