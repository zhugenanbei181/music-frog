//! Headless scene-composition tests: `bsn!` scenes spawn real entity trees
//! on a `MinimalPlugins` app — buttons carry the official `Button` widget
//! plus our visual state, labels carry stamped roles, surfaces accept
//! composed children.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::CommandsSceneExt;
use bevy::scene::ScenePlugin;
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::{ControlVisual, PillLabel, pill_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `spawn_scene` resolves bsn! scenes through the asset infrastructure
    // (AssetServer + Assets<ScenePatch>) — the singleton plugins a windowed
    // run inherits from DefaultPlugins, added explicitly here.
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn pill_scene_spawns_button_with_stamped_label() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("proxy".to_string(), true, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut buttons = world.query::<(&ControlVisual, &Button)>();
    let mut buttons_iter = buttons.iter(world);
    let (visual, _button) = buttons_iter.next().expect("pill spawns one button");
    assert!(visual.0, "selected bit rides the visual component");

    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &TextRole, &TextColor, &TextFont)>();
    let (_, role, ink, font) = texts.iter(world).next().expect("pill spawns one label");
    assert_eq!(role.0, Role::Body);
    assert_eq!(
        ink.0,
        UiPalette::new(&Theme::dark()).ink,
        "body ink stamped"
    );
    assert!(
        matches!(font.font_size, FontSize::Px(size) if size == 15.0),
        "body size stamped from the type scale"
    );
}

#[test]
fn surface_scene_accepts_composed_children() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(surface_scene(
                vec![Box::new(pill_scene(
                    "overview".to_string(),
                    false,
                    &palette,
                ))],
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut buttons = world.query::<&Button>();
    assert_eq!(
        buttons.iter(world).count(),
        1,
        "card hosts exactly one pill"
    );
}

#[test]
fn control_fill_layers_pressed_over_hover_over_selected() {
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(
        infiltrator_bevy_widgets::button::control_fill(true, false, false, &palette),
        palette.accent
    );
    assert_eq!(
        infiltrator_bevy_widgets::button::control_fill(true, true, false, &palette),
        palette.hover_bg
    );
    assert_eq!(
        infiltrator_bevy_widgets::button::control_fill(true, true, true, &palette),
        palette.pressed_bg
    );
    assert_eq!(
        infiltrator_bevy_widgets::button::control_fill(false, false, false, &palette),
        palette.surface_elevated
    );
}

/// The pill label's ink follows the selected bit: `on_accent` while
/// selected, ordinary ink otherwise — restamped in place, ids unchanged.
#[test]
fn pill_label_ink_follows_the_selected_bit() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("mode".to_string(), true, &palette));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut pills = world.query::<(Entity, &ControlVisual, &Children)>();
    let (pill, visual, children) = pills.iter(world).next().expect("pill mounted");
    assert!(visual.0, "spawned selected");
    let label = *children.iter().next().expect("label child");
    let ink = world.get::<TextColor>(label).expect("label ink");
    assert_eq!(ink.0, palette.on_accent, "selected label draws on_accent");

    let world = app.world_mut();
    world.entity_mut(pill).insert(ControlVisual(false));
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let ink = world.get::<TextColor>(label).expect("label survives");
    assert_eq!(ink.0, palette.ink, "idle label draws the ordinary ink");
    assert!(
        world.get::<PillLabel>(label).is_some(),
        "same label entity, restamped in place"
    );
}
