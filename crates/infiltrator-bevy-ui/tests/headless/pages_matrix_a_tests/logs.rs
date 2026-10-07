//! Behavior cases for logs.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
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
