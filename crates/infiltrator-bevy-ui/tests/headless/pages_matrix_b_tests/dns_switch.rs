//! Behavior cases for dns switch.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_dns_switch_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let switch_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DnsSwitchButton>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsSwitchButton>(*entity),
                Some(DnsSwitchButton(DnsSwitchField::Enable, _))
            )
        })
        .expect("enable dns switch button");

    app.world_mut().commands().trigger(Activate {
        entity: switch_entity,
    });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::ApplyDnsSettings { patch } => {
            let switches = patch
                .switches
                .expect("switch patch must carry the full set");
            assert!(!switches.enable, "demo enable=true toggles to false");
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_switch_patch_stays_full_after_shared_form_upgrade() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let switch_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DnsSwitchButton>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsSwitchButton>(*entity),
                Some(DnsSwitchButton(DnsSwitchField::RespectRules, _))
            )
        })
        .expect("respect_rules switch");
    app.world_mut().commands().trigger(Activate {
        entity: switch_entity,
    });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let switches = patch.switches.expect("switch set");
            assert!(switches.respect_rules);
            assert!(switches.enable);
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}
