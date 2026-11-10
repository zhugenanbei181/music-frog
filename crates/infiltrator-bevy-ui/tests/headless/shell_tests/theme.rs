//! Behavior cases for theme.
//! test-intent: behavior

use super::*;
use bevy::color::Color;

#[test]
fn theme_switch_restamps_ink_and_fill_in_place() {
    let mut app = mounted_shell();
    let header = heading_entity(app.world_mut());
    let rail = app
        .world_mut()
        .query::<(Entity, &SidebarPanel)>()
        .single(app.world())
        .expect("sidebar rail")
        .0;
    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
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
        .get(world, rail)
        .expect("rail survives the switch");
    assert_eq!(fill.0, light.sidebar, "rail fill restamped");
    assert!(world.get_entity(header).is_ok(), "header id unchanged");
    assert!(world.get_entity(rail).is_ok(), "rail id unchanged");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Dark));
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
        .get(world, rail)
        .expect("rail survives the round trip");
    assert_eq!(fill.0, dark.sidebar, "rail fill reverts");
    assert!(
        world.get_entity(header).is_ok(),
        "header id still unchanged"
    );
    assert!(world.get_entity(rail).is_ok(), "rail id still unchanged");
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
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();

    let mut rails = world.query::<(Entity, &SidebarPanel, &BackgroundColor)>();
    let (_, _, rail_fill) = rails.iter(world).next().expect("sidebar rail");
    assert_eq!(rail_fill.0, light.sidebar, "rail fill flipped to light");

    // The current destination is a wash over the column's ground and an idle
    // row paints nothing at all, so the column's material shows through. The
    // old expectation (accent on the active row, `surface_elevated` on the
    // rest) described the solid capsule the reference does not paint; the
    // widget layer's single writer now resolves `Token::NavSelectionFill`
    // against the flipped palette, so a skin change repaints the same rows.
    let mut items = world.query::<(Entity, &NavActive, &NavRowKind)>();
    let mut painted = 0;
    for (id, bit, kind) in items.iter(world) {
        assert_eq!(
            *kind,
            NavRowKind::CurrentDestination,
            "a sidebar row states where the user is, not a chosen member"
        );
        let fill = world.get::<BackgroundColor>(id).expect("nav row survives");
        assert_eq!(
            fill.0,
            if bit.0 {
                light.nav_selection_fill()
            } else {
                Color::NONE
            },
            "nav row fill follows the light tokens (wash on the current row, transparent elsewhere)"
        );
        painted += 1;
    }
    assert_eq!(painted, 7, "the rail's seven rows all repainted");

    // The routing selector is no longer a sidebar surface: the rail mounts
    // navigation alone, so the shell paints no mode pill and the dashboard's
    // segmented card owns that repainting (see the Overview theme cases).
    let pills = world
        .query::<&OverviewRoutingIntentSegmentPill>()
        .iter(world)
        .count();
    assert_eq!(pills, 0, "the shell carries no routing choice to repaint");

    let world = app.world_mut();
    let mut ids = world.query::<(Entity, &BackgroundColor)>();
    let mut after: Vec<Entity> = ids.iter(world).map(|(id, _)| id).collect();
    after.sort();
    assert_eq!(after, surface_ids, "the reskin is a restamp: zero remounts");
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
        .trigger(ThemeSwitch(ThemeSkin::Light));
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
