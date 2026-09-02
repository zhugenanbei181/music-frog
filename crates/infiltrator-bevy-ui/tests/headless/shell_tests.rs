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
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, ContentSlot, LayoutMode, ShellHeader,
    ShellLayoutState, ShellPlugin, ShellRoot, SidebarPanel, ThemeMode, ThemeToggle,
};
use infiltrator_bevy_ui::pages::overview::OverviewModePill;
use infiltrator_bevy_ui::projection::ProxyMode;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Breakpoint, LightDark, Theme};

#[test]
fn shell_mounts_camera_content_slot_and_stamped_header() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `spawn_scene` resolves bsn! scenes through the asset infrastructure
    // (AssetServer + Assets<ScenePatch>). These are the singleton plugins a
    // windowed run inherits from DefaultPlugins; headless tests add them
    // explicitly instead.
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

/// Guard against accidental duplicate shell mounts (routing remounts must
/// replace, not stack — BEVY-M2 leans on this invariant).
#[test]
fn shell_reruns_do_not_stack_slots() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `spawn_scene` resolves bsn! scenes through the asset infrastructure
    // (AssetServer + Assets<ScenePatch>). These are the singleton plugins a
    // windowed run inherits from DefaultPlugins; headless tests add them
    // explicitly instead.
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app.update();
    let world = app.world_mut();
    let mut slots = world.query::<&ContentSlot>();
    assert_eq!(slots.iter(world).count(), 1);
}

/// Compile-time declaration that ContentSlot is a plain marker component
/// usable by page routing (keeps the M2 seam honest without a page yet).
#[test]
fn content_slot_is_a_component_marker() {
    fn assert_component<T: Component>() {}
    assert_component::<ContentSlot>();
}

/// The headless composition under test: the real `ShellPlugin` on
/// `MinimalPlugins` plus the asset/scene singletons, mounted and settled.
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

/// M1 a11y: the shell seeds semantic nodes on the root (Window role, named),
/// the title row (Header role, named) and the theme pill (an explicitly
/// named Button role replacing the official widget's anonymous required
/// default). The components are inert data under `MinimalPlugins` — the
/// winit AccessKit bridge that publishes them exists only in windowed
/// compositions (bevy_winit mounts `AccessKitPlugin`), which is the honest
/// headless boundary.
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

/// M2 a11y: the sidebar's nav entries carry labeled Button semantics (the
/// two 未迁移 entries stamped disabled), the mode pills carry their mode
/// names, and the content slot reads as the named page region.
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
    assert_eq!(
        nav_labels,
        vec![
            ("核心概览".to_owned(), false),
            ("数据同步".to_owned(), true),
            ("系统设置".to_owned(), true),
        ],
        "every nav entry is a named button; the 未迁移 ones read disabled"
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

/// The sidebar mounts its chrome: the active nav item, the two honest
/// disabled nav items, and the three mode pills of the segment control.
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
    assert_eq!(idle, 2, "数据同步 and 系统设置 mount as idle items");

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

/// Whether any entity at or below `root` carries `T`.
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

/// Whether any text node at or below `root` spells exactly `needle`.
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

/// The reference's sidebar rhythm: identity block, mode segment, the nav
/// group in the content flow (one S16 row-gap step below the segment), a
/// flex spacer, and the version caption at the foot. Guards against the
/// nav group ever being pushed back to the rail's bottom edge.
#[test]
fn sidebar_orders_nav_into_the_content_flow_above_the_spacer() {
    let mut app = mounted_shell();
    let world = app.world_mut();
    let mut rails = world.query::<(Entity, &SidebarPanel)>();
    let (rail, _) = rails.single(world).expect("one sidebar rail");
    let children: Vec<Entity> = world.get::<Children>(rail).expect("rail children").to_vec();
    assert_eq!(
        children.len(),
        5,
        "identity, mode segment, nav, spacer, version expected"
    );

    assert!(
        subtree_contains::<NavItem>(world, children[2]),
        "the nav group sits directly after the mode segment (content flow)"
    );
    assert!(
        !subtree_contains::<NavItem>(world, children[4]),
        "the version foot carries no nav items"
    );

    let spacer = world.get::<Node>(children[3]).expect("spacer node");
    assert!(
        spacer.flex_grow > 0.0,
        "the gap between nav and version stays a flexible spacer"
    );
    assert!(
        subtree_has_text(world, children[4], "0.30 demo"),
        "the version caption closes the rail"
    );
}

/// M1 theme affordance: triggering `ThemeSwitch` re-resolves the palette and
/// restamps text ink and pill fill in place — the title text and pill
/// entities keep their ids across dark → light → dark (zero remounts).
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

/// Every token-filled sidebar surface (rail, nav items, mode pills) flips
/// with the theme and keeps its entity id — the reskin hard requirement
/// for the shell side of the tree.
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
    for (id, pill, visual) in pills.iter(world) {
        let fill = world.get::<BackgroundColor>(id).expect("pill survives");
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

/// The full affordance chain: an `Activate` on the theme pill flips the
/// shell-owned mode mirror and triggers `ThemeSwitch`; an `Activate` on any
/// other entity must not.
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

/// Responsive shell mounts both sidebar and bottom nav bar, defaulting to
/// desktop mode (width >= 600px) with sidebar visible and bottom nav hidden.
#[test]
fn responsive_shell_mounts_both_modes_and_defaults_to_sidebar() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let layout = world.resource::<ShellLayoutState>();
    assert_eq!(layout.breakpoint, Breakpoint::Desktop);
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

/// On mobile viewport width (<600px), responsive shell automatically collapses
/// the sidebar and switches to bottom navigation bar mode.
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
    assert_eq!(layout.breakpoint, Breakpoint::Mobile);
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

/// Transitioning from mobile (<600px) back to desktop (>=600px) restores the sidebar
/// and hides the bottom navigation bar.
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
    assert_eq!(layout.breakpoint, Breakpoint::Desktop);
    assert_eq!(layout.mode, LayoutMode::Sidebar);

    let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
    let (sidebar_node, _) = sidebars.single(world).expect("one sidebar rail");
    assert_eq!(sidebar_node.display, Display::Flex);

    let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
    let (bottom_node, _) = bottom_navs.single(world).expect("one bottom nav bar");
    assert_eq!(bottom_node.display, Display::None);
}

/// Responsive layout transitions preserve entity IDs (zero remounts, component restamp in place).
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

/// Theme flip repaints the bottom navigation bar and active items in place without respawn.
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

/// Bottom navigation bar carries accessible semantic nodes for all its items.
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
            ("核心概览".to_owned(), false),
            ("数据同步".to_owned(), true),
            ("系统设置".to_owned(), true),
        ],
        "bottom nav carries named button accessibility semantics matching sidebar"
    );
}
