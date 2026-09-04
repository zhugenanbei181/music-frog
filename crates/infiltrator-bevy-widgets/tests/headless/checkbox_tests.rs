//! Headless tests for the checkbox wrapper: official `Checkbox` semantics on
//! the row, token box visuals driven by the official `Checked` marker,
//! tri-state / indeterminate states, and in-place repaint when state or palette changes.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::Has;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use bevy::ui::BorderColor;
use bevy::ui::Checked;
use bevy::ui_widgets::Checkbox;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::checkbox::{
    CheckboxBox, CheckboxDash, TriState, checkbox_border, checkbox_fill, checkbox_scene,
    tri_checkbox_border, tri_checkbox_fill, tri_checkbox_next, tri_checkbox_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn checkbox_scene_spawns_official_widget_with_token_box() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(checkbox_scene("tun".to_string(), true, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut rows = world.query::<(&Checkbox, Has<Checked>, &Children)>();
    let (_, checked, _) = rows.iter(world).next().expect("one check row");
    assert!(checked, "spawned checked state rides the official marker");

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut boxes = world.query::<(&CheckboxBox, &BackgroundColor)>();
    let (_, fill) = boxes.iter(world).next().expect("the box carries its fill");
    assert_eq!(*fill, BackgroundColor(checkbox_fill(true, &palette)));
    assert_eq!(
        *fill,
        BackgroundColor(palette.accent),
        "checked box fills with the accent token"
    );
}

#[test]
fn checked_marker_flip_repaints_box_without_respawn() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(checkbox_scene("tun".to_string(), true, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut rows = world.query::<(Entity, &Checkbox)>();
    let row = rows.iter(world).next().expect("one check row").0;

    let world = app.world_mut();
    world.entity_mut(row).remove::<Checked>();
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut fills = world.query::<(&CheckboxBox, &BackgroundColor)>();
    let (_, fill) = fills.iter(world).next().expect("box fill survives");
    assert_eq!(*fill, BackgroundColor(checkbox_fill(false, &palette)));
    assert_eq!(*fill, BackgroundColor(palette.surface_elevated));

    let mut borders = world.query::<(&CheckboxBox, &BorderColor)>();
    let (_, border) = borders.iter(world).next().expect("box border survives");
    assert_eq!(border.top, checkbox_border(false, &palette));
}

#[test]
fn box_fill_layers_follow_the_tokens() {
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(checkbox_fill(true, &palette), palette.accent);
    assert_eq!(checkbox_fill(false, &palette), palette.surface_elevated);
    assert_eq!(checkbox_border(true, &palette), palette.accent);
    assert_eq!(checkbox_border(false, &palette), palette.border);
    let light = UiPalette::new(&Theme::light());
    assert_eq!(checkbox_fill(false, &light), light.surface_elevated);
}

#[test]
fn tri_state_transitions_and_dash_visibility() {
    let palette = UiPalette::new(&Theme::dark());

    // Cycle without indeterminate
    assert_eq!(
        tri_checkbox_next(TriState::Unchecked, false),
        TriState::Checked
    );
    assert_eq!(
        tri_checkbox_next(TriState::Checked, false),
        TriState::Unchecked
    );

    // Cycle with indeterminate
    assert_eq!(
        tri_checkbox_next(TriState::Unchecked, true),
        TriState::Checked
    );
    assert_eq!(
        tri_checkbox_next(TriState::Checked, true),
        TriState::Indeterminate
    );
    assert_eq!(
        tri_checkbox_next(TriState::Indeterminate, true),
        TriState::Unchecked
    );

    // Fills & Borders
    assert_eq!(
        tri_checkbox_fill(TriState::Indeterminate, &palette),
        palette.accent
    );
    assert_eq!(
        tri_checkbox_border(TriState::Indeterminate, &palette),
        palette.accent
    );

    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(tri_checkbox_scene(
                "All Rules".to_owned(),
                TriState::Indeterminate,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut dashes = world.query::<(&CheckboxDash, &Visibility)>();
    let (_, vis) = dashes.iter(world).next().expect("dash mounted");
    assert_eq!(*vis, Visibility::Visible);
}
