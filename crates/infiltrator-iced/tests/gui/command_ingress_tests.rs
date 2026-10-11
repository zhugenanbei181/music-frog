use super::*;
use crate::state::AppState;
use crate::test_mounts::command_harness::{
    RecordingHandler, attach_product_application_with_accepted_owner_snapshot,
};
use crate::test_mounts::identified_core_application::identified_core_application;
use infiltrator_contract::command::{CommandKind, CommandResult};
use infiltrator_contract::snapshot::CoreEvent;
use std::sync::Arc;

fn product_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let application = identified_core_application();
    let handler = Arc::new(RecordingHandler::default());
    application.install_command_handler(handler.clone());
    (application, handler)
}

#[tokio::test]
async fn product_core_command_without_an_accepted_snapshot_fails_closed() {
    let (application, handler) = product_application();
    let (mut state, _) = AppState::new();
    state.attach_product_application(application.clone());
    let ingress = state.command_ingress().expect("attached product ingress");

    let outcome = ingress.execute_outcome(CommandIntent::ClearLogs).await;

    assert!(matches!(outcome.result, CommandResult::Rejected { .. }));
    assert_eq!(outcome.scope, None);
    assert!(handler.0.lock().expect("handler calls").is_empty());
    assert!(application.drain_events().iter().any(|event| matches!(
        event,
        CoreEvent::CommandFailed {
            scope: None,
            kind: CommandKind::Runtime,
            ..
        }
    )));
}

#[tokio::test]
async fn product_core_command_uses_the_captured_surface_scope() {
    let (application, handler) = product_application();
    let (mut state, _) = AppState::new();
    attach_product_application_with_accepted_owner_snapshot(&mut state, application);
    let ingress = state.command_ingress().expect("accepted product ingress");
    let scope = ingress
        .observed_scope()
        .expect("accepted snapshot owner scope");

    let outcome = ingress.execute_outcome(CommandIntent::ClearLogs).await;

    assert!(matches!(outcome.result, CommandResult::Completed { .. }));
    assert_eq!(outcome.scope, Some(scope));
    assert_eq!(
        handler.0.lock().expect("handler calls").as_slice(),
        [CommandIntent::ClearLogs]
    );
}
