//! BEVY-020.2: the shell's D-pad / analog-stick focus moves the official
//! input focus.
//!
//! The official directional navigation owns the neighbour search and moves the
//! official `InputFocus`; the shell's own edge policy (the wrap) is only the
//! fallback for "the official navigator found no neighbour". The headless rig
//! has no UI camera, so the official *automatic* search has no layout to
//! measure; the official manual navigation map states the hop instead — the
//! official channel for stating a navigation edge.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::input_focus::directional_navigation::DirectionalNavigationMap;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::math::{CompassOctant, Vec2};
use bevy::scene::ScenePlugin;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::shell_focus::{
    ShellFocusConfig, ShellFocusNav, ShellFocusOrder, ShellFocusState, ShellFocusable,
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

fn official_focus(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn the_dpad_moves_the_official_focus_through_the_official_navigator() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let middle = measured(&mut app, 100.0, 0.0);
    let right = measured(&mut app, 200.0, 0.0);
    app.update();
    assert_eq!(official_focus(&app), Some(left));

    // The official map states the hops; the official navigator moves the
    // official input focus along them.
    let mut map = app.world_mut().resource_mut::<DirectionalNavigationMap>();
    map.add_edge(left, middle, CompassOctant::East);
    map.add_edge(middle, right, CompassOctant::East);

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(
        official_focus(&app),
        Some(middle),
        "the D-pad moves the official input focus"
    );
    assert_eq!(state(&app), ShellFocusState::Focused(middle));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(official_focus(&app), Some(right));
}

#[test]
fn the_analog_stick_honours_the_deadzone_before_moving() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    assert_eq!(official_focus(&app), Some(left));

    app.world_mut()
        .resource_mut::<DirectionalNavigationMap>()
        .add_edge(left, right, CompassOctant::East);

    // Inside the radial deadzone: the sample is discarded.
    app.world_mut()
        .write_message(ShellFocusNav::Stick(Vec2::new(0.05, 0.0)));
    app.update();
    assert_eq!(official_focus(&app), Some(left));

    // Past the deadzone: the dominant axis moves the focus.
    app.world_mut()
        .write_message(ShellFocusNav::Stick(Vec2::new(0.9, 0.1)));
    app.update();
    assert_eq!(official_focus(&app), Some(right));
}

#[test]
fn the_wrap_falls_back_to_the_far_end_when_there_is_no_neighbour() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let _middle = measured(&mut app, 100.0, 0.0);
    let right = measured(&mut app, 200.0, 0.0);
    app.update();

    // The focus sits on the last control of the row.
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(right, FocusCause::Navigated);
    app.update();
    assert_eq!(official_focus(&app), Some(right));

    // The official navigator finds no neighbour to the east, so the shell's own
    // edge policy wraps the focus around the row.
    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(
        official_focus(&app),
        Some(left),
        "the focus wraps around the edge of the row"
    );
}

#[test]
fn the_wrap_can_be_disabled_through_the_config() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    app.world_mut().resource_mut::<ShellFocusConfig>().wrap = false;

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(right, FocusCause::Navigated);
    app.update();

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::DpadRight));
    app.update();
    assert_eq!(
        official_focus(&app),
        Some(right),
        "no wrap keeps the focus on the edge control"
    );
    assert!(app.world().get::<Focused>(left).is_none());
}

#[test]
fn confirm_activates_the_officially_focused_control_and_cancel_navigates_back() {
    let mut app = mounted_shell();
    let first = measured(&mut app, 0.0, 0.0);
    let _second = measured(&mut app, 100.0, 0.0);
    app.update();
    assert_eq!(official_focus(&app), Some(first));

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::ButtonAConfirm));
    app.update();
    assert_eq!(
        app.world().resource::<Activated>().0,
        vec![first],
        "A confirms the officially focused control"
    );

    app.world_mut()
        .write_message(ShellFocusNav::Action(GamepadNavAction::ButtonBCancel));
    app.update();
    assert_eq!(app.world().resource::<BackRequests>().0, 1);
}

#[test]
fn the_officially_focused_control_carries_the_widget_focus_marker() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    assert!(app.world().get::<Focused>(left).is_some());
    assert!(app.world().get::<Focused>(right).is_none());

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(right, FocusCause::Navigated);
    app.update();
    assert!(app.world().get::<Focused>(left).is_none());
    assert!(app.world().get::<Focused>(right).is_some());
}

#[test]
fn no_measured_controls_is_a_typed_no_focus_state() {
    let app = mounted_shell();
    assert_eq!(state(&app), ShellFocusState::NoFocus);
    assert_eq!(state(&app).focused_entity(), None);
    assert_eq!(official_focus(&app), None);
}

#[test]
fn mounted_page_buttons_are_reachable_without_a_shell_marker() {
    let mut app = mounted_shell();
    // Spawn the lower row first so the order cannot inherit the query order.
    let bottom = measured(&mut app, -100.0, 100.0);
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
    let ordered_targets: Vec<_> = app
        .world()
        .resource::<ShellFocusOrder>()
        .targets
        .iter()
        .map(|target| target.entity)
        .collect();
    assert_eq!(
        ordered_targets,
        vec![marked, page_button, bottom],
        "the focus order uses top-to-bottom, then left-to-right screen order"
    );
    assert_eq!(official_focus(&app), Some(marked));
    assert_eq!(state(&app), ShellFocusState::Focused(marked));
}
