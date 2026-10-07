//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::probe_settings_store::ProbeSettingsStore;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::advanced::text;
use iced::advanced::widget::{Tree, tree::Tag};
use iced::widget::text_input::State;
use iced::{Renderer, Task};
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::language_choice::project_language;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::ime::ImeCompositionEvent;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream =
                into_stream(task).expect("language selection submits the durable command");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("language terminal result")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn replay(state: &mut AppState, store: &ProbeSettingsStore) {
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated host capability"),
        )
    });
    snapshot.revision = state.surface.revision() + 1;
    snapshot.language_settings =
        project_language(Some(&Ok(store.settings.lock().unwrap().clone())));
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn input_state(tree: &mut Tree) -> Option<&mut State<<Renderer as text::Renderer>::Paragraph>> {
    if tree.tag == Tag::of::<State<<Renderer as text::Renderer>::Paragraph>>() {
        return Some(tree.state.downcast_mut());
    }
    tree.children.iter_mut().find_map(input_state)
}

#[test]
fn native_language_save_failure_retry_and_refresh_preserve_widget_focus_selection_draft_and_ime() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Settings));
    let store = Arc::new(ProbeSettingsStore::default());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone())),
    ));
    state.commands = Some(application);
    state.shell.demo = false;
    replay(&mut state, &store);
    state.editor.editor_path_setting = "unsaved editor {name}".into();
    state
        .shell
        .ime
        .apply(ImeCompositionEvent::Preedit("zhong".into()));
    let ime = state.shell.ime.clone();
    let mut tree = Tree::new(state.view().as_widget());
    let field = input_state(&mut tree).expect("real Settings input");
    field.focus();
    field.select_all();
    let cursor = field.cursor();
    store.reject.store(true, Ordering::SeqCst);
    let original = state.shell.lang.clone();
    let task = state.update(Message::SetLanguage("en-US".into()));
    let token = state.shell.language_choice.pending.unwrap().token;
    assert_eq!(
        state.shell.lang, original,
        "an accepted save cannot apply the language early"
    );
    assert_eq!(
        state.update(Message::SetLanguage("system".into())).units(),
        0
    );
    let _ = state.update(Message::LanguageChoiceApplied {
        token: token + 1,
        result: Ok(()),
    });
    assert!(state.shell.language_choice.pending.is_some());
    let _ = state.update(terminal(task));
    assert_eq!(state.shell.lang, original);
    assert_eq!(
        state.shell.language_choice.failure.as_ref().unwrap().code,
        ErrorCode::Storage
    );
    assert_eq!(
        state.shell.language_choice.requested,
        Some(LanguagePreference::English)
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.reject.store(false, Ordering::SeqCst);
    let task = state.update(Message::RetryLanguageChoice);
    let _ = state.update(terminal(task));
    assert_eq!(state.shell.lang, "en-US");
    assert_eq!(store.settings.lock().unwrap().language, "en-US");
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(state.editor.editor_path_setting, "unsaved editor {name}");
    assert_eq!(state.shell.ime, ime);
    tree.diff(state.view().as_widget());
    let field = input_state(&mut tree).unwrap();
    assert!(field.is_focused());
    assert_eq!(field.cursor(), cursor);
    let mut delayed = state.surface.latest().unwrap().clone();
    delayed.revision += 1;
    assert!(state.apply_shared_surface_snapshot(delayed));
    assert_eq!(
        state.shell.lang, "en-US",
        "delayed pre-save observation cannot restore the old language"
    );
    replay(&mut state, &store);
    assert_eq!(
        state.shell.language_choice.applied,
        Some(LanguagePreference::English)
    );
    let _ = state.update(Message::SetLanguage("unknown".into()));
    assert_eq!(
        state.shell.language_choice.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(state.shell.lang, "en-US");
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
}
