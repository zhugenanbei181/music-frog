//! Navigation item: the sidebar route pill (selected = accent fill with
//! `on_accent` ink, idle = the elevated control surface with ordinary
//! ink).
//!
//! Same contract as the other controls: pure function core
//! ([`nav_fill`] / [`nav_label_ink`]), a `bsn!` scene adapter
//! ([`nav_item_scene`]) and a compare-and-set repaint system
//! ([`sync_nav_visuals`], the checkbox idiom) so a palette swap or an
//! active-bit flip re-projects fill and label ink in place — no switch
//! hook, no tree rebuild. Activation wiring belongs to the caller: the
//! item is a plain node, not the official `Button`, because a nav target
//! that routes nowhere must not pretend to be pressable.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, Node, UiRect, Val, px};
use bevy::ui::widget::Text;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Marker on the item root; [`sync_nav_visuals`] re-projects its fill and
/// its label's ink from [`NavActive`] and the live palette.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavItem;

/// The page-owned active bit. Spawned state; later flips restamp the
/// visuals through the sync system without any remount.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavActive(pub bool);

/// Marker on the item's label; its ink follows the active bit.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavLabel;

/// The item fill: accent while active, the elevated control surface
/// otherwise. Pure function — headless-testable without any app.
pub fn nav_fill(active: bool, palette: &UiPalette) -> Color {
    if active {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

/// The label ink: `on_accent` while active, ordinary ink otherwise. Pure
/// function.
pub fn nav_label_ink(active: bool, palette: &UiPalette) -> Color {
    if active {
        palette.on_accent
    } else {
        palette.ink
    }
}

/// One navigation item: a control-height pill row carrying its label. The
/// `flex_grow` root lets callers park a trailing caption beside it (a
/// disabled "not migrated" tag) or stack items in a column.
pub fn nav_item_scene(label: String, active: bool, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            flex_grow: 1.0,
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ nav_fill(active, palette) })
        NavItem
        NavActive(active)
        Children [
            ( Text(label) TextRole(Role::Body) NavLabel ),
        ]
    }
}

/// Repaint every nav item from its [`NavActive`] bit and the live palette:
/// root fill and label ink, compare-and-set. Unchanged frames cost
/// nothing; a theme switch repaints items without any switch-specific hook.
#[allow(clippy::type_complexity)]
pub fn sync_nav_visuals(
    palette: Res<UiPalette>,
    items: Query<(Entity, &NavActive, &Children), With<NavItem>>,
    mut fills: Query<&mut BackgroundColor>,
    mut labels: Query<(&NavLabel, &mut TextColor)>,
) {
    for (entity, active, children) in &items {
        let fill = nav_fill(active.0, &palette);
        if let Ok(mut bg) = fills.get_mut(entity)
            && bg.0 != fill
        {
            bg.0 = fill;
        }
        let ink = nav_label_ink(active.0, &palette);
        for child in children.iter() {
            if let Ok((_, mut label_ink)) = labels.get_mut(*child)
                && label_ink.0 != ink
            {
                label_ink.0 = ink;
            }
        }
    }
}
