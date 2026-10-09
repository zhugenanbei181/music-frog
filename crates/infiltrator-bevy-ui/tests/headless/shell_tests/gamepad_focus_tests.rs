//! BEVY-038: the shell-level D-pad / analog-stick focus controller wired into
//! the real `ShellPlugin`. Focus moves across measured controls with the shared
//! radial deadzone and wrap; "no focus" is a typed state, and confirm/cancel map
//! onto the shell's activation / back vocabulary.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::math::Vec2;
use bevy::scene::ScenePlugin;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::shell_focus::{
    ShellFocusConfig, ShellFocusNav, ShellFocusState, ShellFocusable,
};
use infiltrator_bevy_ui::route::NavigateBack;
use infiltrator_bevy_widgets::focus::Focused;
use infiltrator_bevy_widgets::gamepad_ui::GamepadNavAction;

#[derive(Resource, Default)]
struct Activated(Vec<Entity>);

#[derive(Resource, Default)]
struct BackRequests(u32);

fn record_activate(activate: On<Activate>, mut log: ResMut<Activated>) {
    log.0.push(activate.entity);
}

fn record_back(_back: On<NavigateBack>, mut count: ResMut<BackRequests>) {
    count.0 += 1;
}

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.init_resource::<Activated>();
    app.init_resource::<BackRequests>();
    app.add_observer(record_activate);
    app.add_observer(record_back);
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

fn state(app: &App) -> ShellFocusState {
    *app.world().resource::<ShellFocusState>()
}

#[test]
fn dpad_moves_focus_and_wraps_across_measured_controls() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let middle = measured(&mut app, 100.0, 0.0);
    let right = measured(&mut app, 200.0, 0.0);
    app.update();

    assert_eq!(state(&app), ShellFocusState::Focused(left));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(middle));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(right));

    // Wrap right at the edge returns to the same-row leftmost control.
    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(left));
}

#[test]
fn analog_stick_honours_the_deadzone_before_moving() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(left));

    // Inside the radial deadzone: the sample is discarded.
    app.world_mut()
        .write_message(ShellFocusNav::Stick(Vec2::new(0.05, 0.0)));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(left));

    // Past the deadzone: the dominant axis moves focus.
    app.world_mut()
        .write_message(ShellFocusNav::Stick(Vec2::new(0.9, 0.1)));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(right));
}

#[test]
fn wrap_can_be_disabled_through_the_config() {
    let mut app = mounted_shell();
    let _left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    app.world_mut().resource_mut::<ShellFocusConfig>().wrap = false;

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(right));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(
        state(&app),
        ShellFocusState::Focused(right),
        "no wrap keeps focus on the edge control"
    );
}

#[test]
fn confirm_activates_the_focused_control_and_cancel_navigates_back() {
    let mut app = mounted_shell();
    let first = measured(&mut app, 0.0, 0.0);
    let _second = measured(&mut app, 100.0, 0.0);
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(first));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::ButtonAConfirm));
    app.update();
    assert_eq!(
        app.world().resource::<Activated>().0,
        vec![first],
        "A confirms the focused control"
    );

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::ButtonBCancel));
    app.update();
    assert_eq!(app.world().resource::<BackRequests>().0, 1);
}

#[test]
fn the_focused_control_carries_the_widget_focus_marker() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    assert!(app.world().get::<Focused>(left).is_some());
    assert!(app.world().get::<Focused>(right).is_none());

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert!(app.world().get::<Focused>(left).is_none());
    assert!(app.world().get::<Focused>(right).is_some());
}

#[test]
fn no_measured_controls_is_a_typed_no_focus_state() {
    let app = mounted_shell();
    assert_eq!(state(&app), ShellFocusState::NoFocus);
    assert_eq!(state(&app).focused_entity(), None);
}

#[test]
fn mounted_page_buttons_are_reachable_without_a_shell_marker() {
    let mut app = mounted_shell();
    let marked = measured(&mut app, 0.0, 0.0);
    let page_button = app
        .world_mut()
        .spawn((
            Button,
            ComputedNode {
                size: Vec2::new(80.0, 32.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(100.0, 0.0),
        ))
        .id();
    app.update();
    assert_eq!(state(&app), ShellFocusState::Focused(marked));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(
        state(&app),
        ShellFocusState::Focused(page_button),
        "a mounted page button is a D-pad target"
    );
}
