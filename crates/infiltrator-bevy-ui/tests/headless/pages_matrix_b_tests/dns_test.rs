//! Behavior cases for dns test.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_dns_test_latency_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TestDnsLatencyButton>>()
        .single(app.world())
        .expect("test dns latency button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::TestDnsLatency]);
}

#[test]
fn test_dns_test_leak_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TestDnsLeakButton>>()
        .single(app.world())
        .expect("test dns leak button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::TestDnsLeak]);
}
