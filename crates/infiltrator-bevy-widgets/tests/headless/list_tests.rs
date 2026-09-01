//! Headless tests for the list: the virtual-window clamp math, the
//! scrollable column scene, in-place selection re-projection onto the nav
//! vocabulary, and theme reskin — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::list::{
    List, ListSelection, list_row_scene, list_scene, visible_window,
};
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, nav_fill};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn visible_window_covers_the_viewport_with_clamping() {
    // Everything fits.
    assert_eq!(visible_window(10, 300.0, 30.0, 0.0), (0, 10));
    // Partially visible rows count as visible.
    assert_eq!(visible_window(100, 250.0, 30.0, 0.0), (0, 9));
    // A scrolled offset moves the window.
    assert_eq!(visible_window(100, 300.0, 30.0, 450.0), (15, 25));
    // Scrolling past the end pins to the last full window.
    assert_eq!(visible_window(100, 300.0, 30.0, 100_000.0), (90, 100));
    // A negative offset pins to the first window.
    assert_eq!(visible_window(100, 300.0, 30.0, -50.0), (0, 10));
    // Degenerate inputs stay total.
    assert_eq!(visible_window(0, 300.0, 30.0, 0.0), (0, 0));
    assert_eq!(visible_window(10, 0.0, 30.0, 0.0), (0, 0));
    assert_eq!(visible_window(10, 300.0, 0.0, 0.0), (0, 0));
    assert_eq!(
        visible_window(3, 300.0, 30.0, 0.0),
        (0, 3),
        "never out of bounds"
    );
}

#[test]
fn list_scene_mounts_rows_and_selection_flip_reprojects_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let rows = (0..4)
                .map(|index| list_row_scene(format!("proxy {index}"), index == 1, &palette))
                .collect();
            commands.spawn_scene(list_scene(rows, Some(1), &palette));
        },
    );
    app.update();

    let row_ids = |world: &mut bevy::ecs::world::World| -> Vec<Entity> {
        let mut lists = world.query::<(Entity, &List, &Children)>();
        let (_, _, children) = lists.iter(world).next().expect("one list");
        children.iter().copied().collect()
    };
    let ids = row_ids(app.world_mut());
    assert_eq!(ids.len(), 4);

    let palette = UiPalette::new(&Theme::dark());
    let active_of = |world: &bevy::ecs::world::World, entity: Entity| {
        world.get::<NavActive>(entity).expect("row active bit").0
    };
    let fill_of = |world: &bevy::ecs::world::World, entity: Entity| {
        world.get::<BackgroundColor>(entity).expect("row fill").0
    };
    assert!(active_of(app.world_mut(), ids[1]), "row 1 selected");
    assert_eq!(fill_of(app.world_mut(), ids[1]), nav_fill(true, &palette));

    // Flip the selection to row 3: bits restamp, ids stay, paints follow.
    let world = app.world_mut();
    let mut lists = world.query::<(Entity, &ListSelection)>();
    let (list_id, _) = lists.iter(world).next().expect("one list");
    app.world_mut()
        .entity_mut(list_id)
        .insert(ListSelection(Some(3)));
    app.update();

    let world = app.world_mut();
    assert!(!active_of(world, ids[1]), "row 1 deselected in place");
    assert!(active_of(world, ids[3]), "row 3 selected in place");
    assert_eq!(fill_of(world, ids[3]), nav_fill(true, &palette));
    assert!(
        world.get::<NavItem>(ids[1]).is_some() && world.get::<NavItem>(ids[3]).is_some(),
        "the rows kept their entity ids across the flip"
    );
}

#[test]
fn theme_flip_repaints_the_list_and_rows_without_respawn() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let rows = (0..3)
                .map(|index| list_row_scene(format!("proxy {index}"), index == 0, &palette))
                .collect();
            commands.spawn_scene(list_scene(rows, Some(0), &palette));
        },
    );
    app.update();

    let ids: Vec<Entity> = {
        let world = app.world_mut();
        let mut lists = world.query::<(Entity, &List, &Children)>();
        let (_, _, children) = lists.iter(world).next().expect("one list");
        children.iter().copied().collect()
    };

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    assert_eq!(
        world
            .get::<BackgroundColor>(ids[0])
            .expect("row survives")
            .0,
        nav_fill(true, &light),
        "the selected row re-derives from the light accent"
    );
    let mut lists = world.query::<&List>();
    assert_eq!(lists.iter(world).count(), 1, "the list itself survives");
}
