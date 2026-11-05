//! Headless scene-composition tests: `bsn!` scenes spawn real entity trees
//! on a `MinimalPlugins` app — buttons carry the official `Button` widget
//! plus our visual state, labels carry stamped roles, surfaces accept
//! composed children.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::{AssetPlugin, Assets};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderColor, Pressed, UiTransform};
use bevy::ui_render::ui_material::MaterialNode;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::press::{BASE_SCALE, ButtonPressPhase, ButtonPressScale};
use infiltrator_bevy_widgets::button::{
    ControlVisual, PillLabel, control_fill, pill_caption_scene, pill_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::{SurfacePanel, surface_scene};
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::REM_BASE;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_bevy_widgets::waveform::{
    WaveformGlowMaterial, WaveformMaterialOwner, WaveformMaterialPaint, WaveformPlate,
    WaveformSeries, waveform_scene,
};
use infiltrator_contract::design_resources;

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
    assert_eq!(role.0, design_resources::TextRole::Body);
    assert_eq!(
        ink.0,
        UiPalette::new(&Theme::dark()).on_accent,
        "a selected pill stamps the on-accent ink"
    );
    assert!(
        matches!(font.font_size, FontSize::Rem(f) if (f * REM_BASE - 15.0).abs() < 1e-4),
        "body size stamped from the type scale through the rem channel"
    );
}

#[test]
fn an_unselected_pill_stamps_the_ordinary_ink() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("direct".to_string(), false, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<(&TextColor, &PillLabel)>();
    let (ink, _) = texts.iter(world).next().expect("pill spawns one label");
    assert_eq!(
        ink.0,
        UiPalette::new(&Theme::dark()).ink,
        "an idle pill keeps the ordinary reading ink"
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
fn surface_scene_paints_a_hairline_theme_border() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(surface_scene(Vec::new(), &palette));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut cards = world.query::<(&SurfacePanel, &BackgroundColor, &BorderColor)>();
    let (_, fill, border) = cards.iter(world).next().expect("surface card mounted");
    assert_eq!(fill.0, palette.surface);
    assert_eq!(border.top, palette.border);
    assert_eq!(border.right, palette.border);
    assert_eq!(border.bottom, palette.border);
    assert_eq!(border.left, palette.border);
}

#[test]
fn waveform_scene_binds_a_wesl_material_in_place() {
    let mut app = headless_app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(waveform_scene(WaveformSeries::Upload, vec![0.0, 1.0, 0.5]));
    });
    app.update();

    let world = app.world_mut();
    let mut plates = world.query::<(
        Entity,
        &WaveformPlate,
        &MaterialNode<WaveformGlowMaterial>,
        &WaveformMaterialOwner,
        &WaveformMaterialPaint,
    )>();
    let (_, plate, binding, owner, _paint) = plates
        .iter(world)
        .next()
        .expect("the waveform plate mounts and binds");
    assert_eq!(plate.series, WaveformSeries::Upload);
    assert_eq!(binding.0, owner.0);
    let uniform = world
        .resource::<Assets<WaveformGlowMaterial>>()
        .get(&owner.0)
        .expect("the waveform material is owned")
        .uniform;
    assert_eq!(uniform.sample_count, 3.0);
    assert_eq!(uniform.samples[0].x, 0.0);
    assert_eq!(uniform.samples[0].y, 1.0);
    assert_eq!(uniform.samples[0].z, 0.5);
}

#[test]
fn control_fill_layers_pressed_over_hover_over_selected() {
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(control_fill(true, false, false, &palette), palette.accent);
    assert_eq!(control_fill(true, true, false, &palette), palette.hover_bg);
    assert_eq!(control_fill(true, true, true, &palette), palette.pressed_bg);
    assert_eq!(
        control_fill(false, false, false, &palette),
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

#[test]
fn pills_carry_the_same_elastic_press_feedback_as_buttons() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("proxy".to_string(), false, &palette));
            commands.spawn_scene(pill_caption_scene("rule".to_string(), true, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut pills = world.query::<(Entity, &ButtonPressScale, &UiTransform)>();
    let mounted: Vec<Entity> = pills
        .iter(world)
        .map(|(entity, press, transform)| {
            assert_eq!(press.phase, ButtonPressPhase::Resting);
            assert_eq!(transform.scale.x, BASE_SCALE);
            entity
        })
        .collect();
    assert_eq!(mounted.len(), 2, "both pill variants carry the spring");

    world.entity_mut(mounted[0]).insert(Pressed);
    app.update();

    let world = app.world_mut();
    let press = world
        .get::<ButtonPressScale>(mounted[0])
        .expect("pill press state survives");
    assert!(press.scale() < BASE_SCALE, "a pressed pill shrinks");
}
