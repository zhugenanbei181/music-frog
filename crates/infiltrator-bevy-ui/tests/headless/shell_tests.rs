//! Headless shell tests: the real `ShellPlugin` on `MinimalPlugins` — no
//! window, no render hardware. Asserts the sidebar/content shell mounts,
//! typography is stamped by the widget layer's observer, the AccessKit
//! semantic seeds land with the right roles, the theme affordance restamps
//! the mounted tree in place (zero remounts), and the theme flip repaints
//! every token-filled sidebar surface (rail, nav items, mode pills)
//! without changing a single entity id.

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
use bevy::ui::prelude::px;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, ContentSlot, ContentTitleLabel, DensityToggle,
    GlobalModeCapsule, GlobalStatusDot, HistoryBackButton, HistoryForwardButton, LayoutMode,
    ShellHeader, ShellLayoutState, ShellPlugin, ShellRoot, SidebarActiveProfileCard,
    SidebarNavItem, SidebarPanel, SidebarScriptModePill, SidebarShortcutMatrix,
    SidebarShortcutTile, SidebarSpeedFooter, SidebarSystemProxyCard, SidebarSystemProxyToggle,
    SidebarTunCard, SidebarTunToggle, ThemeToggle,
};
use infiltrator_bevy_ui::appearance::ThemeMode;
use infiltrator_bevy_ui::pages::overview::OverviewModePill;
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{Density, ResponsiveContext};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Breakpoint, Theme, ThemeSkin};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::theme::ThemePreference;

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
        .find(|(_, role)| role.0 == Role::Heading)
        .expect("title row heading text mounted")
        .0
}

fn theme_pill_entity(world: &mut World) -> Entity {
    let mut pills = world.query::<(Entity, &ThemeToggle)>();
    pills.single(world).expect("exactly one theme pill").0
}

fn density_pill_entity(world: &mut World) -> Entity {
    let mut pills = world.query::<(Entity, &DensityToggle)>();
    pills.single(world).expect("exactly one density pill").0
}

fn subtree_contains<T: Component>(world: &World, root: Entity) -> bool {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if world.get::<T>(entity).is_some() {
            return true;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    false
}

fn subtree_has_text(world: &World, root: Entity, needle: &str) -> bool {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if world
            .get::<Text>(entity)
            .is_some_and(|text| text.0 == needle)
        {
            return true;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    false
}

#[path = "shell_tests/activating.rs"]
mod activating;
#[path = "shell_tests/bottom.rs"]
mod bottom;
#[path = "shell_tests/content.rs"]
mod content;
#[path = "shell_tests/nav.rs"]
mod nav;
#[path = "shell_tests/responsive.rs"]
mod responsive;
#[path = "shell_tests/shell.rs"]
mod shell;
#[path = "shell_tests/sidebar.rs"]
mod sidebar;
#[path = "shell_tests/theme.rs"]
mod theme;
