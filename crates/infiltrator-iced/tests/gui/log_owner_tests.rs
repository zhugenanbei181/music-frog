//! test-intent: behavior
//! TEA consumes the application buffer and executes clear/filter on that same owner.
use crate::state::AppState;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::sync::Arc;
use tokio::runtime::Builder;

fn composed() -> (AppState, CoreApplication) {
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_logs(core.log_application()),
    ));
    let (mut state, _) = AppState::new();
    state.commands = Some(core.clone());
    (state, core)
}
fn observe(state: &mut AppState, core: &CoreApplication, revision: u64) {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "fixture", true),
    );
    snapshot.core = core.snapshot();
    snapshot.pages.logs = core.log_application().page(&snapshot.core);
    snapshot.revision = revision;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn finish(state: &mut AppState, task: Task<Message>) {
    let message = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("real command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal message");
            };
            message
        });
    let _ = state.update(message);
}
#[test]
fn shared_logs_ignore_private_stream_and_clear_only_after_actual_command_execution() {
    let (mut state, core) = composed();
    let scope = LogSession {
        generation: 0,
        token: SessionToken::new(1),
    };
    core.log_application().bind(Some(scope));
    core.log_application()
        .ingest(scope, RuntimeStreamEvent::Item("WARN[1] retained".into()));
    observe(&mut state, &core, 1);
    assert_eq!(state.diag.logs.len(), 1);
    let _ = state.update(Message::LogReceived("private duplicate".into()));
    assert_eq!(state.diag.logs.len(), 1);
    let task = state.update(Message::ClearRuntimeLogs);
    assert_eq!(
        core.log_application()
            .page(&core.snapshot())
            .data
            .unwrap()
            .entries
            .len(),
        1
    );
    finish(&mut state, task);
    observe(&mut state, &core, 2);
    assert!(state.diag.logs.is_empty());
    assert!(
        core.log_application()
            .page(&core.snapshot())
            .data
            .unwrap()
            .entries
            .is_empty()
    );
}
#[test]
fn shared_filter_has_actual_effect_and_invalid_selection_retains_typed_failure() {
    let (mut state, core) = composed();
    let scope = LogSession {
        generation: 0,
        token: SessionToken::new(1),
    };
    core.log_application().bind(Some(scope));
    for raw in ["WARN[1] visible", "INFO[2] filtered"] {
        core.log_application()
            .ingest(scope, RuntimeStreamEvent::Item(raw.into()));
    }
    let task = state.update(Message::SetLogLevelFilter("WARN".into()));
    finish(&mut state, task);
    observe(&mut state, &core, 1);
    assert_eq!(
        state.diag.logs.iter().collect::<Vec<_>>(),
        vec!["WARN[1] visible"]
    );
    let task = state.update(Message::SetLogLevelFilter("pretend".into()));
    finish(&mut state, task);
    assert_eq!(
        state.diag.log_command_failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    observe(&mut state, &core, 2);
    assert_eq!(state.diag.logs.len(), 1);
}
