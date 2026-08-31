//! Doctor 体检面板的消息处理与状态迁移（headless：单测从不发起真实 HTTP，
//! 只驱动 `Result` 消息；demo 拦截在 dispatcher 层验证）。
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::types::doctor::{
    BootstrapReport, BootstrapStep, DoctorCheckResult, DoctorFixAction, DoctorFixReport,
    DoctorReport, DoctorStatus,
};
use crate::types::message::Message;
use infiltrator_core::error::InfiltratorError;

fn check(id: &str, status: DoctorStatus) -> DoctorCheckResult {
    DoctorCheckResult {
        id: id.to_string(),
        category: "config".to_string(),
        status,
        summary: format!("summary of {id}"),
        detail: None,
        hint: Some(format!("hint for {id}")),
    }
}

fn report(checks: Vec<DoctorCheckResult>) -> DoctorReport {
    DoctorReport {
        started_at: 1,
        finished_at: 2,
        checks,
    }
}

#[test]
fn test_run_doctor_sets_busy_flag_and_report_ready_stores_report() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunDoctor);
    assert!(state.diag.doctor.is_running, "RunDoctor arms the panel");
    assert!(state.diag.doctor.error.is_none());

    let sample = report(vec![
        check("config.settings_parse", DoctorStatus::Pass),
        check("config.configs_dir", DoctorStatus::Warn),
        check("process.pidfile", DoctorStatus::Fail),
        check("network.controller", DoctorStatus::Skip),
    ]);
    let _ = state.update(Message::DoctorReportReady(Ok(sample)));

    assert!(!state.diag.doctor.is_running);
    let stored = state.diag.doctor.report.as_ref().expect("report stored");
    assert_eq!(stored.checks.len(), 4);
    assert_eq!(stored.count_by_status(DoctorStatus::Pass), 1);
    assert_eq!(stored.count_by_status(DoctorStatus::Warn), 1);
    assert_eq!(stored.count_by_status(DoctorStatus::Fail), 1);
    assert_eq!(stored.count_by_status(DoctorStatus::Skip), 1);
}

#[test]
fn test_run_doctor_is_guarded_while_busy() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunDoctor);
    // busy 期间重复触发不改变状态（节流），也不会清掉在途标记。
    let _ = state.update(Message::RunDoctor);
    assert!(state.diag.doctor.is_running);
}

#[test]
fn test_doctor_report_error_surfaces_in_panel_and_clears_busy() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunDoctor);
    let _ = state.update(Message::DoctorReportReady(Err(InfiltratorError::Internal(
        "connection refused".to_string(),
    ))));

    assert!(!state.diag.doctor.is_running);
    assert!(
        state
            .diag
            .doctor
            .error
            .as_deref()
            .is_some_and(|e| e.contains("connection refused")),
        "panel error must surface the transport failure"
    );
    assert!(state.diag.doctor.report.is_none());
}

#[test]
fn test_doctor_report_error_keeps_previous_report_visible() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::DoctorReportReady(Ok(report(vec![check(
        "config.settings_parse",
        DoctorStatus::Pass,
    )]))));
    // 刷新失败（服务已停）时保留上一份报告，仅置错误位。
    let _ = state.update(Message::RunDoctor);
    let _ = state.update(Message::DoctorReportReady(Err(InfiltratorError::Internal(
        "closed".to_string(),
    ))));
    assert!(
        state.diag.doctor.report.is_some(),
        "previous report must stay visible"
    );
    assert!(state.diag.doctor.error.is_some());
}

#[test]
fn test_doctor_fix_transitions_and_clears_busy() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunDoctorFix);
    assert!(state.diag.doctor.is_fixing);

    let fixed = DoctorFixReport {
        actions: vec![DoctorFixAction {
            id: "config.configs_dir".to_string(),
            summary: "created configs dir".to_string(),
        }],
    };
    let _ = state.update(Message::DoctorFixApplied(Ok(fixed)));
    assert!(!state.diag.doctor.is_fixing);
    assert!(state.diag.doctor.error.is_none());
}

#[test]
fn test_doctor_fix_error_sets_panel_error() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunDoctorFix);
    let _ = state.update(Message::DoctorFixApplied(Err(InfiltratorError::Internal(
        "fix failed".to_string(),
    ))));
    assert!(!state.diag.doctor.is_fixing);
    assert!(state.diag.doctor.error.is_some());
}

#[test]
fn test_bootstrap_transitions_and_clears_busy() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RunBootstrap);
    assert!(state.diag.doctor.is_bootstrapping);

    let bootstrapped = BootstrapReport {
        steps: vec![BootstrapStep {
            id: "configs_dir".to_string(),
            executed: true,
            detail: "created".to_string(),
        }],
    };
    let _ = state.update(Message::BootstrapFinished(Ok(bootstrapped)));
    assert!(!state.diag.doctor.is_bootstrapping);
    assert!(state.diag.doctor.error.is_none());

    // Err 分支同样只清 busy + 置错误位。
    let _ = state.update(Message::RunBootstrap);
    let _ = state.update(Message::BootstrapFinished(Err(InfiltratorError::Internal(
        "bootstrap failed".to_string(),
    ))));
    assert!(!state.diag.doctor.is_bootstrapping);
    assert!(state.diag.doctor.error.is_some());
}

#[test]
fn test_doctor_actions_are_noop_in_demo_mode() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    for message in [Message::RunDoctor, Message::RunDoctorFix, Message::RunBootstrap] {
        let _ = state.update(message.clone());
    }
    assert!(!state.diag.doctor.is_running);
    assert!(!state.diag.doctor.is_fixing);
    assert!(!state.diag.doctor.is_bootstrapping);
}

#[test]
fn test_doctor_status_badge_mapping() {
    use crate::view::components::BadgeKind;
    use crate::view::doctor::{status_badge_kind, status_label};
    assert_eq!(status_badge_kind(DoctorStatus::Pass), BadgeKind::Success);
    assert_eq!(status_badge_kind(DoctorStatus::Warn), BadgeKind::Warning);
    assert_eq!(status_badge_kind(DoctorStatus::Fail), BadgeKind::Danger);
    assert_eq!(status_badge_kind(DoctorStatus::Skip), BadgeKind::Neutral);
    assert_eq!(status_label(DoctorStatus::Pass), "PASS");
    assert_eq!(status_label(DoctorStatus::Warn), "WARN");
    assert_eq!(status_label(DoctorStatus::Fail), "FAIL");
    assert_eq!(status_label(DoctorStatus::Skip), "SKIP");
}

#[test]
fn test_doctor_report_deserializes_admin_api_shape() {
    // 与 /admin/api/doctor 的 JSON 形状（DoctorRunResponse flatten report）
    // 对齐：exit_code 是额外键，反序列化必须忽略。
    let raw = serde_json::json!({
        "started_at": 100,
        "finished_at": 200,
        "exit_code": 1,
        "checks": [
            {
                "id": "config.settings_parse",
                "category": "config",
                "status": "pass",
                "summary": "App settings file parses as TOML",
                "detail": null,
                "hint": null
            },
            {
                "id": "config.configs_dir",
                "category": "config",
                "status": "fail",
                "summary": "Configs directory exists"
            }
        ]
    });
    let parsed: DoctorReport = serde_json::from_value(raw).expect("admin shape must parse");
    assert_eq!(parsed.checks.len(), 2);
    assert_eq!(parsed.checks[0].status, DoctorStatus::Pass);
    assert_eq!(parsed.checks[1].status, DoctorStatus::Fail);
    assert!(parsed.checks[1].hint.is_none(), "hint/detail are optional");
    assert_eq!(parsed.count_by_status(DoctorStatus::Fail), 1);
}
