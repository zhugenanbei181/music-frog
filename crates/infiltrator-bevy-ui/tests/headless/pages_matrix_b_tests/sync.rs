//! Behavior cases for sync.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;

#[test]
fn test_sync_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Sync);

    assert!(subtree_has_text(
        app.world(),
        root,
        "数据同步 · 已连接 · 同步就绪"
    ));
    assert!(subtree_has_text(app.world(), root, "立即同步"));
    assert!(subtree_has_text(app.world(), root, "创建备份"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "https://dav.jianguoyun.com/dav/MusicFrog/"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Linux Desktop (CachyOS) · 2026-09-02 10:15"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "字段级三向冲突差异合并 (3-Way Merge & Conflict Resolver)"
    ));
    assert!(subtree_has_text(app.world(), root, "智能合并两者"));
}

#[test]
fn test_sync_now_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<SyncNowButton>>()
        .single(app.world())
        .expect("sync now button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::SyncNow]);
}

#[test]
fn test_sync_create_backup_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<CreateBackupButton>>()
        .single(app.world())
        .expect("create backup button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::CreateBackupSnapshot]);
}

#[test]
fn test_sync_conflict_state_and_resolution_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Sync);

    let mut conflict_proj = SyncProjection::demo();
    conflict_proj.status = SyncStatus::Conflict;
    conflict_proj.conflict = Some(SyncConflictInfo {
        remote_device: "Android (Pixel 9 Pro)".to_owned(),
        conflict_time: "2026-09-02 10:14".to_owned(),
        conflicting_keys: vec![ConflictingKey {
            key: "mode".to_owned(),
            local_value: "Rule".to_owned(),
            remote_value: "Global".to_owned(),
        }],
    });

    app.world_mut()
        .commands()
        .trigger(SyncProjectionUpdated(conflict_proj));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "数据同步 · 同步冲突 · 需要手动解决"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "检测到冲突：远端设备 Android (Pixel 9 Pro) 于 2026-09-02 10:14 产生变更，共 1 处不一致"
    ));

    // Test keep local button
    let keep_local_entity = app
        .world_mut()
        .query_filtered::<Entity, With<KeepLocalConflictButton>>()
        .single(app.world())
        .expect("keep local button");

    app.world_mut().commands().trigger(Activate {
        entity: keep_local_entity,
    });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ResolveConflictKeepLocal]);

    // Test take remote button
    sink.clear();
    let take_remote_entity = app
        .world_mut()
        .query_filtered::<Entity, With<TakeRemoteConflictButton>>()
        .single(app.world())
        .expect("take remote button");

    app.world_mut().commands().trigger(Activate {
        entity: take_remote_entity,
    });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ResolveConflictTakeRemote]);
}

#[test]
fn test_sync_restore_snapshot_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let mut query = app.world_mut().query::<(Entity, &RestoreSnapshotButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.snapshot_id == "snap-1")
        .expect("snap-1 restore button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SnapshotRestore {
            intent: CommandIntent::PrepareSnapshotRestore {
                target: SnapshotRestoreTarget {
                    profile: "main".into(),
                    snapshot_id: "snap-1".into()
                }
            }
        }]
    );
}
