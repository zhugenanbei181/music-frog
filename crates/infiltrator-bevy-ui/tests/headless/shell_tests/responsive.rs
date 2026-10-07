//! Behavior cases for responsive.
//! test-intent: behavior

use super::*;

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
        .set_width(1000.0);
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

    // 2. Medium: 768px -> Rail (64px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(768.0);
    app.update();
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let (node, _) = sidebars.single(world).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.width, px(64.0));
    }

    // 3. Expanded: 1000px -> Sidebar (240px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1000.0);
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
