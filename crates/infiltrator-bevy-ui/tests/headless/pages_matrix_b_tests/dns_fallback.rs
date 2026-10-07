//! Behavior cases for dns fallback.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_dns_fallback_policy_toggle_and_trigger_submit_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    // Demo fixture starts with git-ip fallback geoip enabled; toggling flips it.
    let toggle_before = app
        .world_mut()
        .query::<&DnsEditGeoipToggle>()
        .single(app.world())
        .copied()
        .expect("geoip toggle");
    let toggle_entity = app
        .world_mut()
        .query_filtered::<Entity, With<DnsEditGeoipToggle>>()
        .single(app.world())
        .expect("geoip toggle entity");
    app.world_mut().commands().trigger(Activate {
        entity: toggle_entity,
    });
    app.update();

    set_dns_field(
        &mut app,
        DnsFormField::FallbackTriggerIp,
        "240.0.0.0/4, 10.0.0.0/8",
    );
    trigger_dns_edit_apply(&mut app);

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let policy = patch.fallback_policy.as_ref().expect("fallback policy");
            assert_eq!(policy.geoip, !toggle_before.0);
            assert_eq!(
                policy.trigger_ipcidr,
                vec!["240.0.0.0/4".to_owned(), "10.0.0.0/8".to_owned()]
            );
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}
