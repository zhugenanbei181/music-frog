//! Behavior cases for doctor.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_contract::doctor::DoctorStatus;
use infiltrator_contract::surface_snapshot::PageStatus;

#[test]
fn test_doctor_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Doctor);

    assert!(subtree_has_text(
        app.world(),
        root,
        "诊断结果 · 通过 6 · 警告 0 · 失败 0 · 跳过 0"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "TUN 虚拟网卡与路由表健康度"
    ));
    assert!(subtree_has_text(app.world(), root, "通过"));
    assert!(subtree_has_text(app.world(), root, "立即诊断"));
    assert!(subtree_has_text(app.world(), root, "一键修复"));
}

#[test]
fn test_doctor_run_diagnostics_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Doctor);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<RunDoctorDiagnosticsButton>>()
        .single(app.world())
        .expect("run doctor button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RunDoctorDiagnostics]);
}

#[test]
fn test_doctor_repair_all_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Doctor);

    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.pages.doctor.status = PageStatus::Ready;
    snapshot.pages.doctor.data.as_mut().unwrap().checks[0].state = DoctorStatus::Fail;
    snapshot.pages.doctor.data.as_mut().unwrap().checks[0].fix_available = true;
    snapshot.revision += 1;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<RepairAllDoctorButton>>()
        .single(app.world())
        .expect("repair all doctor button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RepairAllDoctorIssues]);
}

#[test]
fn test_doctor_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Doctor);

    let mut updated = DoctorProjection::demo();
    updated.overall_healthy = false;
    updated.last_run = "2026-09-02 12:00:00".to_owned();
    updated.checks[0].state = DoctorStatus::Fail;
    updated.checks[0].detail_copy_key = None;
    updated.checks[0].detail = "TUN 接口 utun9 意外掉线".to_owned();
    updated.watchdog = CoreWatchdogSnapshot {
        state: CoreWatchdogState::Waiting {
            attempt: 2,
            retry_in_ms: 200,
        },
        session_token: None,
        consecutive_failures: 2,
        last_error: None,
    };

    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "诊断结果 · 通过 5 · 警告 0 · 失败 1 · 跳过 0"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "最近诊断: 2026-09-02 12:00:00"
    ));
    assert!(subtree_has_text(app.world(), root, "失败"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "核心看门狗：第 2 次重启将在 200 ms 后执行"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "TUN 接口 utun9 意外掉线"
    ));
}
