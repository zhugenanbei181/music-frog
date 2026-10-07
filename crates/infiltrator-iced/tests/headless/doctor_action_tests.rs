//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::doctor_port::DiagnosticPort;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual diagnostic command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal result")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn publish(state: &mut AppState, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = state.surface.revision() + 1;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
#[test]
fn actual_doctor_command_failure_retry_single_repair_and_bootstrap_share_reader_and_keep_native_surface()
 {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_doctor(doctor.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_doctor(doctor);
    let (mut state, _) = AppState::demo(&demo_env(Route::Doctor));
    state.commands = Some((*application).clone());
    publish(&mut state, &reader);
    let task = state.update(Message::RunDoctor);
    let token = state.diag.doctor.action.pending.as_ref().unwrap().token;
    assert_eq!(state.update(Message::RunDoctor).units(), 0);
    let _ = state.update(Message::DoctorCommandFinished {
        token: token + 1,
        result: Ok(()),
    });
    assert!(state.diag.doctor.action.pending.is_some());
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    let first = state.surface.latest().unwrap().pages.doctor.clone();
    assert!(first.data.as_ref().unwrap().checks[0].fix_available);
    let mut tree = Tree::new(state.view().as_widget());
    assert!(!tree.children.is_empty());
    port.deny.store(true, Ordering::SeqCst);
    let task = state.update(Message::RunDoctor);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.doctor.action.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(
        state.surface.latest().unwrap().pages.doctor.data,
        first.data
    );
    assert!(matches!(
        state.surface.latest().unwrap().pages.doctor.status,
        PageStatus::Failed { .. }
    ));
    assert_eq!(
        state
            .update(Message::RepairDoctorIssue("config.integrity".into()))
            .units(),
        0
    );
    tree.diff(state.view().as_widget());
    port.deny.store(false, Ordering::SeqCst);
    let task = state.update(Message::RetryDoctorCommand);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    let task = state.update(Message::RepairDoctorIssue("config.integrity".into()));
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(
        state
            .surface
            .latest()
            .unwrap()
            .pages
            .doctor
            .data
            .as_ref()
            .unwrap()
            .overall_healthy
    );
    assert_eq!(port.fixes.load(Ordering::SeqCst), 1);
    assert_eq!(
        state
            .update(Message::RepairDoctorIssue("config.integrity".into()))
            .units(),
        0
    );
    let task = state.update(Message::RunBootstrap);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(port.bootstraps.load(Ordering::SeqCst), 1);
    assert_eq!(port.runs.load(Ordering::SeqCst), 5);
    assert!(state.diag.doctor.action.pending.is_none());
    assert!(state.diag.doctor.action.failure.is_none());
}

#[test]
fn doctor_capture_initializes_without_prior_snapshot_and_actual_task_retains_finding_and_retry() {
    let mut env = demo_env(Route::Doctor);
    env.scenario = Some(FeatureId::DoctorFailureRecovery);
    let (mut state, task) = AppState::demo(&env);
    assert!(state.diag.doctor.action.pending.is_some());
    let _ = state.update(terminal(task));
    assert!(state.diag.doctor.action.can_retry());
    let page = &state.surface.latest().unwrap().pages.doctor;
    assert!(matches!(page.status, PageStatus::Failed { .. }));
    assert_eq!(
        page.data.as_ref().unwrap().checks[0].name,
        "Configuration requires repair"
    );
    assert_eq!(
        state.diag.doctor.action.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    let tree = Tree::new(state.view().as_widget());
    assert!(!tree.children.is_empty());
}
