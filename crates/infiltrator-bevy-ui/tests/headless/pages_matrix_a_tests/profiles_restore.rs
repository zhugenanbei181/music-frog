//! Behavior cases for profiles restore.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_profiles_restore_backup_submits_shared_command_and_restamps_status() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "安全备份已就绪"),
        "available backup reaches the status line"
    );

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<RestoreSubscriptionBackupButton>>()
        .single(app.world())
        .expect("restore backup button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RestoreSubscriptionBackup {
            id: "sub-fetch".to_owned(),
        }]
    );

    let mut no_backup = subscription_fetch_projection();
    no_backup.profiles[0].has_backup = false;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(no_backup));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "安全备份：暂无"),
        "missing backup restamps the status line"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<SubscriptionBackupStatus>>()
            .iter(app.world())
            .next()
            .is_some(),
        "backup status marker is mounted"
    );
}
