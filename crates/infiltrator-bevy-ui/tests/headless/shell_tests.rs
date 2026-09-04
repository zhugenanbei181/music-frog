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
    SidebarTunCard, SidebarTunToggle, ThemeMode, ThemeToggle,
};
use infiltrator_bevy_ui::pages::overview::OverviewModePill;
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{Density, ResponsiveContext};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Breakpoint, LightDark, Theme};
use infiltrator_contract::command::ProxyMode;

#[test]
fn shell_mounts_camera_content_slot_and_stamped_header() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();

    let world = app.world_mut();
    let mut cameras = world.query::<&Camera2d>();
    assert_eq!(cameras.iter(world).count(), 1, "ui camera mounted");

    let world = app.world_mut();
    let mut slots = world.query::<&ContentSlot>();
    assert_eq!(slots.iter(world).count(), 1, "exactly one content slot");

    let world = app.world_mut();
    let mut rails = world.query::<&SidebarPanel>();
    assert_eq!(rails.iter(world).count(), 1, "exactly one sidebar rail");

    let world = app.world_mut();
    let mut headers = world.query::<(&Text, &TextRole, &TextColor, &TextFont)>();
    let (_, role, ink, font) = headers
        .iter(world)
        .find(|(_, role, _, _)| role.0 == Role::Heading)
        .expect("title row carries the heading role");
    assert_eq!(role.0, Role::Heading);
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(ink.0, palette.ink, "heading ink stamped from tokens");
    assert!(
        matches!(font.font_size, FontSize::Px(size) if size == palette.heading_font_px),
        "heading size stamped from the type scale"
    );
}

#[test]
fn shell_reruns_do_not_stack_slots() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app.update();
    let world = app.world_mut();
    let mut slots = world.query::<&ContentSlot>();
    assert_eq!(slots.iter(world).count(), 1);
}

#[test]
fn content_slot_is_a_component_marker() {
    fn assert_component<T: Component>() {}
    assert_component::<ContentSlot>();
}

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

#[test]
fn shell_exposes_named_semantic_nodes_on_root_header_and_pill() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut roots = world.query::<(&AccessibilityNode, &ShellRoot)>();
    let (root, _) = roots.single(world).expect("shell root semantic node");
    assert_eq!(root.role(), accesskit::Role::Window);
    assert_eq!(root.label(), Some("MusicFrog Infiltrator"));

    let mut headers = world.query::<(&AccessibilityNode, &ShellHeader)>();
    let (header, _) = headers.single(world).expect("header semantic node");
    assert_eq!(header.role(), accesskit::Role::Header);
    assert_eq!(header.label(), Some("MusicFrog Infiltrator"));

    let mut pills = world.query::<(&AccessibilityNode, &ThemeToggle)>();
    let (pill, _) = pills.single(world).expect("pill semantic node");
    assert_eq!(pill.role(), accesskit::Role::Button);
    assert_eq!(pill.label(), Some("Toggle color theme"));
}

#[test]
fn nav_entries_mode_pills_and_content_region_carry_semantics() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut navs = world.query::<(&NavItem, &AccessibilityNode)>();
    let mut nav_labels: Vec<(String, bool)> = Vec::new();
    for (_, node) in navs.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        nav_labels.push((
            node.label().expect("nav label").to_owned(),
            node.is_disabled(),
        ));
    }
    let expected_nav_labels: Vec<(String, bool)> = Route::ALL
        .iter()
        .map(|r| (r.label().to_owned(), false))
        .collect();
    assert_eq!(
        nav_labels, expected_nav_labels,
        "every nav entry is a named button; all 11 routes are enabled"
    );

    let mut pills = world.query::<(&OverviewModePill, &AccessibilityNode)>();
    let mut pill_labels: Vec<(ProxyMode, String)> = Vec::new();
    for (pill, node) in pills.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        pill_labels.push((pill.0, node.label().expect("mode label").to_owned()));
    }
    assert_eq!(
        pill_labels,
        vec![
            (ProxyMode::Rule, "规则模式".to_owned()),
            (ProxyMode::Global, "全局模式".to_owned()),
            (ProxyMode::Direct, "直连模式".to_owned()),
        ],
        "every mode pill carries its mode name"
    );

    let mut slots = world.query::<(&ContentSlot, &AccessibilityNode)>();
    let (_, region) = slots.single(world).expect("content region semantic node");
    assert_eq!(region.role(), accesskit::Role::Region);
    assert_eq!(region.label(), Some("核心概览"));
}

#[test]
fn sidebar_mounts_nav_and_mode_segment() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut items = world.query::<(&NavItem, &NavActive)>();
    let mut active = 0;
    let mut idle = 0;
    for (_, bit) in items.iter(world) {
        if bit.0 {
            active += 1;
        } else {
            idle += 1;
        }
    }
    assert_eq!(active, 1, "exactly one active nav item (核心概览)");
    assert_eq!(
        idle, 10,
        "the remaining 10 routes in Route::ALL mount as idle items"
    );

    let mut pills = world.query::<(&OverviewModePill, &ControlVisual)>();
    let mut selected = Vec::new();
    for (pill, visual) in pills.iter(world) {
        if visual.0 {
            selected.push(pill.0);
        }
    }
    assert_eq!(
        selected,
        vec![ProxyMode::Rule],
        "the segment control preselects the default projection mode"
    );
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

#[test]
fn sidebar_orders_nav_into_the_content_flow_above_the_spacer() {
    let mut app = mounted_shell();
    let world = app.world_mut();
    let mut rails = world.query::<(Entity, &SidebarPanel)>();
    let (rail, _) = rails.single(world).expect("one sidebar rail");
    let children: Vec<Entity> = world.get::<Children>(rail).expect("rail children").to_vec();
    assert_eq!(
        children.len(),
        9,
        "identity, mode segment, system toggles, active profile, shortcut matrix, speed footer, nav, spacer, version expected"
    );

    assert!(
        subtree_contains::<NavItem>(world, children[6]),
        "the nav group sits directly in content flow above the spacer"
    );
    assert!(
        !subtree_contains::<NavItem>(world, children[8]),
        "the version foot carries no nav items"
    );

    let spacer = world.get::<Node>(children[7]).expect("spacer node");
    assert!(
        spacer.flex_grow > 0.0,
        "the gap between nav and version stays a flexible spacer"
    );
    assert!(
        subtree_has_text(world, children[8], "0.30 demo"),
        "the version caption closes the rail"
    );
}

#[test]
fn theme_switch_restamps_ink_and_fill_in_place() {
    let mut app = mounted_shell();
    let header = heading_entity(app.world_mut());
    let pill = theme_pill_entity(app.world_mut());
    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();
    let world = app.world_mut();

    assert_eq!(world.resource::<UiPalette>(), &light, "palette re-resolved");
    let (_, ink) = world
        .query::<(&TextRole, &TextColor)>()
        .get(world, header)
        .expect("title text survives the switch");
    assert_eq!(ink.0, light.ink, "heading ink restamped to light");
    let fill = world
        .query::<&BackgroundColor>()
        .get(world, pill)
        .expect("pill survives the switch");
    assert_eq!(fill.0, light.surface_elevated, "pill fill restamped");
    assert!(world.get_entity(header).is_ok(), "header id unchanged");
    assert!(world.get_entity(pill).is_ok(), "pill id unchanged");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Dark));
    app.update();
    let world = app.world_mut();

    assert_eq!(world.resource::<UiPalette>(), &dark, "palette reverts");
    let (_, ink) = world
        .query::<(&TextRole, &TextColor)>()
        .get(world, header)
        .expect("title text survives the round trip");
    assert_eq!(ink.0, dark.ink, "heading ink restamped back to dark");
    let fill = world
        .query::<&BackgroundColor>()
        .get(world, pill)
        .expect("pill survives the round trip");
    assert_eq!(fill.0, dark.surface_elevated, "pill fill reverts");
    assert!(
        world.get_entity(header).is_ok(),
        "header id still unchanged"
    );
    assert!(world.get_entity(pill).is_ok(), "pill id still unchanged");
}

#[test]
fn theme_flip_repaints_every_sidebar_surface_in_place() {
    let mut app = mounted_shell();
    let world = app.world_mut();
    let mut ids = world.query::<(Entity, &BackgroundColor)>();
    let mut surface_ids: Vec<Entity> = ids.iter(world).map(|(id, _)| id).collect();
    surface_ids.sort();

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();

    let mut rails = world.query::<(Entity, &SidebarPanel, &BackgroundColor)>();
    let (_, _, rail_fill) = rails.iter(world).next().expect("sidebar rail");
    assert_eq!(rail_fill.0, light.sidebar, "rail fill flipped to light");

    let mut items = world.query::<(Entity, &NavActive)>();
    for (id, bit) in items.iter(world) {
        let fill = world.get::<BackgroundColor>(id).expect("nav item survives");
        assert_eq!(
            fill.0,
            if bit.0 {
                light.accent
            } else {
                light.surface_elevated
            },
            "nav item fill follows the light tokens"
        );
    }

    let mut pills = world.query::<(Entity, &OverviewModePill, &ControlVisual)>();
    for (entity, pill, visual) in pills.iter(world) {
        let fill = world.get::<BackgroundColor>(entity).expect("pill survives");
        assert_eq!(
            fill.0,
            if visual.0 {
                light.accent
            } else {
                light.surface_elevated
            },
            "mode pill {pill:?} fill follows the light tokens"
        );
    }

    let world = app.world_mut();
    let mut ids = world.query::<(Entity, &BackgroundColor)>();
    let mut after: Vec<Entity> = ids.iter(world).map(|(id, _)| id).collect();
    after.sort();
    assert_eq!(after, surface_ids, "the reskin is a restamp: zero remounts");
}

#[test]
fn activating_the_pill_flips_the_mode_mirror() {
    let mut app = mounted_shell();
    let pill = theme_pill_entity(app.world_mut());
    let slot = {
        let world = app.world_mut();
        let mut slots = world.query::<(Entity, &ContentSlot)>();
        slots.single(world).expect("content slot").0
    };
    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());
    assert_eq!(app.world().resource::<ThemeMode>().0, LightDark::Dark);

    // A non-pill activation is a no-op for the mode.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: slot });
    app.update();
    assert_eq!(app.world().resource::<ThemeMode>().0, LightDark::Dark);
    assert_eq!(app.world().resource::<UiPalette>(), &dark);

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();
    assert_eq!(app.world().resource::<ThemeMode>().0, LightDark::Light);
    assert_eq!(app.world().resource::<UiPalette>(), &light);

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();
    assert_eq!(app.world().resource::<ThemeMode>().0, LightDark::Dark);
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &dark,
        "palette is Color-exact through the round trip"
    );
}

#[test]
fn activating_density_pill_toggles_density() {
    let mut app = mounted_shell();
    let density_pill = density_pill_entity(app.world_mut());

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Comfortable
    );

    app.world_mut().commands().trigger(Activate {
        entity: density_pill,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Compact
    );
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Compact
    );

    app.world_mut().commands().trigger(Activate {
        entity: density_pill,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Comfortable
    );
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Comfortable
    );
}

#[test]
fn responsive_shell_mounts_both_modes_and_defaults_to_sidebar() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let layout = world.resource::<ShellLayoutState>();
    assert_eq!(layout.breakpoint, Breakpoint::Expanded);
    assert_eq!(layout.mode, LayoutMode::Sidebar);
    assert!(layout.width_px >= 600.0);

    let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
    let (sidebar_node, _) = sidebars.single(world).expect("one sidebar rail");
    assert_eq!(
        sidebar_node.display,
        Display::Flex,
        "sidebar is visible in desktop mode"
    );

    let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
    let (bottom_node, _) = bottom_navs.single(world).expect("one bottom nav bar");
    assert_eq!(
        bottom_node.display,
        Display::None,
        "bottom nav is collapsed/hidden in desktop mode"
    );
}

#[test]
fn responsive_shell_switches_to_bottom_nav_on_mobile_width() {
    let mut app = mounted_shell();

    // Resize viewport to mobile width (375px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(375.0);
    app.update();

    let world = app.world_mut();
    let layout = world.resource::<ShellLayoutState>();
    assert_eq!(layout.breakpoint, Breakpoint::Compact);
    assert_eq!(layout.mode, LayoutMode::BottomNav);
    assert!(layout.breakpoint.is_compact());

    let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
    let (sidebar_node, _) = sidebars.single(world).expect("one sidebar rail");
    assert_eq!(
        sidebar_node.display,
        Display::None,
        "sidebar is collapsed on mobile"
    );

    let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
    let (bottom_node, _) = bottom_navs.single(world).expect("one bottom nav bar");
    assert_eq!(
        bottom_node.display,
        Display::Flex,
        "bottom nav is visible on mobile"
    );
}

#[test]
fn responsive_shell_switches_back_to_sidebar_on_desktop_width() {
    let mut app = mounted_shell();

    // Switch to mobile
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(400.0);
    app.update();
    assert_eq!(
        app.world().resource::<ShellLayoutState>().mode,
        LayoutMode::BottomNav
    );

    // Switch back to desktop
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1280.0);
    app.update();

    let world = app.world_mut();
    let layout = world.resource::<ShellLayoutState>();
    assert_eq!(layout.breakpoint, Breakpoint::Expanded);
    assert_eq!(layout.mode, LayoutMode::Sidebar);

    let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
    let (sidebar_node, _) = sidebars.single(world).expect("one sidebar rail");
    assert_eq!(sidebar_node.display, Display::Flex);

    let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
    let (bottom_node, _) = bottom_navs.single(world).expect("one bottom nav bar");
    assert_eq!(bottom_node.display, Display::None);
}

#[test]
fn responsive_four_tier_sidebar_morphology() {
    let mut app = mounted_shell();

    // 1. Compact: 375px -> BottomNav
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(375.0);
    app.update();
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        assert_eq!(sidebars.single(world).unwrap().0.display, Display::None);
    }

    // 2. Medium: 768px -> Rail (72px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(768.0);
    app.update();
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let (node, _) = sidebars.single(world).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.width, px(72.0));
    }

    // 3. Expanded: 1280px -> Sidebar (240px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1280.0);
    app.update();
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let (node, _) = sidebars.single(world).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.width, px(240.0));
    }

    // 4. Ultra: 1920px -> Wide (280px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1920.0);
    app.update();
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let (node, _) = sidebars.single(world).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.width, px(280.0));
    }
}

#[test]
fn responsive_mode_switch_keeps_entity_identities() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let sidebar_id = world
        .query::<(Entity, &SidebarPanel)>()
        .single(world)
        .expect("sidebar entity")
        .0;
    let bottom_nav_id = world
        .query::<(Entity, &BottomNavBar)>()
        .single(world)
        .expect("bottom nav entity")
        .0;

    // Desktop -> Mobile -> Tablet -> Desktop
    for w in [390.0, 768.0, 1440.0, 320.0] {
        app.world_mut()
            .resource_mut::<ShellLayoutState>()
            .set_width(w);
        app.update();

        let world = app.world_mut();
        assert!(
            world.get_entity(sidebar_id).is_ok(),
            "sidebar entity id preserved across width {w}"
        );
        assert!(
            world.get_entity(bottom_nav_id).is_ok(),
            "bottom nav entity id preserved across width {w}"
        );
    }
}

#[test]
fn theme_flip_repaints_bottom_nav_bar_in_place() {
    let mut app = mounted_shell();

    // Switch to mobile mode
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(375.0);
    app.update();

    let world = app.world_mut();
    let bar_id = world
        .query::<(Entity, &BottomNavBar)>()
        .single(world)
        .expect("bottom nav")
        .0;

    // Flip to light theme
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let bar_fill = world
        .get::<BackgroundColor>(bar_id)
        .expect("bottom nav fill survives");
    assert_eq!(
        bar_fill.0, light.sidebar,
        "bottom nav bar background matches light sidebar"
    );

    // Active item icon tint matches light accent
    let mut items = world.query::<(&BottomNavItem, &BottomNavActive, &Children)>();
    let mut icons = world.query::<&IconTint>();
    for (_, active, children) in items.iter(world) {
        if active.0 {
            for child in children.iter() {
                if let Ok(tint) = icons.get(world, *child) {
                    assert_eq!(
                        tint.0, light.accent,
                        "active bottom nav item icon tinted with light accent"
                    );
                }
            }
        }
    }
}

#[test]
fn bottom_nav_carries_named_button_semantics() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut items = world.query::<(&BottomNavItem, &AccessibilityNode)>();
    let mut labels: Vec<(String, bool)> = Vec::new();
    for (_, node) in items.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Button);
        labels.push((
            node.label().expect("bottom nav label").to_owned(),
            node.is_disabled(),
        ));
    }
    assert_eq!(
        labels,
        vec![
            (Route::Overview.label().to_owned(), false),
            (Route::Proxies.label().to_owned(), false),
            (Route::Profiles.label().to_owned(), false),
            (Route::Settings.label().to_owned(), false),
        ],
        "bottom nav carries 4 clean button semantics matching the 4 key routes"
    );
}

#[test]
fn content_title_syncs_with_active_route() {
    let mut app = mounted_shell();

    // Default title is "核心概览"
    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "核心概览");
    }

    // Set active route to Proxies
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Proxies)));
    app.update();

    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "代理策略");
    }

    // Set active route to Settings
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Settings)));
    app.update();

    {
        let world = app.world_mut();
        let mut titles = world.query::<(&Text, &ContentTitleLabel)>();
        let (text, _) = titles.single(world).expect("title text");
        assert_eq!(text.0, "系统设置");
    }
}

#[test]
fn sidebar_nav_click_triggers_route_change_and_updates_visuals() {
    let mut app = mounted_shell();
    let dark = UiPalette::new(&Theme::dark());

    // Initially Overview is active
    {
        let world = app.world_mut();
        let mut items = world.query::<(&SidebarNavItem, &NavActive, &BackgroundColor)>();
        for (item, active, bg) in items.iter(world) {
            if item.0 == Route::Overview {
                assert!(active.0);
                assert_eq!(bg.0, dark.accent);
            } else {
                assert!(!active.0);
                assert_eq!(bg.0, dark.surface_elevated);
            }
        }
    }

    // Find the Proxies nav item entity and activate it
    let proxies_entity = {
        let world = app.world_mut();
        let mut items = world.query::<(Entity, &SidebarNavItem)>();
        items
            .iter(world)
            .find(|(_, item)| item.0 == Route::Proxies)
            .expect("proxies nav item")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: proxies_entity,
    });
    // Simulate router setting active route on RouteChanged
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Proxies)));
    app.update();

    // Now Proxies is active and Overview is idle
    {
        let world = app.world_mut();
        let mut items = world.query::<(&SidebarNavItem, &NavActive, &BackgroundColor)>();
        for (item, active, bg) in items.iter(world) {
            if item.0 == Route::Proxies {
                assert!(active.0, "Proxies should be active");
                assert_eq!(bg.0, dark.accent);
            } else {
                assert!(!active.0, "{:?} should be idle", item.0);
                assert_eq!(bg.0, dark.surface_elevated);
            }
        }
    }
}

#[test]
fn bottom_nav_renders_four_items_and_click_activates() {
    let mut app = mounted_shell();

    // 4 items exist
    {
        let world = app.world_mut();
        let mut items = world.query::<&BottomNavItem>();
        let routes: Vec<Route> = items.iter(world).map(|i| i.0).collect();
        assert_eq!(
            routes,
            vec![
                Route::Overview,
                Route::Proxies,
                Route::Profiles,
                Route::Settings
            ]
        );
    }

    // Activate Profiles
    let profiles_entity = {
        let world = app.world_mut();
        let mut items = world.query::<(Entity, &BottomNavItem)>();
        items
            .iter(world)
            .find(|(_, item)| item.0 == Route::Profiles)
            .expect("profiles bottom nav item")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: profiles_entity,
    });
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Profiles)));
    app.update();

    // Active marker updated
    {
        let world = app.world_mut();
        let mut items = world.query::<(&BottomNavItem, &BottomNavActive)>();
        for (item, active) in items.iter(world) {
            if item.0 == Route::Profiles {
                assert!(active.0);
            } else {
                assert!(!active.0);
            }
        }
    }
}

#[test]
fn test_shell_header_history_and_status_indicators() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut back_query = world.query::<(Entity, &HistoryBackButton)>();
    let (back_entity, _) = back_query
        .iter(world)
        .next()
        .expect("HistoryBackButton must be mounted in header");

    let mut forward_query = world.query::<(Entity, &HistoryForwardButton)>();
    let (forward_entity, _) = forward_query
        .iter(world)
        .next()
        .expect("HistoryForwardButton must be mounted in header");

    let mut dot_query = world.query::<(Entity, &GlobalStatusDot)>();
    assert!(
        dot_query.iter(world).next().is_some(),
        "GlobalStatusDot must be mounted"
    );

    let mut mode_capsule_query = world.query::<(Entity, &GlobalModeCapsule)>();
    assert!(
        mode_capsule_query.iter(world).next().is_some(),
        "GlobalModeCapsule must be mounted"
    );

    // Trigger back button activation
    world.commands().trigger(Activate {
        entity: back_entity,
    });
    app.update();

    // Trigger forward button activation
    app.world_mut().commands().trigger(Activate {
        entity: forward_entity,
    });
    app.update();
}

#[test]
fn test_sidebar_modern_control_center_parity() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut rails = world.query::<(Entity, &SidebarPanel)>();
    let (rail, _) = rails.single(world).expect("one sidebar rail");
    let children: Vec<Entity> = world.get::<Children>(rail).expect("rail children").to_vec();

    // 1. Header row: logo + MusicFrog + v0.20.0
    assert!(
        subtree_has_text(world, children[0], "MusicFrog"),
        "sidebar identity header contains MusicFrog"
    );
    assert!(
        subtree_has_text(world, children[0], "v0.20.0"),
        "sidebar identity header contains v0.20.0"
    );

    // 2. Proxy Mode Segmented Control: Script segment alongside Rule, Global, Direct
    let mut script_pills = world.query::<(&SidebarScriptModePill, &AccessibilityNode)>();
    let (_, script_node) = script_pills
        .iter(world)
        .next()
        .expect("SidebarScriptModePill mounted");
    assert_eq!(script_node.role(), accesskit::Role::Button);
    assert_eq!(script_node.label(), Some("脚本模式"));
    assert!(
        subtree_has_text(world, children[1], "脚本模式"),
        "mode segment contains 脚本模式"
    );

    // 3. Double System Toggle Cards
    let mut proxy_cards = world.query::<(Entity, &SidebarSystemProxyCard)>();
    let (proxy_card, _) = proxy_cards
        .iter(world)
        .next()
        .expect("SidebarSystemProxyCard mounted");
    assert!(
        subtree_has_text(world, proxy_card, "系统代理"),
        "proxy card contains label 系统代理"
    );
    let mut proxy_toggles = world.query::<(Entity, &SidebarSystemProxyToggle)>();
    assert!(proxy_toggles.iter(world).next().is_some());

    let mut tun_cards = world.query::<(Entity, &SidebarTunCard)>();
    let (tun_card, _) = tun_cards
        .iter(world)
        .next()
        .expect("SidebarTunCard mounted");
    assert!(
        subtree_has_text(world, tun_card, "TUN 模式"),
        "tun card contains label TUN 模式"
    );
    let mut tun_toggles = world.query::<(Entity, &SidebarTunToggle)>();
    assert!(tun_toggles.iter(world).next().is_some());

    // 4. Active Profile Card
    let mut profile_cards = world.query::<(Entity, &SidebarActiveProfileCard)>();
    let (profile_card, _) = profile_cards
        .iter(world)
        .next()
        .expect("SidebarActiveProfileCard mounted");
    assert!(
        subtree_has_text(world, profile_card, "Default Profile"),
        "profile card displays subscription name"
    );
    assert!(
        subtree_has_text(world, profile_card, "46.4 GB / 186.2 GB"),
        "profile card displays usage progress line"
    );
    assert!(
        subtree_has_text(world, profile_card, "25%"),
        "profile card displays usage percentage"
    );

    // 5. 2x2 Shortcut Grid Matrix
    let mut matrix_query = world.query::<(Entity, &SidebarShortcutMatrix)>();
    let (matrix, _) = matrix_query
        .iter(world)
        .next()
        .expect("SidebarShortcutMatrix mounted");
    assert!(
        subtree_has_text(world, matrix, "代理策略 (8)"),
        "shortcut tile 代理策略 (8)"
    );
    assert!(
        subtree_has_text(world, matrix, "分流规则 (2842)"),
        "shortcut tile 分流规则 (2842)"
    );
    assert!(
        subtree_has_text(world, matrix, "连接审计 (12)"),
        "shortcut tile 连接审计 (12)"
    );
    assert!(
        subtree_has_text(world, matrix, "域名解析 (4)"),
        "shortcut tile 域名解析 (4)"
    );

    // 6. Live Speed Footer
    let mut speed_query = world.query::<(Entity, &SidebarSpeedFooter)>();
    let (speed_footer, _) = speed_query
        .iter(world)
        .next()
        .expect("SidebarSpeedFooter mounted");
    assert!(
        subtree_has_text(world, speed_footer, "↑ 124.5 KB/s"),
        "speed footer displays upload rate"
    );
    assert!(
        subtree_has_text(world, speed_footer, "↓ 1.8 MB/s"),
        "speed footer displays download rate"
    );

    // 7. Shortcut tile clicking activates route change
    let mut shortcut_tiles = world.query::<(Entity, &SidebarShortcutTile)>();
    let (proxies_tile_entity, _) = shortcut_tiles
        .iter(world)
        .find(|(_, tile)| tile.0 == Route::Proxies)
        .expect("Proxies shortcut tile entity");
    app.world_mut().commands().trigger(Activate {
        entity: proxies_tile_entity,
    });
    app.world_mut()
        .insert_resource(ActiveRoute(Some(Route::Proxies)));
    app.update();
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Proxies),
        "clicking shortcut tile navigates to Route::Proxies"
    );
}
