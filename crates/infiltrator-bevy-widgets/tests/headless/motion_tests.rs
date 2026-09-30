//! Headless tests for spring physics and animation systems in `infiltrator-bevy-widgets`.

use std::time::Duration;

use bevy::app::{App, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use bevy::picking::hover::PickingInteraction;
use bevy::time::{Time, TimePlugin};
use bevy::transform::components::Transform;
use bevy::ui::prelude::{Node, Val};
use infiltrator_bevy_widgets::motion::{
    SpringAnimationPlugin, SpringButtonPop, SpringRouteTransition, SpringToggleKnob,
    sync_spring_button_pop,
};

#[test]
fn test_spring_animation_plugin_systems_execution() {
    let mut app = App::new();
    app.add_plugins(TimePlugin);
    app.add_plugins(SpringAnimationPlugin);

    // Advance simulated time before spring systems run
    app.add_systems(
        Update,
        (|mut time: ResMut<Time>| {
            time.advance_by(Duration::from_millis(50));
        })
        .before(sync_spring_button_pop),
    );

    // 1. Button Pop
    let btn = app
        .world_mut()
        .spawn((
            SpringButtonPop::default(),
            Transform::default(),
            PickingInteraction::Pressed,
        ))
        .id();

    // 2. Toggle Knob
    let knob = app
        .world_mut()
        .spawn((
            SpringToggleKnob::new(0.0, 20.0, true),
            Node::default(),
        ))
        .id();

    // 3. Route Transition
    let route = app
        .world_mut()
        .spawn((
            SpringRouteTransition::new(8.0),
            Transform::default(),
        ))
        .id();

    // Initialize app
    app.update();

    // Button should compress towards 0.96
    let btn_transform = app.world().get::<Transform>(btn).unwrap();
    assert!(btn_transform.scale.x < 1.0);

    // Knob should advance towards 20.0
    let knob_node = app.world().get::<Node>(knob).unwrap();
    match knob_node.left {
        Val::Px(x) => assert!(x > 0.0),
        _ => panic!("Expected Val::Px for knob.left"),
    }

    // Route transition should move towards 0.0
    let route_transform = app.world().get::<Transform>(route).unwrap();
    assert!(route_transform.translation.y < 8.0);
}
