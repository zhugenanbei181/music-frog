//! Destructive confirmation drives the production update path with an isolated demo.
//! test-intent: behavior

use crate::command_harness::recording_application;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_iced::demo::DemoEnv;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::{ConfirmAction, Route};
use infiltrator_iced::types::message::Message;
use infiltrator_iced::types::options::EditorPane;
use tokio::runtime::Builder;

#[test]
fn close_all_confirmation_cancel_keeps_connections_and_schedules_no_work() {
    let env = DemoEnv {
        enabled: true,
        page: Route::Runtime,
        pane: EditorPane::Profile,
        providers_tab: false,
        lang: "en-US".into(),
        skin: iced::Theme::Dark,
        window_size: (1180.0, 780.0),
        capture_marker: None,
        scenario: None,
    };
    let (mut state, _) = AppState::demo(&env);
    let (application, handler) = recording_application();
    state.commands = Some(application);
    // Drive production dispatch against the injected recording application.
    // The demo constructor supplies only data; no real host runtime is present.
    state.shell.demo = false;
    let before = state.diag.connections.clone();
    assert_eq!(state.shell.confirmation, None);
    let open = state.update(Message::ExecuteCommand(CommandTarget::CloseAllConnections));
    assert_eq!(open.units(), 0);
    assert_eq!(
        state.shell.confirmation,
        Some(ConfirmAction::CloseAllConnections)
    );
    drop(state.view());
    let cancel = state.update(Message::CancelConfirmation);
    assert_eq!(cancel.units(), 0);
    assert_eq!(state.shell.confirmation, None);
    assert_eq!(state.diag.connections, before);
    assert_eq!(*handler.0.lock().unwrap(), Vec::<CommandIntent>::new());
    let stale = state.update(Message::ConfirmAction);
    assert_eq!(stale.units(), 0);
    assert_eq!(state.diag.connections, before);
    let _ = state.update(Message::RequestConfirmation(
        ConfirmAction::CloseAllConnections,
    ));
    let confirm = state.update(Message::ConfirmAction);
    assert_eq!(confirm.units(), 1);
    let executor = Builder::new_current_thread().enable_all().build().unwrap();
    executor.block_on(async {
        use futures_util::StreamExt;
        let mut stream = into_stream(confirm).expect("confirmed operation task");
        while let Some(action) = stream.next().await {
            if let Action::Output(message) = action {
                assert!(matches!(message, Message::OperationResult(Ok(()))));
                let _ = state.update(message);
            }
        }
    });
    assert_eq!(
        *handler.0.lock().unwrap(),
        vec![CommandIntent::CloseAllConnections]
    );
    assert_eq!(state.shell.confirmation, None);
}
