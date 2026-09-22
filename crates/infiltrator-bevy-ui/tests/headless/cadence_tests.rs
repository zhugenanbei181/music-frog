//! DUAL-15-08 render-cadence tests: the shared policy drives Bevy's real power
//! knob (`WinitSettings`) and the widget-layer frame-pacing vocabulary mirrors
//! the same three product rates.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::window::{Window, WindowFocused, WindowOccluded};
use bevy::winit::{UpdateMode, WinitSettings};
use infiltrator_bevy_ui::cadence::{
    CadencePlugin, SUSPENDED_WAIT, WindowCadenceState, winit_settings_for,
};
use infiltrator_bevy_widgets::cadence::FramePacingMode;
use infiltrator_contract::cadence::RenderCadence;

fn reactive_wait(mode: UpdateMode) -> std::time::Duration {
    match mode {
        UpdateMode::Reactive { wait, .. } => wait,
        other => panic!("expected a reactive mode, got {other:?}"),
    }
}

fn cadence_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CadencePlugin);
    app.world_mut().spawn(Window::default());
    app.update();
    app
}

fn primary_window(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<Window>>();
    query.iter(world).next().expect("the primary window entity")
}

#[test]
fn the_winit_modes_follow_the_shared_cadence() {
    let active = winit_settings_for(RenderCadence::Active);
    assert_eq!(active.focused_mode, UpdateMode::Continuous);
    assert_eq!(
        reactive_wait(active.unfocused_mode),
        std::time::Duration::from_millis(RenderCadence::BACKGROUND_FRAME_TIME_MS)
    );

    let background = winit_settings_for(RenderCadence::Background);
    assert_eq!(
        reactive_wait(background.focused_mode),
        std::time::Duration::from_millis(RenderCadence::BACKGROUND_FRAME_TIME_MS),
        "a backgrounded window runs at the shared 2 FPS rate"
    );
    assert_eq!(
        reactive_wait(background.unfocused_mode),
        reactive_wait(background.focused_mode)
    );

    let suspended = winit_settings_for(RenderCadence::Suspended);
    assert_eq!(reactive_wait(suspended.focused_mode), SUSPENDED_WAIT);
    assert_eq!(reactive_wait(suspended.unfocused_mode), SUSPENDED_WAIT);
    assert!(SUSPENDED_WAIT >= std::time::Duration::from_secs(60));
}

#[test]
fn the_widget_frame_pacing_vocabulary_mirrors_the_shared_cadence() {
    assert_eq!(
        FramePacingMode::HighRefresh.target_frame_time_ms(),
        RenderCadence::Active.frame_time_ms()
    );
    assert_eq!(
        FramePacingMode::BackgroundThrottled.target_frame_time_ms(),
        RenderCadence::Background.frame_time_ms()
    );
    assert_eq!(
        FramePacingMode::Suspended.target_frame_time_ms(),
        RenderCadence::Suspended.frame_time_ms()
    );
}

#[test]
fn focus_and_occlusion_events_reselect_the_winit_cadence() {
    let mut app = cadence_app();
    assert_eq!(
        app.world().resource::<WindowCadenceState>().cadence(),
        RenderCadence::Active
    );

    let window = primary_window(&mut app);
    app.world_mut().write_message(WindowFocused {
        window,
        focused: false,
    });
    app.update();
    assert_eq!(
        app.world().resource::<WindowCadenceState>().cadence(),
        RenderCadence::Background
    );
    assert_eq!(
        reactive_wait(app.world().resource::<WinitSettings>().focused_mode),
        std::time::Duration::from_millis(RenderCadence::BACKGROUND_FRAME_TIME_MS)
    );

    // Occlusion is the host's hard-suspend fact.
    app.world_mut().write_message(WindowOccluded {
        window,
        occluded: true,
    });
    app.update();
    assert_eq!(
        app.world().resource::<WindowCadenceState>().cadence(),
        RenderCadence::Suspended
    );
    assert_eq!(
        reactive_wait(app.world().resource::<WinitSettings>().unfocused_mode),
        SUSPENDED_WAIT
    );

    app.world_mut().write_message(WindowOccluded {
        window,
        occluded: false,
    });
    app.update();
    assert_eq!(
        app.world().resource::<WindowCadenceState>().cadence(),
        RenderCadence::Background,
        "still unfocused after the window becomes visible again"
    );
}

/// The plugin keeps the resource inert until a real change arrives: a plain
/// update must not rewrite the winit settings another writer installed.
#[test]
fn an_unchanged_cadence_does_not_rewrite_the_winit_settings() {
    let mut app = cadence_app();
    let custom = WinitSettings {
        focused_mode: UpdateMode::reactive(std::time::Duration::from_secs(3)),
        unfocused_mode: UpdateMode::reactive(std::time::Duration::from_secs(9)),
    };
    app.insert_resource(custom.clone());
    app.update();
    let installed = app.world().resource::<WinitSettings>().clone();
    assert_eq!(installed.focused_mode, custom.focused_mode);
    assert_eq!(installed.unfocused_mode, custom.unfocused_mode);

    // The tracked facts still describe the cold-start active window.
    assert_eq!(
        app.world().resource::<WindowCadenceState>().cadence(),
        RenderCadence::Active
    );
}
