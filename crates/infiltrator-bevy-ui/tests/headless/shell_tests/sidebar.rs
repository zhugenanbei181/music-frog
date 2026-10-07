//! Behavior cases for sidebar.
//! test-intent: behavior

use super::*;

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
        Vec::<ProxyMode>::new(),
        "an uncomposed shell cannot invent an observed mode"
    );
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
    assert_eq!(script_node.label(), Some("脚本"));
    assert!(
        subtree_has_text(world, children[1], "脚本"),
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
        subtree_has_text(world, profile_card, "未观测"),
        "an uncomposed shell cannot display a fabricated profile or quota"
    );
    assert!(!subtree_has_text(world, profile_card, "46.4 GB"));

    let mut matrix_query = world.query::<(Entity, &SidebarShortcutMatrix)>();
    let (matrix, _) = matrix_query
        .single(world)
        .expect("SidebarShortcutMatrix mounted");
    assert!(subtree_has_text(world, matrix, "未观测"));
    assert!(!subtree_has_text(world, matrix, "2842"));

    let mut speed_query = world.query::<(Entity, &SidebarSpeedFooter)>();
    let (speed_footer, _) = speed_query
        .single(world)
        .expect("SidebarSpeedFooter mounted");
    assert!(subtree_has_text(world, speed_footer, "未观测"));
    assert!(!subtree_has_text(world, speed_footer, "124.5 KB/s"));

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
