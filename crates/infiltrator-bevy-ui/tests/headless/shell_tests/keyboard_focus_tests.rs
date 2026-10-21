//! BEVY-038: the shell-level keyboard focus gesture and the visible focus
//! ring.
//!
//! The D-pad focus lands on the first measured control without any gesture,
//! so a keyboard gesture is what actually *moves* the ring onto another
//! control — and the ring itself is the visible cue that the focus moved.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::scene::ScenePlugin;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::shell_focus::{ShellFocusRing, ShellFocusState, ShellFocusable};

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app
}

/// Spawn a focusable control with a measured box, as the layout would provide.
fn measured(app: &mut App, x: f32, y: f32) -> Entity {
    app.world_mut()
        .spawn((
            ShellFocusable,
            ComputedNode {
                size: Vec2::new(80.0, 32.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(x, y),
        ))
        .id()
}

#[test]
fn the_tab_gesture_moves_the_focus_ring_onto_another_control() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();

    // The route-only baseline already focuses the first control.
    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(left)
    );

    // The keyboard gesture moves the focus onto the next control.
    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::Tab);
    app.insert_resource(keyboard);
    app.update();

    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(right),
        "the Tab gesture moves the focus off the baseline control"
    );

    // The ring follows the focus: it decorates the focused control.
    let ring = app
        .world_mut()
        .query::<&ShellFocusRing>()
        .single(app.world())
        .expect("the focus ring is drawn over the focused control");
    assert_eq!(ring.0, right, "the ring decorates the focused control");
}

#[test]
fn shift_tab_moves_the_focus_ring_backwards() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();

    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::ShiftLeft);
    keyboard.press(KeyCode::Tab);
    app.insert_resource(keyboard);
    app.update();

    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(right),
        "Shift+Tab wraps to the previous control"
    );

    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::ShiftLeft);
    keyboard.press(KeyCode::Tab);
    app.insert_resource(keyboard);
    app.update();

    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(left),
        "Shift+Tab steps back through the focus order"
    );
}
