//! Headless tests for polymorphic buttons: Primary, Default, Secondary, Ghost,
//! Danger, Outline variants, size ladders, and loading/disabled state sync.

use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Startup, Update};
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use bevy::ecs::system::{Commands, Res};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input_focus::FocusedInput;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, Node};
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui::{Val, widget};
use bevy::ui_widgets::{Activate, ButtonPlugin};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::{
    ButtonDisabled, ButtonLoading, ButtonSize, ButtonSizeStyle, ButtonVariant, ButtonVariantStyle,
    button_fill, button_scene, button_sized_scene, button_text_color, loading_button_scene,
    sync_button_disabled,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};
use std::time::Duration;

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
        Color::NONE
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
        palette.disabled_ink
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
                assert!(matches!(node.height, Val::Px(h) if h < 36.0));
            }
            ButtonSize::Lg => {
                assert_eq!(variant.0, ButtonVariant::Primary);
                assert!(matches!(node.height, Val::Px(h) if h > 36.0));
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

    let mut texts = world.query::<&widget::Text>();
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
    let mut btns = world.query_filtered::<(Entity, &BackgroundColor), With<ButtonVariantStyle>>();
    let (entity, dark_fill) = btns.iter(world).next().expect("button mounted");
    let dark_palette = UiPalette::new(&Theme::dark());
    assert_eq!(*dark_fill, BackgroundColor(dark_palette.accent));

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light_palette = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let light_fill = world
        .get::<BackgroundColor>(entity)
        .expect("button survives");
    assert_eq!(*light_fill, BackgroundColor(light_palette.accent));
}

#[test]
fn disabled_skin_sdk_and_accessibility_recover_on_the_same_native_button_and_label() {
    let mut app = headless_app();
    let palette = *app.world().resource::<UiPalette>();
    let button = app
        .world_mut()
        .commands()
        .spawn_scene(button_scene(
            "Repair".into(),
            ButtonVariant::Default,
            &palette,
        ))
        .id();
    app.update();
    let label = app.world().get::<Children>(button).unwrap()[0];
    app.world_mut().get_mut::<ButtonDisabled>(button).unwrap().0 = true;
    app.update();
    app.update();
    assert!(app.world().get::<InteractionDisabled>(button).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(button)
            .unwrap()
            .is_disabled()
    );
    assert_eq!(
        app.world().get::<TextColor>(label).unwrap().0,
        palette.disabled_ink
    );
    app.world_mut().get_mut::<ButtonDisabled>(button).unwrap().0 = false;
    app.update();
    app.update();
    assert!(app.world().get::<InteractionDisabled>(button).is_none());
    assert!(
        !app.world()
            .get::<AccessibilityNode>(button)
            .unwrap()
            .is_disabled()
    );
    assert_eq!(app.world().get::<TextColor>(label).unwrap().0, palette.ink);
    assert_eq!(app.world().get::<Children>(button).unwrap()[0], label);
}

#[derive(Resource, Default)]
struct NativeActivationCount(usize);
fn press_space(app: &mut App, button: Entity) {
    app.world_mut().commands().trigger(FocusedInput::new(
        button,
        KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        },
        Entity::PLACEHOLDER,
    ));
    app.world_mut().flush();
}
#[test]
fn disabled_constructor_blocks_sdk_keyboard_activation_before_first_frame_and_recovery_restores_it()
{
    let mut app = headless_app();
    app.add_plugins(ButtonPlugin);
    app.init_resource::<NativeActivationCount>();
    app.add_observer(|_: On<Activate>, mut count: ResMut<NativeActivationCount>| count.0 += 1);
    let palette = *app.world().resource::<UiPalette>();
    let button = app
        .world_mut()
        .commands()
        .spawn_scene(button_scene(
            "Apply".into(),
            ButtonVariant::Default,
            &palette,
        ))
        .insert(ButtonDisabled(true))
        .id();
    app.world_mut().flush();
    assert!(app.world().get::<InteractionDisabled>(button).is_some());
    press_space(&mut app, button);
    assert_eq!(app.world().resource::<NativeActivationCount>().0, 0);
    app.world_mut().get_mut::<ButtonDisabled>(button).unwrap().0 = false;
    app.update();
    press_space(&mut app, button);
    assert_eq!(app.world().resource::<NativeActivationCount>().0, 1);
}

fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::None {
                width: 100,
                height: 100,
            },
            position: Vec2::new(10.0, 10.0),
        },
    )
}
fn pointer_press(app: &mut App, button: Entity) {
    app.world_mut().commands().trigger(PointerPress {
        entity: button,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.world_mut().flush();
}
fn pointer_click(app: &mut App, button: Entity) {
    app.world_mut().commands().trigger(PointerClick {
        entity: button,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: Duration::from_millis(10),
        count: 1,
    });
    app.world_mut().flush();
}
#[test]
fn disabling_during_sdk_pointer_press_cancels_that_press_and_recovery_rejects_its_old_click() {
    let mut app = headless_app();
    app.add_plugins(ButtonPlugin);
    app.init_resource::<NativeActivationCount>();
    app.add_observer(|_: On<Activate>, mut count: ResMut<NativeActivationCount>| count.0 += 1);
    let palette = *app.world().resource::<UiPalette>();
    let button = app
        .world_mut()
        .commands()
        .spawn_scene(button_scene(
            "Apply".into(),
            ButtonVariant::Default,
            &palette,
        ))
        .id();
    app.update();
    pointer_press(&mut app, button);
    assert!(app.world().get::<Pressed>(button).is_some());
    app.world_mut().get_mut::<ButtonDisabled>(button).unwrap().0 = true;
    app.update();
    assert!(app.world().get::<Pressed>(button).is_none());
    app.world_mut().get_mut::<ButtonDisabled>(button).unwrap().0 = false;
    app.update();
    pointer_click(&mut app, button);
    assert_eq!(app.world().resource::<NativeActivationCount>().0, 0);
    pointer_press(&mut app, button);
    pointer_click(&mut app, button);
    assert_eq!(app.world().resource::<NativeActivationCount>().0, 1);
}

#[test]
fn disabled_sync_respects_row_retirement_before_the_schedule_deferred_flush() {
    let mut app = headless_app();
    let retired = app.world_mut().spawn(ButtonDisabled(true)).id();
    app.world_mut().flush();
    let replacement = app.world_mut().spawn(ButtonDisabled(false)).id();
    app.world_mut().flush();
    app.add_systems(
        Update,
        (move |mut commands: Commands| {
            commands.entity(retired).despawn();
        })
        .before_ignore_deferred(sync_button_disabled),
    );
    app.update();
    assert!(app.world().get_entity(retired).is_err());
    assert!(
        app.world()
            .get::<InteractionDisabled>(replacement)
            .is_none()
    );
    assert!(!app.world().get::<ButtonDisabled>(replacement).unwrap().0);
}
