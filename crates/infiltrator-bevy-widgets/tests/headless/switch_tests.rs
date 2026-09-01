//! Headless test for the theme-switch seam: a `ThemeSwitch` trigger rewrites
//! the palette resource and restamps text and control fills in place — the
//! same entities before and after, no remount of the scene tree.

use bevy::app::{Startup, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::CommandsSceneExt;
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::button::{ControlVisual, pill_scene};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

use super::support::headless_app;

#[test]
fn switch_to_light_rethemes_the_mounted_tree_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("proxy".to_string(), false, &palette));
            commands.spawn_scene(checkbox_scene("tun".to_string(), true, &palette));
        },
    );
    app.update();

    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());
    assert_ne!(dark.ink, light.ink, "the modes under test are distinct");

    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &TextColor)>();
    let text_ids: Vec<Entity> = texts.iter(world).map(|(entity, _)| entity).collect();
    assert!(!text_ids.is_empty());

    let world = app.world_mut();
    let mut pills = world.query::<(&ControlVisual, &BackgroundColor)>();
    let pill_fill_before = pills
        .iter(world)
        .next()
        .map(|(_, fill)| *fill)
        .expect("one pill with its fill");
    assert_eq!(pill_fill_before, BackgroundColor(dark.surface_elevated));

    app.add_systems(Update, |mut commands: Commands| {
        commands.trigger(ThemeSwitch(LightDark::Light));
    });
    app.update();

    // The palette resource now resolves the light token set.
    let world = app.world_mut();
    assert_eq!(*world.resource::<UiPalette>(), light);

    // Text entities kept their identity and picked up light ink.
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &TextColor)>();
    let after: Vec<Entity> = texts.iter(world).map(|(entity, _)| entity).collect();
    assert_eq!(
        after, text_ids,
        "text entities were restamped, not remounted"
    );
    let mut texts = world.query::<&TextColor>();
    for ink in texts.iter(world) {
        assert_eq!(ink.0, light.ink, "text ink refreshed to the light token");
    }

    // The idle pill re-projected the light control surface.
    let world = app.world_mut();
    let mut pills = world.query::<(&ControlVisual, &BackgroundColor)>();
    let pill_fill_after = pills
        .iter(world)
        .next()
        .map(|(_, fill)| *fill)
        .expect("the pill keeps its fill");
    assert_eq!(pill_fill_after, BackgroundColor(light.surface_elevated));

    // A second switch returns everything to the dark tokens.
    app.add_systems(Update, |mut commands: Commands| {
        commands.trigger(ThemeSwitch(LightDark::Dark));
    });
    app.update();

    let world = app.world_mut();
    assert_eq!(*world.resource::<UiPalette>(), dark);
    let mut texts = world.query::<&TextColor>();
    for ink in texts.iter(world) {
        assert_eq!(ink.0, dark.ink);
    }
}
