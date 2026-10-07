//! Behavior cases for theme.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant, button_fill};

#[test]
fn theme_switch_restamps_ink_and_fill_in_place() {
    let mut app = mounted_shell();
    let header = heading_entity(app.world_mut());
    let pill = theme_pill_entity(app.world_mut());
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
        .get(world, pill)
        .expect("pill survives the switch");
    assert_eq!(fill.0, light.surface_elevated, "pill fill restamped");
    assert!(world.get_entity(header).is_ok(), "header id unchanged");
    assert!(world.get_entity(pill).is_ok(), "pill id unchanged");

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
        .trigger(ThemeSwitch(ThemeSkin::Light));
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
            button_fill(
                ButtonVariant::Default,
                visual.0,
                false,
                false,
                world
                    .get::<ButtonDisabled>(entity)
                    .is_some_and(|disabled| disabled.0),
                &light
            ),
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
