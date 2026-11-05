//! Headless tests for spring physics and animation systems in `infiltrator-bevy-widgets`.

use bevy::app::{App, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use bevy::time::{Time, TimePlugin};
use bevy::transform::components::Transform;
use infiltrator_bevy_widgets::motion::{
    SpringAnimationPlugin, SpringRouteTransition, sync_spring_route_transition,
};
use std::time::Duration;

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
        .before(sync_spring_route_transition),
    );

    // The plugin's one live system: the route entry spring.
    let route = app
        .world_mut()
        .spawn((SpringRouteTransition::new(8.0), Transform::default()))
        .id();

    // Initialize app
    app.update();

    // Route transition should move towards 0.0
    let route_transform = app.world().get::<Transform>(route).unwrap();
    assert!(route_transform.translation.y < 8.0);
}
