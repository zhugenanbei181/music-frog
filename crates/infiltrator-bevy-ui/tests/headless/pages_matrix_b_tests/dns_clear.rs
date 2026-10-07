//! Behavior cases for dns clear.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::dns_cache::{CacheAction, CacheConfirmation};

#[test]
fn test_dns_clear_cache_requires_an_independent_confirmation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ClearDnsCacheButton>>()
        .single(app.world())
        .expect("clear dns cache button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert!(sink.submitted().is_empty());
    assert!(app.world().resource::<CacheConfirmation>().model.open);
    let confirm = app
        .world_mut()
        .query::<(Entity, &CacheAction)>()
        .iter(app.world())
        .find(|(_, action)| matches!(action, CacheAction::Confirm))
        .unwrap()
        .0;
    app.world_mut().trigger(Activate { entity: confirm });
    app.update();
    let model = &app.world().resource::<CacheConfirmation>().model;
    assert!(model.pending.is_some());
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ClearDnsCache {
            operation: model.requested.unwrap()
        }]
    );
}
