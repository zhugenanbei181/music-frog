//! Behavior cases for profiles snapshot.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::pages::profiles_diff::SnapshotDiffViewState;
use infiltrator_bevy_ui::pages::snapshot_restore::RestoreState;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;

#[test]
fn test_profiles_snapshot_diff_renders_shared_rows_and_confirms_rollback() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(snapshot_diff_page_projection(
            Some(snapshot_diff_fixture()),
        )));
    app.update();

    assert!(
        subtree_has_text(
            app.world(),
            root,
            "对比 snapshot-1735489200000 → current-profile · +1 -1 ~1 · 保真通过 L3-Anchors"
        ),
        "the summary restamps from the shared diff snapshot"
    );
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "rules: [DOMAIN-SUFFIX,google.com,DIRECT]"
        ),
        "the inline rows come from the shared unified lines"
    );
    assert!(
        subtree_has_text(app.world(), root, "远程订阅 · 只读保护"),
        "the profile card renders the shared write protection"
    );

    let refresh = marker_entity::<RefreshSnapshotDiffButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: refresh });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadSnapshotDiff { snapshot_id: None }),
        "refresh asks the shared application for the newest snapshot diff"
    );

    let split = {
        let mut query = app.world_mut().query::<(Entity, &SnapshotDiffModeButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.split)
            .map(|(entity, _)| entity)
            .expect("split mode button")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: split });
    app.update();
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "   4|     ~ tun: { enable: false, stack: gvisor }"
        ),
        "the split layout renders aligned line numbers from the shared split rows"
    );

    let rollback = marker_entity::<RollbackSnapshotButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: rollback });
    app.update();
    let submitted = sink.submitted();
    assert!(
        matches!(submitted.last(), Some(UiCommand::SnapshotRestore { intent: CommandIntent::PrepareSnapshotRestore { target } }) if target.snapshot_id == "/fake/configs/main-history/snap-001.yaml")
    );
    assert!(app.world().resource::<RestoreState>().model.visible);
    let count = submitted.len();
    app.world_mut().trigger(Activate { entity: rollback });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        count,
        "repeating the launcher cannot confirm a restoration"
    );
    assert!(
        !sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::SnapshotRestore {
                intent: CommandIntent::ConfirmSnapshotRestore { .. }
            }
        )),
        "only the independent confirmation surface can commit"
    );
}

#[test]
fn test_profiles_snapshot_diff_states_are_honest_without_a_diff() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(snapshot_diff_page_projection(
            None,
        )));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "尚未计算快照差异"),
        "no diff means an honest prompt, never fabricated rows"
    );

    let identical = YamlAstDiffSnapshot::demo_fixture();
    let identical = YamlAstDiffSnapshot {
        stats: Default::default(),
        unified_lines: Vec::new(),
        split_rows: Vec::new(),
        ..identical
    };
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(snapshot_diff_page_projection(
            Some(identical),
        )));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "快照与当前配置内容一致"),
        "an identical diff reports equality instead of a fake change"
    );
}

#[test]
fn test_profiles_snapshot_history_lists_prunes_and_backs_up_through_the_shared_app() {
    use infiltrator_bevy_ui::pages::profiles_diff_history::{
        BackupSnapshotButton, PruneSnapshotsButton, RefreshSnapshotHistoryButton,
        SnapshotHistoryEntryButton, SnapshotPruneKeepButton,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut projection = snapshot_diff_page_projection(None);
    projection.snapshot_history = Some(snapshot_history_fixture());
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "main · 2 份快照（上限 20）· 待修剪 1 份"),
        "the summary comes from the shared history snapshot"
    );
    assert!(
        subtree_has_text(app.world(), root, "重复内容"),
        "the duplicated entry is marked from the shared prune view"
    );

    let keep = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &SnapshotPruneKeepButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.keep == 10)
            .map(|(entity, _)| entity)
            .expect("keep preset 10")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: keep });
    app.update();
    assert_eq!(
        app.world().resource::<SnapshotDiffViewState>().prune_keep,
        10,
        "the retention preset is surface view state, the prune itself is shared"
    );

    let prune = marker_entity::<PruneSnapshotsButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: prune });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PruneSnapshots { keep: Some(10) }),
        "prune submits the shared dedupe+LRU command"
    );

    let backup = marker_entity::<BackupSnapshotButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: backup });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::CreateBackupSnapshot),
        "manual backup uses the shared snapshot application"
    );

    let refresh = marker_entity::<RefreshSnapshotHistoryButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: refresh });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadSnapshotHistory),
        "refresh reloads the shared history read model"
    );

    let entry = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &SnapshotHistoryEntryButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.id.ends_with("snap-001.yaml"))
            .map(|(entity, _)| entity)
            .expect("history entry row")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: entry });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadSnapshotDiff {
            snapshot_id: Some("/fake/configs/main-history/snap-001.yaml".to_owned()),
        }),
        "selecting a history entry diffs that exact snapshot through the shared app"
    );
    assert_eq!(
        app.world()
            .resource::<SnapshotDiffViewState>()
            .selected_snapshot
            .as_deref(),
        Some("/fake/configs/main-history/snap-001.yaml"),
        "the surface remembers which entry it asked to diff"
    );
}

/// DUAL-09-14: the history card's per-entry restore is the same two-step
/// confirmed action the Iced panel offers (first click arms, second submits).
#[test]
fn test_profiles_snapshot_history_entry_restore_is_armed_before_it_executes() {
    use infiltrator_bevy_ui::pages::profiles_diff_history::SnapshotHistoryRestoreButton;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    let mut projection = snapshot_diff_page_projection(None);
    projection.snapshot_history = Some(snapshot_history_fixture());
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    let restore = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &SnapshotHistoryRestoreButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.id.ends_with("snap-001.yaml"))
            .map(|(entity, _)| entity)
            .expect("restore button")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: restore });
    app.update();
    assert!(app.world().resource::<RestoreState>().model.visible);
    assert!(
        matches!(sink.submitted().last(), Some(UiCommand::SnapshotRestore { intent: CommandIntent::PrepareSnapshotRestore { target } }) if target.profile == "main" && target.snapshot_id.ends_with("snap-001.yaml"))
    );
    let count = sink.submitted().len();
    app.world_mut().trigger(Activate { entity: restore });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        count,
        "the history launcher never doubles as confirm"
    );
    assert!(!sink.submitted().iter().any(|command| matches!(
        command,
        UiCommand::SnapshotRestore {
            intent: CommandIntent::ConfirmSnapshotRestore { .. }
        }
    )));
}

#[test]
fn snapshot_locale_replay_preserves_all_history_rows_native_entities_selection_and_source_identity()
{
    use infiltrator_bevy_ui::pages::profiles_diff_history::{
        SnapshotEntryLabel, SnapshotHistoryEntryButton,
    };
    use infiltrator_bevy_widgets::localization::UiLocale;
    use infiltrator_shared::locales::{Lang, Localizer};
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);
    let mut projection = snapshot_diff_page_projection(Some(snapshot_diff_fixture()));
    let mut history = snapshot_history_fixture();
    let row = history.entries[0].clone();
    history.entries = (0..24)
        .map(|index| {
            let mut row = row.clone();
            row.id = format!("/用户/{{id}}/{index}.yaml");
            row.sha256 = format!("{index:064x}");
            row.is_newest = index == 0;
            row
        })
        .collect();
    let selected = history.entries[21].id.clone();
    projection.snapshot_history = Some(history);
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let controls: Vec<_> = app
        .world_mut()
        .query::<(Entity, &SnapshotHistoryEntryButton)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.id.clone()))
        .collect();
    assert_eq!(
        controls.len(),
        24,
        "every shared row is reachable, including rows beyond twelve"
    );
    let labels: Vec<_> = app
        .world_mut()
        .query::<(Entity, &SnapshotEntryLabel)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.0.id.clone()))
        .collect();
    app.world_mut()
        .resource_mut::<SnapshotDiffViewState>()
        .selected_snapshot = Some(selected.clone());
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "Compare snapshot-1735489200000 → current-profile"
    ));
    assert!(subtree_has_text(app.world(), root, "main · 24 snapshots"));
    assert!(subtree_has_text(
        app.world(),
        root,
        Lang("en-US").tr("snapshot_diff_inline").as_ref()
    ));
    for (entity, id) in &labels {
        assert_eq!(
            &app.world().get::<SnapshotEntryLabel>(*entity).unwrap().0.id,
            id
        );
        assert!(app.world().get::<Text>(*entity).unwrap().0.contains("Diff"));
    }
    for (entity, id) in &controls {
        assert_eq!(
            &app.world()
                .get::<SnapshotHistoryEntryButton>(*entity)
                .unwrap()
                .id,
            id
        );
    }
    assert_eq!(
        app.world()
            .resource::<SnapshotDiffViewState>()
            .selected_snapshot
            .as_deref(),
        Some(selected.as_str())
    );
    assert!(
        sink.submitted().is_empty(),
        "changing locale never submits a business command"
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("zh-CN");
    app.update();
    for (entity, id) in labels {
        assert_eq!(
            app.world().get::<SnapshotEntryLabel>(entity).unwrap().0.id,
            id
        );
    }
}
