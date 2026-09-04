//! Headless tests for polymorphic buttons: Primary, Default, Secondary, Ghost,
//! Danger, Outline variants, size ladders, and loading/disabled state sync.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::{BackgroundColor, Node};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::{
    ButtonDisabled, ButtonLoading, ButtonSize, ButtonSizeStyle, ButtonVariant, ButtonVariantStyle,
    button_fill, button_scene, button_sized_scene, button_text_color, loading_button_scene,
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

#[test]
fn button_variants_fill_and_text_color_matrix() {
    let palette = UiPalette::new(&Theme::dark());

    // Primary
    assert_eq!(
        button_fill(ButtonVariant::Primary, false, false, false, false, &palette),
        palette.accent
    );
    assert_eq!(
        button_text_color(ButtonVariant::Primary, false, false, &palette),
        palette.on_accent
    );

    // Default
    assert_eq!(
        button_fill(ButtonVariant::Default, false, false, false, false, &palette),
        palette.surface_elevated
    );
    assert_eq!(
        button_text_color(ButtonVariant::Default, false, false, &palette),
        palette.ink
    );

    // Secondary
    assert_eq!(
        button_fill(
            ButtonVariant::Secondary,
            false,
            false,
            false,
            false,
            &palette
        ),
        palette.accent_container
    );
    assert_eq!(
        button_text_color(ButtonVariant::Secondary, false, false, &palette),
        palette.accent
    );

    // Ghost
    assert_eq!(
        button_fill(ButtonVariant::Ghost, false, false, false, false, &palette),
        bevy::color::Color::NONE
    );
    assert_eq!(
        button_fill(ButtonVariant::Ghost, false, true, false, false, &palette),
        palette.hover_bg
    );

    // Danger
    assert_eq!(
        button_fill(ButtonVariant::Danger, false, false, false, false, &palette),
        palette.danger
    );
    assert_eq!(
        button_text_color(ButtonVariant::Danger, false, false, &palette),
        palette.on_accent
    );

    // Disabled states
    assert_eq!(
        button_text_color(ButtonVariant::Primary, false, true, &palette),
        palette.ink_dim
    );
}

#[test]
fn button_sized_scene_sets_correct_metrics() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(button_sized_scene(
                "Small".to_owned(),
                ButtonVariant::Default,
                ButtonSize::Sm,
                &palette,
            ));
            commands.spawn_scene(button_sized_scene(
                "Large".to_owned(),
                ButtonVariant::Primary,
                ButtonSize::Lg,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut buttons = world.query::<(&ButtonVariantStyle, &ButtonSizeStyle, &Node)>();
    let mut count = 0;
    for (variant, size, node) in buttons.iter(world) {
        count += 1;
        match size.0 {
            ButtonSize::Sm => {
                assert_eq!(variant.0, ButtonVariant::Default);
                assert!(matches!(node.height, bevy::ui::Val::Px(h) if h < 36.0));
            }
            ButtonSize::Lg => {
                assert_eq!(variant.0, ButtonVariant::Primary);
                assert!(matches!(node.height, bevy::ui::Val::Px(h) if h > 36.0));
            }
            _ => {}
        }
    }
    assert_eq!(count, 2);
}

#[test]
fn loading_button_scene_renders_dots_and_disables() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(loading_button_scene(
                "Save".to_owned(),
                true,
                ButtonVariant::Primary,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut btns = world.query::<(&ButtonLoading, &ButtonDisabled)>();
    let (loading, disabled) = btns.iter(world).next().expect("loading button mounted");
    assert!(loading.0);
    assert!(disabled.0);

    let mut texts = world.query::<&bevy::ui::widget::Text>();
    let text = texts.iter(world).next().expect("text mounted");
    assert_eq!(text.0, "•••");
}

#[test]
fn button_theme_flip_repaints_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(button_scene(
                "Action".to_owned(),
                ButtonVariant::Primary,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut btns = world.query::<(Entity, &BackgroundColor)>();
    let (entity, dark_fill) = btns.iter(world).next().expect("button mounted");
    let dark_palette = UiPalette::new(&Theme::dark());
    assert_eq!(*dark_fill, BackgroundColor(dark_palette.accent));

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light_palette = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let light_fill = world
        .get::<BackgroundColor>(entity)
        .expect("button survives");
    assert_eq!(*light_fill, BackgroundColor(light_palette.accent));
}
