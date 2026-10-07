//! test-intent: behavior
use crate::command_harness::rejecting_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::protocol_form::ProtocolField;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use tokio::runtime::Builder;

const URI: &str =
    "vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node";

#[test]
fn protocol_form_uses_typed_fields_and_retains_invalid_input_and_save_failure() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let (application, handler) = rejecting_application();
    state.commands = Some(application);
    state.shell.demo = false;
    assert_eq!(state.update(Message::OpenCustomNodeModal).units(), 0);
    for (id, value) in [
        (ProtocolField::Name, "local-vless"),
        (ProtocolField::Server, "node.example.com"),
        (ProtocolField::Port, "70000"),
        (
            ProtocolField::Secret,
            "b831381d-6324-4d53-ad4f-8cda48b30811",
        ),
    ] {
        assert_eq!(
            state
                .update(Message::UpdateCustomNodeField(id, value.into()))
                .units(),
            0
        );
    }
    assert_eq!(state.update(Message::SaveCustomNodeForm).units(), 0);
    assert_eq!(
        state.runtime.custom_node_inputs.values[&ProtocolField::Port],
        "70000"
    );
    assert!(
        state
            .runtime
            .custom_node_inputs
            .errors
            .contains_key(&ProtocolField::Port)
    );
    assert_eq!(
        state
            .runtime
            .custom_node_studio
            .draft
            .as_ref()
            .unwrap()
            .port,
        0
    );
    let _ = state.update(Message::UpdateCustomNodeField(
        ProtocolField::Port,
        "443".into(),
    ));
    let _ = state.update(Message::UpdateCustomNodeField(
        ProtocolField::Network,
        "ws".into(),
    ));
    let _ = state.update(Message::UpdateCustomNodeField(
        ProtocolField::WsPath,
        "/proxy".into(),
    ));
    let task = state.update(Message::SaveCustomNodeForm);
    assert_eq!(task.units(), 1);
    assert!(state.runtime.custom_node_saving);
    assert_eq!(state.update(Message::SaveCustomNodeForm).units(), 0);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let mut stream = into_stream(task).unwrap();
        let Some(Action::Output(message)) = stream.next().await else {
            panic!("terminal save result");
        };
        assert!(matches!(&message, Message::CustomNodeSaved(Err(error)) if error.to_string().contains("profile is read-only")));
        let _ = state.update(message);
        assert!(stream.next().await.is_none());
    });
    assert!(
        matches!(&handler.0.lock().unwrap()[0], CommandIntent::SaveCustomNodeDraft { draft } if draft.name == "local-vless" && draft.port == 443 && draft.params.transport.ws.path == "/proxy")
    );
    // A terminal persistence failure must leave the actual editor and its fields available.
    assert!(state.runtime.custom_node_modal_open);
    assert!(!state.runtime.custom_node_saving);
    assert!(
        state
            .runtime
            .custom_node_studio
            .last_error
            .as_deref()
            .unwrap()
            .contains("profile is read-only")
    );
    drop(state.view());
    // Unsolicited or replayed results cannot close the still-open editor.
    assert_eq!(state.update(Message::CustomNodeSaved(Ok(()))).units(), 0);
    assert!(state.runtime.custom_node_modal_open);
    *handler.1.lock().unwrap() = None;
    let retry = state.update(Message::SaveCustomNodeForm);
    runtime.block_on(async {
        let mut stream = into_stream(retry).unwrap();
        let Some(Action::Output(message)) = stream.next().await else {
            panic!("terminal retry result");
        };
        assert!(matches!(message, Message::CustomNodeSaved(Ok(()))));
        let _ = state.update(message);
    });
    assert!(!state.runtime.custom_node_modal_open);
}

#[test]
fn uri_preview_import_error_and_cancel_preserve_draft_without_commit() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let (application, handler) = rejecting_application();
    state.commands = Some(application);
    let _ = state.update(Message::OpenCustomNodeModal);
    let _ = state.update(Message::UpdateCustomNodeUriInput(URI.into()));
    assert_eq!(state.update(Message::ParseAndImportCustomUri).units(), 0);
    let imported = state.runtime.custom_node_studio.draft.clone().unwrap();
    assert_eq!(imported.name, "Imported-Node");
    assert!(
        state
            .runtime
            .custom_node_studio
            .uri_preview
            .as_ref()
            .unwrap()
            .starts_with("vless://")
    );
    let _ = state.update(Message::UpdateCustomNodeUriInput("broken://".into()));
    let _ = state.update(Message::ParseAndImportCustomUri);
    assert_eq!(
        state.runtime.custom_node_studio.draft.as_ref(),
        Some(&imported)
    );
    assert!(state.runtime.custom_node_studio.last_error.is_some());
    assert!(state.runtime.custom_node_modal_open);
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: KeyModifiers::default(),
    });
    assert!(!state.runtime.custom_node_modal_open);
    assert_eq!(state.update(Message::SaveCustomNodeForm).units(), 0);
    assert!(handler.0.lock().unwrap().is_empty());
}
