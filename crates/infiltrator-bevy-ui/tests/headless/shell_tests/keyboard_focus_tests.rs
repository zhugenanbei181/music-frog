//! BEVY-020.2: the shell's visible focus follows the official input focus.
//!
//! The official `bevy_input_focus::InputFocus` is the only focus truth: the
//! shell's ring and the widget `Focused` marker are derived from it, and the
//! official directional navigation (the arrow keys) is what moves it. The
//! headless rig has no UI camera, so the official *automatic* neighbour
//! search has no layout to measure; the official manual navigation map states
//! the hop instead — the official channel for stating a navigation edge.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::directional_navigation::DirectionalNavigationMap;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, InputFocus};
use bevy::math::{CompassOctant, Vec2};
use bevy::scene::ScenePlugin;
use bevy::ui::auto_directional_navigation::AutoDirectionalNavigation;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::ui_widgets::Button;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::shell_focus::{ShellFocusRing, ShellFocusState, ShellFocusable};
use infiltrator_bevy_widgets::focus::Focused;

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

/// The officially focused entity, as the official focus resource reports it.
fn official_focus(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

/// The control the focus ring decorates, if the ring is drawn.
fn ring_target(app: &mut App) -> Option<Entity> {
    app.world_mut()
        .query::<&ShellFocusRing>()
        .single(app.world())
        .ok()
        .map(|ring| ring.0)
}

#[test]
fn the_tab_gesture_moves_the_official_focus_and_the_ring_follows() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();

    // The route-only baseline already focuses the first control.
    assert_eq!(official_focus(&app), Some(left));
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
        official_focus(&app),
        Some(right),
        "the Tab gesture hands the next control to the official focus"
    );
    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(right)
    );
    assert_eq!(
        ring_target(&mut app),
        Some(right),
        "the ring follows the official focus"
    );
    assert!(
        app.world().get::<Focused>(right).is_some(),
        "the widget focus marker sits on the officially focused control"
    );
    assert!(app.world().get::<Focused>(left).is_none());
}

#[test]
fn shift_tab_moves_the_official_focus_backwards() {
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
        official_focus(&app),
        Some(right),
        "Shift+Tab wraps to the next control"
    );

    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::ShiftLeft);
    keyboard.press(KeyCode::Tab);
    app.insert_resource(keyboard);
    app.update();

    assert_eq!(
        official_focus(&app),
        Some(left),
        "Shift+Tab steps back through the focus order"
    );
    assert_eq!(ring_target(&mut app), Some(left));
}

#[test]
fn the_arrow_gesture_moves_the_official_focus_and_the_ring_follows() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    let right = measured(&mut app, 100.0, 0.0);
    app.update();
    assert_eq!(official_focus(&app), Some(left));

    // The official map states the hop; the official navigator moves the
    // official input focus along it.
    app.world_mut()
        .resource_mut::<DirectionalNavigationMap>()
        .add_edge(left, right, CompassOctant::East);

    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::ArrowRight);
    app.insert_resource(keyboard);
    app.update();

    assert_eq!(
        official_focus(&app),
        Some(right),
        "the arrow gesture moves the official input focus"
    );
    assert_eq!(
        *app.world().resource::<ShellFocusState>(),
        ShellFocusState::Focused(right)
    );
    assert_eq!(
        ring_target(&mut app),
        Some(right),
        "the visible focus ring follows the official focus"
    );
    assert!(app.world().get::<Focused>(right).is_some());
    assert!(app.world().get::<Focused>(left).is_none());
}

#[test]
fn the_route_only_baseline_stays_invisible() {
    let mut app = mounted_shell();
    let left = measured(&mut app, 0.0, 0.0);
    app.update();

    assert_eq!(
        official_focus(&app),
        Some(left),
        "the shell lands the official focus on the first measured control"
    );
    assert_eq!(
        ring_target(&mut app),
        None,
        "the automatic landing is the route-only baseline, not a focus the user moved"
    );
    assert!(
        app.world().get::<Focused>(left).is_some(),
        "the widget focus marker still marks the focused control"
    );
}

#[test]
fn the_focusable_controls_carry_the_official_focus_components() {
    let mut app = mounted_shell();
    let control = measured(&mut app, 0.0, 0.0);
    let button = app.world_mut().spawn(Button).id();

    assert!(
        app.world().get::<TabIndex>(control).is_some(),
        "a shell focusable is an official focus target"
    );
    assert!(
        app.world()
            .get::<AutoDirectionalNavigation>(control)
            .is_some(),
        "the official directional navigation may reach a shell focusable"
    );
    assert!(
        app.world().get::<TabIndex>(button).is_some(),
        "a mounted button is an official focus target too"
    );
}

#[test]
fn the_official_focus_reaches_a_control_through_the_official_request() {
    let mut app = mounted_shell();
    let _baseline = measured(&mut app, 0.0, 0.0);
    let control = measured(&mut app, 100.0, 0.0);
    app.update();

    // The official pointer focus asks for the focus with an `AcquireFocus`
    // request; the official resolver takes it because the control carries the
    // official `TabIndex`.
    app.world_mut().trigger(AcquireFocus {
        focused_entity: control,
        window: control,
    });
    app.update();

    assert_eq!(
        official_focus(&app),
        Some(control),
        "the official focus request reaches a control that carries a TabIndex"
    );
    assert_eq!(
        ring_target(&mut app),
        Some(control),
        "the visible focus follows the official focus"
    );
}
