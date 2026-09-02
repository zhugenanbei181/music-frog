//! The product pages of the Bevy frontend.
//!
//! **Page contract** (taskmanager-proven shape, one module per page):
//! a page exposes one scene constructor that takes the projection data it
//! renders plus the [`UiPalette`](infiltrator_bevy_widgets::palette::
//! UiPalette), and returns an `impl Scene` — the *only* entry a route's
//! scene table knows. Page roots carry a
//! [`PageRoot`](crate::route::PageRoot) marker naming their route.
//!
//! - Static structure composes declaratively with `bsn!` through the
//!   widget layer's `*_scene` adapters; the bsn! guard enforces it.
//! - Data never bakes into structure the page can't update: mutable text
//!   and fill nodes carry typed markers, and the page self-registers one
//!   refresh observer (via the root's `on_insert` bind hook, once per
//!   world) that restamps those components in place when its typed
//!   projection event fires. No polling, no tree rebuilds.
//! - All colors come from palette tokens; fonts come from text roles.
//!   Pages never touch bevy color/asset constructors directly.

pub mod app_routing;
pub mod connections;
pub mod dns;
pub mod doctor;
pub mod logs;
pub mod overview;
pub mod profiles;
pub mod proxies;
pub mod rules;
pub mod settings;
pub mod sync;

use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::route::{PageRoot, Route};

/// Generic placeholder scene for routes undergoing migration in the 0.30 blueprint.
///
/// Bounded subtree replacement mounts this scene under the shell's `ContentSlot`.
pub fn placeholder_page(route: Route, palette: &UiPalette) -> impl Scene + use<> {
    let title = route.label().to_owned();
    let subtitle = format!("{} · 0.30 迁移蓝图演进中", route.label());
    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(space::S16)),
                row_gap: Val::Px(space::S12),
            }
            PageRoot(route)
            Children [
                ( Text({ title }) TextRole(Role::Heading) ),
                ( Text({ subtitle }) TextRole(Role::Caption) ),
            ]
        })],
        palette,
    )
}
