//! DUAL-15-11 headless tests: the focused widget-layer text field drives the
//! real window IME slot (`Window::ime_enabled` / `Window::ime_position`, the
//! pair winit turns into `set_ime_allowed` / `set_ime_cursor_area`), and the
//! real `bevy::window::Ime` composition messages land in the field's controlled
//! state through the shared tracker.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::ime::{ImeHostReport, ShellImeComposition, ShellImePlugin};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::ime::ImeCursorArea;
use infiltrator_bevy_widgets::text_input::state::TextFieldState;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused, text_field_scene};
use infiltrator_contract::ime::{ImeCursorSource, ImePhase};

fn ime_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ShellImePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    app
}

fn primary_window(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
    query.iter(world).next().expect("the primary window entity")
}

fn window_of(app: &App, entity: Entity) -> Window {
    app.world()
        .entity(entity)
        .get::<Window>()
        .expect("the window component")
        .clone()
}

fn spawn_field(app: &mut App, focused: bool, area: ImeCursorArea) -> Entity {
    app.world_mut()
        .spawn((
            TextField(TextFieldState::new("")),
            TextFieldFocused(focused),
            area,
        ))
        .id()
}

fn field_text(app: &App, entity: Entity) -> String {
    app.world()
        .entity(entity)
        .get::<TextField>()
        .expect("the controlled field")
        .0
        .text()
        .to_owned()
}

fn field_preedit(app: &App, entity: Entity) -> String {
    app.world()
        .entity(entity)
        .get::<TextField>()
        .expect("the controlled field")
        .0
        .preedit()
        .to_owned()
}

#[test]
fn the_focused_field_enables_the_window_ime_at_the_real_caret() {
    let mut app = ime_app();
    let window = primary_window(&mut app);
    assert!(
        !window_of(&app, window).ime_enabled,
        "a cold window has the IME disabled"
    );
    assert_eq!(
        app.world().resource::<ImeHostReport>().support.source(),
        Some(ImeCursorSource::SurfaceComputed),
        "Bevy computes the caret itself"
    );

    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(96.0, 210.0, 2.0, 18.0),
    );
    app.update();

    let window_state = window_of(&app, window);
    assert!(window_state.ime_enabled, "a focused field enables the IME");
    assert_eq!(
        window_state.ime_position,
        Vec2::new(96.0, 210.0),
        "the window cursor area is the widget caret rect"
    );

    let report = *app.world().resource::<ImeHostReport>();
    assert!(report.is_enabled());
    assert_eq!(report.focused_field, Some(field));
    assert_eq!(report.window, Some(window));
    let cursor = report.cursor().expect("an enabled plan has a caret");
    assert_eq!(cursor.width, 2.0);
    assert_eq!(cursor.height, 18.0);
}

#[test]
fn an_unfocused_shell_disables_the_window_ime_instead_of_tracking_a_stale_caret() {
    let mut app = ime_app();
    let window = primary_window(&mut app);
    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(96.0, 210.0, 2.0, 18.0),
    );
    app.update();
    assert!(window_of(&app, window).ime_enabled);

    app.world_mut()
        .entity_mut(field)
        .insert(TextFieldFocused(false));
    app.update();

    let window_state = window_of(&app, window);
    assert!(!window_state.ime_enabled, "no field keeps the IME closed");
    assert_eq!(
        window_state.ime_position,
        Vec2::new(96.0, 210.0),
        "the stale position stays but the OS ignores it while disabled"
    );
    let report = *app.world().resource::<ImeHostReport>();
    assert!(!report.is_enabled());
    assert_eq!(report.cursor(), None);
    assert_eq!(report.focused_field, None);
}

#[test]
fn a_caret_outside_the_window_is_clamped_before_it_reaches_the_os() {
    let mut app = ime_app();
    let window = primary_window(&mut app);
    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(9_000.0, 8_000.0, 2.0, 18.0),
    );
    app.update();

    let viewport = window_of(&app, window);
    let window_state = window_of(&app, window);
    let cursor = app
        .world()
        .resource::<ImeHostReport>()
        .cursor()
        .expect("an enabled plan has a caret");
    assert!(
        cursor.is_inside(infiltrator_contract::ime::ImeViewport::new(
            viewport.width(),
            viewport.height()
        ))
    );
    assert_eq!(window_state.ime_position, Vec2::new(cursor.x, cursor.y));
    assert!(
        window_state.ime_position.x < viewport.width(),
        "the clamped caret stays inside the window"
    );
    // The field itself is untouched by clamping.
    assert_eq!(field_text(&app, field), "");
}

#[test]
fn composition_events_reach_the_focused_field_through_the_shared_tracker() {
    let mut app = ime_app();
    let window = primary_window(&mut app);
    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(40.0, 60.0, 2.0, 18.0),
    );
    app.update();

    app.world_mut().write_message(Ime::Enabled { window });
    app.update();
    let composition = app.world().resource::<ShellImeComposition>().clone();
    assert!(composition.is_composing());
    assert_eq!(composition.0.phase(), ImePhase::Composing);

    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni hao".to_owned(),
        cursor: Some((3, 6)),
    });
    app.update();
    assert_eq!(field_preedit(&app, field), "ni hao");
    assert_eq!(
        app.world().resource::<ShellImeComposition>().preedit(),
        "ni hao"
    );

    // The preedit widens the shared caret area the window receives.
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "你好".to_owned(),
    });
    app.update();
    assert_eq!(field_text(&app, field), "你好");
    assert_eq!(field_preedit(&app, field), "");
    let composition = app.world().resource::<ShellImeComposition>().clone();
    assert!(!composition.is_composing());
    assert_eq!(composition.0.phase(), ImePhase::Idle);
}

#[test]
fn a_cancelled_composition_rolls_the_field_back() {
    let mut app = ime_app();
    let window = primary_window(&mut app);
    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(40.0, 60.0, 2.0, 18.0),
    );
    // A pre-existing controlled value that the composition must not lose.
    app.world_mut()
        .entity_mut(field)
        .insert(TextField(TextFieldState::new("代理")));
    app.update();

    app.world_mut().write_message(Ime::Enabled { window });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni".to_owned(),
        cursor: None,
    });
    app.update();
    assert_eq!(field_preedit(&app, field), "ni");

    app.world_mut().write_message(Ime::Disabled { window });
    app.update();
    assert_eq!(field_text(&app, field), "代理");
    assert_eq!(field_preedit(&app, field), "");
    assert!(!app.world().resource::<ShellImeComposition>().is_composing());
}

#[test]
fn composition_for_another_window_is_ignored() {
    let mut app = ime_app();
    let primary = primary_window(&mut app);
    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(40.0, 60.0, 2.0, 18.0),
    );
    let secondary = app.world_mut().spawn(Window::default()).id();
    app.update();

    app.world_mut().write_message(Ime::Commit {
        window: secondary,
        value: "别人的窗口".to_owned(),
    });
    app.update();
    assert_eq!(
        field_text(&app, field),
        "",
        "a second window's IME must not type into this shell"
    );
    assert!(!app.world().resource::<ShellImeComposition>().is_composing());

    app.world_mut().write_message(Ime::Commit {
        window: primary,
        value: "本窗口".to_owned(),
    });
    app.update();
    assert_eq!(field_text(&app, field), "本窗口");
}

#[test]
fn a_headless_composition_without_a_window_reports_the_plan_honestly() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ShellImePlugin);
    app.update();
    assert_eq!(
        app.world().resource::<ImeHostReport>().window,
        None,
        "no window entity means no OS slot"
    );

    let field = spawn_field(
        &mut app,
        true,
        ImeCursorArea::from_rect(12.0, 0.0, 2.0, 18.0),
    );
    app.update();
    let report = *app.world().resource::<ImeHostReport>();
    assert!(report.is_enabled());
    assert_eq!(report.window, None);
    assert_eq!(report.focused_field, Some(field));
    assert_eq!(report.cursor().expect("caret reported").x, 12.0);
}

#[test]
fn a_mounted_shell_keeps_its_ime_disabled_until_a_field_is_focused() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(
        bevy::app::Startup,
        |mut commands: bevy::ecs::system::Commands, palette: bevy::ecs::system::Res<UiPalette>| {
            commands.spawn_scene(text_field_scene(String::new(), &palette));
        },
    );
    app.update();
    app.update();

    let window = primary_window(&mut app);
    assert!(
        !window_of(&app, window).ime_enabled,
        "the mounted shell has no focused field, so the IME stays closed"
    );

    let field = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<TextField>>();
        query.iter(world).next().expect("the mounted text field")
    };
    app.world_mut()
        .entity_mut(field)
        .insert(TextFieldFocused(true));
    app.update();
    app.update();

    let state = window_of(&app, window);
    assert!(
        state.ime_enabled,
        "the real widget caret enables the host IME"
    );
    assert!(
        state.ime_position.x > 0.0,
        "the position comes from the widget-computed caret, not a literal"
    );
    let report = *app.world().resource::<ImeHostReport>();
    assert_eq!(report.focused_field, Some(field));
    assert_eq!(
        report.support.source(),
        Some(ImeCursorSource::SurfaceComputed)
    );
}
