//! Behavior cases for dns enhanced.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_dns_enhanced_mode_pill_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let pill_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DnsEnhancedModePill>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsEnhancedModePill>(*entity),
                Some(DnsEnhancedModePill(DnsEnhancedMode::RedirHost))
            )
        })
        .expect("redir-host pill");

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            assert_eq!(patch.enhanced_mode, Some(DnsEnhancedMode::RedirHost));
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}
