//! Headless shell tests: the real `ShellPlugin` on `MinimalPlugins` — no
//! window, no render hardware. Asserts the sidebar/content shell mounts,
//! typography is stamped by the widget layer's observer, the AccessKit
//! semantic seeds land with the right roles, the theme switch restamps the
//! mounted tree in place (zero remounts), and the theme flip repaints every
//! token-filled sidebar surface (rail, nav items, routing intent pills)
//! without changing a single entity id.

use crate::support::subtree_with;
use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::camera::Camera2d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::world::World;
use bevy::scene::ScenePlugin;
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, ContentSlot, ContentTitleLabel, GlobalStatusDot,
    HistoryBackButton, HistoryForwardButton, LayoutMode, RoutingIntentStatusCapsule, ShellHeader,
    ShellLayoutState, ShellPlugin, ShellRoot, SidebarNavItem, SidebarPanel,
};
use infiltrator_bevy_ui::pages::overview_cards::OverviewRoutingIntentSegmentPill;
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_ui::shell_scene::nav::{NavGroupHeading, NavIndicator};
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavRowKind};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::REM_BASE;
use infiltrator_bevy_widgets::theme::{Breakpoint, Theme, ThemeSkin};
use infiltrator_contract::design_resources;

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app
}

fn heading_entity(world: &mut World) -> Entity {
    let mut headings = world.query::<(Entity, &TextRole)>();
    headings
        .iter(world)
        .find(|(_, role)| role.0 == design_resources::TextRole::Heading)
        .expect("title row heading text mounted")
        .0
}

/// The sidebar rail's own entity: the single `SidebarPanel` the shell mounts.
fn rail_entity(world: &mut World) -> Entity {
    world
        .query::<(Entity, &SidebarPanel)>()
        .single(world)
        .expect("one sidebar rail")
        .0
}

/// Whether `T` appears anywhere under `root`.
fn subtree_contains<T: Component>(world: &World, root: Entity) -> bool {
    !subtree_with::<T>(world, root).is_empty()
}

#[path = "shell_tests/bidi_direction_tests.rs"]
mod bidi_direction_tests;
#[path = "shell_tests/bottom.rs"]
mod bottom;
#[path = "shell_tests/content.rs"]
mod content;
#[path = "shell_tests/gamepad_focus_tests.rs"]
mod gamepad_focus_tests;
#[path = "shell_tests/keyboard_focus_tests.rs"]
mod keyboard_focus_tests;
#[path = "shell_tests/nav.rs"]
mod nav;
#[path = "shell_tests/picking.rs"]
mod picking;
#[path = "shell_tests/responsive.rs"]
mod responsive;
#[path = "shell_tests/screen_reader_bridge_tests.rs"]
mod screen_reader_bridge_tests;
#[path = "shell_tests/shell.rs"]
mod shell;
#[path = "shell_tests/sidebar.rs"]
mod sidebar;
#[path = "shell_tests/theme.rs"]
mod theme;
