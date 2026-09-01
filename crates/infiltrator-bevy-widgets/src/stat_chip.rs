//! Stat chip: the compact card that pairs an icon tile with a caption
//! label and a mono value (the Overview metrics row).
//!
//! Same contract as the other controls: pure function core
//! ([`stat_chip_fill`]), a `bsn!` scene adapter ([`stat_chip_scene`]) and a
//! compare-and-set repaint system ([`sync_stat_chip_visuals`], the
//! checkbox idiom) so a palette swap re-projects the chip fill with no
//! switch-specific hook and no tree rebuild. The value text carries the
//! [`StatChipValue`] marker so a page's refresh observer can restamp the
//! number in place — the chip never owns its data.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, px,
};
use bevy::ui::widget::Text;

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::{space, type_scale};

/// Minimum chip height (px) — one card row of the metrics band.
pub const CHIP_MIN_HEIGHT: f32 = 64.0;

/// Marker on the chip root; [`sync_stat_chip_visuals`] re-projects its
/// fill from the live palette.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatChip;

/// Marker on the chip's value text. The page owns the number: its refresh
/// observer finds this marker through the chip's children and restamps the
/// text in place (the checkbox-box routing idiom).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatChipValue;

/// The chip fill: the ordinary surface card token. Pure function —
/// headless-testable without any app.
pub fn stat_chip_fill(palette: &UiPalette) -> Color {
    palette.surface
}

/// One stat chip: an icon tile, then a column of caption label over mono
/// value. `flex_grow` lets a row of chips share the width evenly.
pub fn stat_chip_scene(
    icon: IconId,
    label: String,
    value: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            flex_grow: 1.0,
            min_height: px(CHIP_MIN_HEIGHT),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S12),
            padding: UiRect::all(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ stat_chip_fill(palette) })
        StatChip
        Children [
            ( { icon_tile_scene(icon, type_scale::HEADING + space::S16, palette) } ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text(label) TextRole(Role::Caption) ),
                    ( Text(value) TextRole(Role::Mono) StatChipValue ),
                ]
            ),
        ]
    }
}

/// Repaint every stat chip from the live palette. Compare-and-set:
/// unchanged frames cost nothing.
pub fn sync_stat_chip_visuals(
    palette: Res<UiPalette>,
    mut chips: Query<&mut BackgroundColor, With<StatChip>>,
) {
    let fill = stat_chip_fill(&palette);
    for mut bg in &mut chips {
        if bg.0 != fill {
            bg.0 = fill;
        }
    }
}
