//! Behavior cases for profiles schedule.
//! test-intent: behavior

use super::*;
use infiltrator_contract::subscription_import::SubscriptionScheduleDraft;

/// DUAL-07-14: the update-policy card is driven by the shared projection and
/// submits the whole schedule draft through the shared command bus.
#[test]
fn test_profiles_schedule_policy_restamps_and_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut projection = subscription_fetch_projection();
    projection.profiles[0].url = "https://fetch.example/sub".to_owned();
    projection.profiles[0].auto_update_enabled = true;
    projection.profiles[0].update_interval_hours = Some(12);
    projection.profiles[0].auto_reload_core = true;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "订阅更新策略 (Update Policy)"),
        "the policy card mounts on the profiles page"
    );
    assert!(
        subtree_has_text(app.world(), root, "定时计划：Cron `0 */6 * * *`"),
        "the status line restamps from the shared snapshot"
    );
    assert!(
        subtree_has_text(app.world(), root, "保存更新策略"),
        "the policy save action exists"
    );

    // The interval field is prefilled from the snapshot, then edited.
    set_marker_text::<SubscriptionPolicyIntervalField>(&mut app, "6");
    set_marker_text::<SubscriptionPolicyCronField>(&mut app, "");
    let save = marker_entity::<SaveSubscriptionPolicyButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSubscriptionSchedule {
            profile_id: "sub-fetch".to_owned(),
            draft: SubscriptionScheduleDraft {
                url: "https://fetch.example/sub".to_owned(),
                auto_update_enabled: true,
                update_interval_hours: "6".to_owned(),
                cron_expression: None,
            },
        }],
        "the edited schedule rides the shared command"
    );
}
