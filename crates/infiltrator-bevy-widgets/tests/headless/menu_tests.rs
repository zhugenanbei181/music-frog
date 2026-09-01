//! Headless tests for the menu: the cyclic pure-core navigation, the
//! typed-event seam (MenuNavEvent in, MenuOutcome out), the overlay scene's
//! token layers, and in-place repaint on highlight moves and theme flips —
//! entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::menu::{
    Menu, MenuEntry, MenuNav, MenuNavEvent, MenuOutcome, MenuPanel, MenuRowIndex, MenuScrim,
    MenuState, menu_overlay_scene, menu_row_fill,
};
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

fn demo_entries() -> Vec<MenuEntry> {
    vec![
        MenuEntry::item("switch mode"),
        MenuEntry::Separator,
        MenuEntry::disabled("restart core"),
        MenuEntry::item("open logs"),
    ]
}

#[test]
fn menu_state_cycles_skipping_separators() {
    let mut state = MenuState::new(demo_entries());
    assert_eq!(state.highlight(), 0, "opens on the first item");

    // Down: separator skipped, the disabled item holds the cursor...
    state.advance(MenuNav::Down);
    assert_eq!(state.highlight(), 2);
    // ...then the second enabled item...
    state.advance(MenuNav::Down);
    assert_eq!(state.highlight(), 3);
    // ...and Down wraps to the first item (cyclic).
    state.advance(MenuNav::Down);
    assert_eq!(state.highlight(), 0);
    // Up from the first item wraps to the last.
    state.advance(MenuNav::Up);
    assert_eq!(state.highlight(), 3);
}

#[test]
fn confirm_authorizes_only_enabled_items_and_cancel_closes() {
    let mut state = MenuState::new(demo_entries());
    assert_eq!(
        state.advance(MenuNav::Confirm),
        Some(MenuOutcome::Confirmed(0))
    );

    state.advance(MenuNav::Down);
    assert_eq!(state.highlight(), 2, "cursor rests on the disabled item");
    assert_eq!(
        state.advance(MenuNav::Confirm),
        None,
        "a disabled entry is never authorized"
    );

    assert_eq!(state.advance(MenuNav::Cancel), Some(MenuOutcome::Canceled));
}

#[test]
fn an_empty_menu_absorbs_every_input() {
    let mut state = MenuState::new(Vec::new());
    for nav in [MenuNav::Up, MenuNav::Down, MenuNav::Confirm] {
        assert_eq!(state.advance(nav), None, "{nav:?} is a no-op on nothing");
        assert_eq!(state.highlight(), 0);
    }
    assert_eq!(state.advance(MenuNav::Cancel), Some(MenuOutcome::Canceled));
}

fn panel_id(world: &mut bevy::ecs::world::World) -> Entity {
    let mut panels = world.query::<(Entity, &MenuPanel)>();
    panels.iter(world).next().expect("one menu panel").0
}

/// Fill of the row with the given entry index.
fn row_fill(world: &mut bevy::ecs::world::World, index: usize) -> BackgroundColor {
    let mut rows = world.query::<(&MenuRowIndex, &BackgroundColor)>();
    rows.iter(world)
        .find(|(row, _)| row.0 == index)
        .map(|(_, fill)| *fill)
        .expect("row with the wanted index")
}

#[test]
fn overlay_scene_mounts_scrim_panel_and_rows_on_token_layers() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(menu_overlay_scene(demo_entries(), &palette));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut scrims = world.query::<(&MenuScrim, &BackgroundColor)>();
    let (_, scrim) = scrims.iter(world).next().expect("scrim mounted");
    assert_eq!(scrim.0, palette.scrim());

    let mut panels = world.query::<&MenuPanel>();
    assert_eq!(panels.iter(world).count(), 1);
    assert_eq!(row_fill(world, 0).0, menu_row_fill(true, &palette));
    assert_eq!(
        row_fill(world, 2).0,
        menu_row_fill(false, &palette),
        "the cursor rests on item 0, so the disabled row is idle"
    );
}

#[test]
fn nav_messages_move_the_highlight_and_emit_outcomes_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(menu_overlay_scene(demo_entries(), &palette));
        },
    );
    app.update();

    let panel_before = panel_id(app.world_mut());
    assert_eq!(
        row_fill(app.world_mut(), 0).0,
        UiPalette::new(&Theme::dark()).hover_bg
    );

    // Two Downs from item 0: past the separator onto the disabled item, then
    // to the second enabled item.
    {
        let world = app.world_mut();
        world
            .resource_mut::<Messages<MenuNavEvent>>()
            .write(MenuNavEvent(MenuNav::Down));
        world
            .resource_mut::<Messages<MenuNavEvent>>()
            .write(MenuNavEvent(MenuNav::Down));
    }
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    assert_eq!(
        panel_id(world),
        panel_before,
        "the highlight moved without any remount"
    );
    assert_eq!(
        row_fill(world, 0).0,
        menu_row_fill(false, &palette),
        "the old highlight row is idle"
    );
    assert_eq!(
        row_fill(world, 2).0,
        menu_row_fill(false, &palette),
        "the disabled row is idle again"
    );
    assert_eq!(
        row_fill(world, 3).0,
        menu_row_fill(true, &palette),
        "the second enabled row paints the hover token"
    );

    // Confirm: the outcome names the confirmed entry index.
    {
        let world = app.world_mut();
        world
            .resource_mut::<Messages<MenuNavEvent>>()
            .write(MenuNavEvent(MenuNav::Confirm));
    }
    app.update();
    let world = app.world_mut();
    let confirmed: Vec<MenuOutcome> = world
        .resource::<Messages<MenuOutcome>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(confirmed, vec![MenuOutcome::Confirmed(3)]);

    // Cancel emits the explicit cancel outcome.
    {
        let world = app.world_mut();
        world
            .resource_mut::<Messages<MenuNavEvent>>()
            .write(MenuNavEvent(MenuNav::Cancel));
    }
    app.update();
    let world = app.world_mut();
    let outcomes: Vec<MenuOutcome> = world
        .resource::<Messages<MenuOutcome>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(
        outcomes.last(),
        Some(&MenuOutcome::Canceled),
        "cancel lands as the latest outcome"
    );
    assert!(
        world.get::<Menu>(panel_before).is_some(),
        "outcomes never unmount anything: the host owns open/close"
    );
}

#[test]
fn theme_flip_repaints_the_overlay_without_respawn() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(menu_overlay_scene(demo_entries(), &palette));
        },
    );
    app.update();

    let panel = panel_id(app.world_mut());
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let mut scrims = world.query::<(&MenuScrim, &BackgroundColor)>();
    let (_, scrim) = scrims.iter(world).next().expect("scrim survives");
    assert_eq!(scrim.0, light.scrim(), "the scrim re-derives from light");
    assert_eq!(
        row_fill(world, 0).0,
        menu_row_fill(true, &light),
        "the highlight row re-derives from light"
    );
    assert!(world.get::<MenuPanel>(panel).is_some());
}
