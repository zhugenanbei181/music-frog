//! Behavior cases for logs.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::logs_rows::LogRowIdentity;
use infiltrator_bevy_ui::pages::logs_virtual::{LogsVirtualNodes, LogsVirtualSlot};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::surface_snapshot::PageStatus;

#[test]
fn test_logs_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 5 行日志"
    ));
    assert!(subtree_has_text(app.world(), root, "清空"));
    assert!(subtree_has_text(app.world(), root, "滚屏锁定"));
    assert!(subtree_has_text(app.world(), root, "导出日志"));
    assert!(subtree_has_text(app.world(), root, "DEBUG"));
    assert!(subtree_has_text(app.world(), root, "INFO"));
    assert!(subtree_has_text(app.world(), root, "WARN"));
    assert!(subtree_has_text(app.world(), root, "ERROR"));
    assert!(subtree_has_text(app.world(), root, "[INFO]"));
    assert!(subtree_has_text(app.world(), root, "[WARN]"));
    assert!(subtree_has_text(app.world(), root, "[ERROR]"));
    assert!(
        app.world_mut()
            .query::<&PauseLogsButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ExportLogsButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_logs_clear_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Logs);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<ClearLogsButton>>()
        .single(app.world())
        .expect("clear logs button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ClearLogs]);
}

#[test]
fn test_logs_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    let mut updated = LogsProjection::demo();
    updated.total_entries = 100;
    updated.entries[0].message = "[TCP] connection reset by peer in test".to_owned();
    updated.entries[0].level = LogLevel::Error;
    updated.entries[0].timestamp = "11:22:33.444".to_owned();

    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 100 行日志"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "[TCP] connection reset by peer in test"
    ));
    assert!(subtree_has_text(app.world(), root, "11:22:33.444"));
}

#[test]
fn test_logs_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    let empty = LogsProjection {
        status: PageStatus::Ready,
        generation: 0,
        session_token: None,
        total_entries: 0,
        active_level: None,
        entries: vec![],
    };
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 0 行日志"
    ));
}

#[test]
fn log_rows_grow_from_empty_shrink_and_preserve_entities_for_stable_records() {
    let mut app = setup_matrix_a_app(Arc::new(DemoCommandSink::accepting()));
    navigate_to(&mut app, Route::Logs);
    let empty = LogsProjection {
        status: PageStatus::Ready,
        generation: 0,
        session_token: None,
        total_entries: 0,
        active_level: None,
        entries: Vec::new(),
    };
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(empty));
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&LogMessageText>()
            .iter(app.world())
            .count(),
        0
    );
    let mut growing = LogsProjection::demo();
    for index in 5..12 {
        let mut entry = growing.entries[0].clone();
        entry.id = index as u64 + 1;
        entry.message = format!("streamed row {index}");
        growing.entries.push(entry);
    }
    growing.total_entries = growing.entries.len();
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(growing.clone()));
    app.update();
    let before: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<LogMessageText>>()
        .iter(app.world())
        .collect();
    assert_eq!(before.len(), 12);
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "streamed row 11")
    );
    growing.entries[0].message = "same identity, fresh copy".into();
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(growing.clone()));
    app.update();
    let after: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<LogMessageText>>()
        .iter(app.world())
        .collect();
    assert_eq!(before, after);
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "same identity, fresh copy")
    );
    growing.entries.drain(..11);
    growing.total_entries = 1;
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(growing));
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&LogMessageText>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "streamed row 11")
    );
}

/// BANDROID-010: a 50,000-entry stream mounts through the recycler window, not
/// the whole ring buffer; the window slides to the tail and the recycled slot
/// entities stay stable across the scroll.
#[test]
fn test_logs_large_stream_mounts_a_bounded_recycled_window() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);

    // Publish a 50k-entry stream before the page mounts, so the large-list path
    // is selected from the very first scene build.
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    let page = snapshot.pages.logs.data.as_mut().unwrap();
    let template = page.entries[0].clone();
    page.entries = (0..50_000)
        .map(|index| {
            let mut entry = template.clone();
            entry.id = index as u64;
            entry.message = format!("streamed row {index}");
            entry
        })
        .collect();
    page.total_entries = 50_000;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    navigate_to(&mut app, Route::Logs);
    app.update();
    app.update();

    let mounted = app
        .world_mut()
        .query::<&LogRowIdentity>()
        .iter(app.world())
        .count();
    assert!(mounted > 0, "the recycler must mount the visible window");
    assert!(
        mounted < 50_000,
        "the full 50k ring buffer must not be mounted, got {mounted}"
    );
    assert!(
        mounted <= 60,
        "mounted rows must be viewport-bounded, got {mounted}"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<LogsVirtualNodes>>()
            .iter(app.world())
            .next()
            .is_some(),
        "the virtual container must own the large list"
    );

    // Stable slot identity: the pre-spawned slot entities are reused, never
    // respawned, as the window slides.
    let slots_before: Vec<Entity> = {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<LogsVirtualSlot>>();
        let mut slots: Vec<Entity> = query.iter(app.world()).collect();
        slots.sort();
        slots
    };
    assert!(!slots_before.is_empty(), "the pool must own slot entities");

    // Scrolling to the bottom slides the window to the tail row.
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<LogsPageRoot>>()
        .single(app.world())
        .expect("logs page root");
    app.world_mut()
        .get_mut::<ScrollPosition>(root)
        .expect("scroll position")
        .0
        .y = 1.0e9;
    app.update();
    assert!(
        app.world_mut()
            .query::<&LogRowIdentity>()
            .iter(app.world())
            .any(|identity| identity.0 == 49_999),
        "the tail log entry must become reachable"
    );
    let slots_after: Vec<Entity> = {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<LogsVirtualSlot>>();
        let mut slots: Vec<Entity> = query.iter(app.world()).collect();
        slots.sort();
        slots
    };
    assert_eq!(
        slots_before, slots_after,
        "scrolling must reuse the same slot entities"
    );
}
