//! test-intent: behavior
//! The repair progress of the diagnostics panel of the real Iced surface.
//!
//! The repair business is a state machine the shared owner drives: the panel
//! that projects it shows the pending phase while the owner still owes its
//! terminal, the owner's success closes the business, the owner's refusal
//! opens the recovery path, and the retry repeats the refused operation.
//! Every case below drives the production surface through the shared command
//! application, so a surface that lost one of those phases fails here.

use crate::command_harness::recording_application;
use crate::doctor_port::DiagnosticPort;
use crate::host_command_harness::host_test_command_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::ErrorCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;

/// The check the diagnostic fixture published as repairable.
const REPAIRABLE_CHECK: &str = "config.integrity";

/// The terminal message of a real diagnostic command.
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

/// Publish the owner's own surface snapshot to the surface.
fn publish(state: &mut AppState, reader: &ProductSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = state.surface.revision() + 1;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}

/// The doctor page with the shared diagnostic owner behind it.
///
/// The owner really diagnoses first: the page publishes the repairable
/// finding the repair command works on, so the repair is a command the owner
/// accepts instead of a control the surface refuses.
fn doctor_surface() -> (AppState, Arc<DiagnosticPort>, ProductSurfaceReader) {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        host_test_command_application([Capability::CoreLifecycle]).with_doctor(doctor.clone()),
    ));
    let application = Arc::new(application);
    let reader = ProductSurfaceReader::new(
        application.clone(),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_doctor(doctor);
    let (mut state, _) = AppState::new();
    state.shell.current_route = Route::Doctor;
    state.attach_product_application((*application).clone());
    publish(&mut state, &reader);
    let task = state.update(Message::RunDoctor);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    (state, port, reader)
}

#[test]
fn the_repair_progress_is_pending_while_the_owner_owes_its_terminal() {
    let mut env = demo_env(Route::Doctor);
    env.scenario = Some(FeatureId::DoctorRepairProgress);
    // The capture's diagnostic host keeps its answer: the repair command runs
    // and the owner never publishes a terminal, so the phase the surface
    // shows is the pending one the capture measures.
    let (state, _task) = AppState::demo(&env);
    assert!(
        state.diag.doctor.is_busy(),
        "the panel projects the owner's pending phase"
    );
    assert!(
        matches!(
            state.diag.doctor.current_action(),
            Some(DoctorAction::RepairAll | DoctorAction::RepairOne(_))
        ),
        "the pending phase names the repair the owner is running"
    );
    assert!(
        state.diag.doctor.failure().is_none(),
        "an answer the owner still owes is not a failure"
    );
}

#[test]
fn the_owner_success_closes_the_repair_progress() {
    let (mut state, port, _reader) = doctor_surface();
    let task = state.update(Message::RepairDoctorIssue(REPAIRABLE_CHECK.into()));
    let _ = state.update(terminal(task));
    assert_eq!(
        port.fixes.load(Ordering::SeqCst),
        1,
        "the repair really reached the owner"
    );
    assert!(
        matches!(&state.diag.doctor.action.phase, DoctorActionPhase::Idle),
        "the owner's success closes the repair progress"
    );
    assert!(
        !state.diag.doctor.is_busy(),
        "the panel has nothing left to show as running"
    );
}

#[test]
fn the_owner_refusal_opens_the_recovery_path_of_the_repair() {
    let (mut state, port, _reader) = doctor_surface();
    port.deny.store(true, Ordering::SeqCst);
    let task = state.update(Message::RepairDoctorIssue(REPAIRABLE_CHECK.into()));
    let _ = state.update(terminal(task));
    assert_eq!(
        match &state.diag.doctor.action.phase {
            DoctorActionPhase::Failed { failure, .. } => failure.code.clone(),
            DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => {
                panic!("the owner's refusal must be retained")
            }
        },
        ErrorCode::Permission
    );
    assert!(
        state.diag.doctor.can_retry(),
        "a permission refusal opens the recovery path of the repair"
    );
}

#[test]
fn the_retry_repeats_the_refused_repair() {
    let (mut state, port, _reader) = doctor_surface();
    port.deny.store(true, Ordering::SeqCst);
    let task = state.update(Message::RepairDoctorIssue(REPAIRABLE_CHECK.into()));
    let _ = state.update(terminal(task));
    let task = state.update(Message::RetryDoctorCommand);
    assert_eq!(
        match &state.diag.doctor.action.phase {
            DoctorActionPhase::Pending(pending) => pending.action.clone(),
            DoctorActionPhase::Idle | DoctorActionPhase::Failed { .. } => {
                panic!("the retry runs the refused repair")
            }
        },
        DoctorAction::RepairOne(REPAIRABLE_CHECK.into()),
        "the retry repeats the refused operation instead of a new one"
    );
    let _ = state.update(terminal(task));
    assert_eq!(
        port.fixes.load(Ordering::SeqCst),
        2,
        "the retry reached the same owner again"
    );
}
