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

use crate::feathers_theme::Token;
use crate::palette::{UiPalette, rem};
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BorderRadius, Node, UiRect};
use bevy::ui::widget::Text;

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
pub fn nav_fill_token(active: bool) -> Token {
    if active {
        Token::Accent
    } else {
        Token::SurfaceElevated
    }
}

/// The label ink: `on_accent` while active, ordinary ink otherwise. Pure
/// function.
pub fn nav_label_ink_token(active: bool) -> Token {
    if active { Token::OnAccent } else { Token::Ink }
}

/// One navigation item: a control-height pill row carrying its label. The
/// `flex_grow` root lets callers park a trailing caption beside it (a
/// disabled "not migrated" tag) or stack items in a column.
pub fn nav_item_scene(label: String, active: bool, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                flex_grow: 1.0,
                min_height: rem(palette.control_height_px),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ nav_fill_token(active) })
            NavItem
            NavActive(active)
            Children [
                Text(label) TextRole(Role::Body) NavLabel
            ]
    }
}

/// Repaint every nav item from its [`NavActive`] bit and the live palette:
/// root fill and label ink, compare-and-set. Unchanged frames cost
/// nothing; a theme switch repaints items without any switch-specific hook.
pub fn sync_nav_visuals(
    mut commands: Commands,
    items: Query<(Entity, &NavActive, &Children, Option<&ThemeBackgroundColor>), With<NavItem>>,
    labels: Query<(Entity, &NavLabel, Option<&ThemeTextColor>)>,
) {
    for (entity, active, children, fill) in &items {
        let fill_token = nav_fill_token(active.0);
        if fill
            .map(|f| f.0 != fill_token.theme_token())
            .unwrap_or(true)
        {
            commands
                .entity(entity)
                .insert(ThemeBackgroundColor(fill_token.into()));
        }
        let ink_token = nav_label_ink_token(active.0);
        for child in children.iter() {
            if let Ok((label_entity, _, ink)) = labels.get(*child)
                && ink.map(|t| t.0 != ink_token.theme_token()).unwrap_or(true)
            {
                commands
                    .entity(label_entity)
                    .insert(ThemeTextColor(ink_token.into()));
            }
        }
    }
}
