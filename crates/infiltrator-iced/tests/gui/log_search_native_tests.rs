//! test-intent: behavior
use crate::state::AppState;
use crate::test_mounts::native_widgets::native;
use crate::types::message::Message;
use crate::view::runtime::logs_controls::{follow, search};
use crate::view_root::interaction_regions::InteractionRegion;
use futures_util::StreamExt;
use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, RelativeOffset, Scrollable};
use iced::widget::Id;
use iced::{Rectangle, Task, Vector};
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{LogCaptureProcess, populate_logs};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use tokio::runtime::Builder;

fn edit(state: &mut AppState, query: &str) {
    let messages = native(
        search(state),
        InteractionRegion::LogsSearch.id(),
        Some(query),
    );
    assert!(!messages.is_empty());
    for message in messages {
        let _ = state.update(message);
    }
}
#[test]
fn native_log_query_drives_shared_regex_highlights_error_empty_and_clear() {
    let (mut state, _) = AppState::new();
    for raw in [
        "INFO[1] API.example connected",
        "ERROR[2] dial timeout",
        "DEBUG[3] healthy",
    ] {
        let _ = state.update(Message::LogReceived(raw.into()));
    }
    edit(&mut state, "api\\.example|timeout");
    assert_eq!(state.diag.log_search.matched_count(), 2);
    assert!(
        state
            .diag
            .log_search
            .row(1)
            .unwrap()
            .message
            .iter()
            .any(|run| run.highlighted && run.text == "API.example")
    );
    edit(&mut state, "(");
    assert_eq!(state.diag.log_search.status_key(), "logs_search_invalid");
    assert!(state.diag.log_search.invalid_pattern().is_some());
    edit(&mut state, "no-match");
    assert_eq!(state.diag.log_search.status_key(), "logs_search_no_matches");
    let messages = native(
        search(&state),
        InteractionRegion::LogsSearchClear.id(),
        None,
    );
    assert!(!messages.is_empty());
    for message in messages {
        let _ = state.update(message);
    }
    assert_eq!(state.diag.log_search.query(), "");
    assert_eq!(state.diag.log_search.matched_count(), 3);
    let _ = state.update(Message::ClearRuntimeLogs);
    assert_eq!(
        state.diag.log_search.status_key(),
        "logs_no_realtime_records"
    );
    assert_eq!(state.diag.log_search.source_count(), 0);
}

#[test]
fn native_log_query_is_retained_across_refresh_language_and_observed_empty() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::LogReceived("INFO[1] connected".into()));
    edit(&mut state, "timeout");
    assert_eq!(state.diag.log_search.status_key(), "logs_search_no_matches");
    let _ = state.update(Message::LogReceived("WARN[2] TIMEOUT on stream".into()));
    assert_eq!(state.diag.log_search.matched_count(), 1);
    state.shell.lang = "en-US".into();
    assert_eq!(state.diag.log_search.query(), "timeout");
    assert!(
        state
            .diag
            .log_search
            .row(2)
            .unwrap()
            .message
            .iter()
            .any(|run| run.highlighted && run.text == "TIMEOUT")
    );
    let _ = state.update(Message::ClearRuntimeLogs);
    assert_eq!(state.diag.log_search.query(), "timeout");
    assert_eq!(
        state.diag.log_search.status_key(),
        "logs_no_realtime_records"
    );
}

#[test]
fn native_log_query_keeps_failed_facts_and_retires_them_on_actual_core_restart() {
    let (mut state, _) = AppState::new();
    let runtime = tokio_application_runtime().unwrap();
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(process.clone(), process, runtime.clone());
    state.commands = Some(core.clone());
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let producing = core.clone();
    runtime.block_on(Box::pin(async move {
        populate_logs(&producing).await.unwrap();
    }));
    let read = |state: &mut AppState| {
        let snapshot = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(reader.read())
            .unwrap();
        assert!(state.apply_shared_surface_snapshot(snapshot));
    };
    read(&mut state);
    edit(&mut state, "api\\.example");
    assert_eq!(state.diag.log_search.matched_count(), 2);
    let current = core.snapshot();
    let scope = LogSession {
        generation: current.generation,
        token: current.session_token.unwrap(),
    };
    core.log_application().fail(
        scope,
        Failure::new(ErrorCode::Permission, "controller denied", false),
    );
    read(&mut state);
    assert_eq!(
        state.diag.log_search.status_key(),
        "logs_search_unavailable"
    );
    assert_eq!(state.diag.log_search.matched_count(), 2);
    let restarting = core.clone();
    runtime.block_on(Box::pin(async move {
        restarting
            .execute(CommandIntent::RestartCore)
            .await
            .into_unit()
            .unwrap();
    }));
    read(&mut state);
    assert_eq!(state.diag.log_search.query(), "api\\.example");
    assert!(state.diag.log_search.rows().is_empty());
    let current = core.snapshot();
    let scope = LogSession {
        generation: current.generation,
        token: current.session_token.unwrap(),
    };
    core.log_application().bind(Some(scope));
    core.log_application()
        .ingest(scope, RuntimeStreamEvent::Connected);
    read(&mut state);
    assert_eq!(
        state.diag.log_search.status_key(),
        "logs_no_realtime_records"
    );
    let closing = core.clone();
    runtime.block_on(Box::pin(async move {
        closing.close().await.unwrap();
    }));
}

#[test]
fn native_follow_toggle_locks_scroll_operations_without_stopping_actual_log_ingestion() {
    let (mut state, _) = AppState::new();
    let messages = native(follow(&state), InteractionRegion::LogFollow.id(), None);
    assert!(!messages.is_empty());
    for message in messages {
        assert_eq!(
            native_scroll_effect(state.update(message)),
            ScrollEffect::Position(0.0)
        );
    }
    assert!(!state.diag.log_search.follow.should_follow());
    assert_eq!(
        native_scroll_effect(state.update(Message::LogReceived("INFO[1] still received".into()))),
        ScrollEffect::Position(0.0)
    );
    assert_eq!(state.diag.log_search.source_count(), 1);
    let messages = native(follow(&state), InteractionRegion::LogFollow.id(), None);
    assert!(!messages.is_empty());
    for message in messages {
        assert_eq!(
            native_scroll_effect(state.update(message)),
            ScrollEffect::End
        );
    }
    assert!(state.diag.log_search.follow.should_follow());
    let _ = state.update(Message::LogsScrolled {
        offset: 400.0,
        content: 600.0,
        viewport: 200.0,
    });
    let _ = state.update(Message::LogsScrolled {
        offset: 300.0,
        content: 600.0,
        viewport: 200.0,
    });
    assert!(!state.diag.log_search.follow.should_follow());
    assert_eq!(
        native_scroll_effect(state.update(Message::LogReceived("WARN[2] no forced jump".into()))),
        ScrollEffect::Position(300.0)
    );
    assert_eq!(state.diag.log_search.source_count(), 2);
}

#[derive(Debug, Default, PartialEq)]
enum ScrollEffect {
    #[default]
    None,
    End,
    Position(f32),
}
impl Scrollable for ScrollEffect {
    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        assert_eq!(offset.y, Some(1.0));
        *self = Self::End;
    }
    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        *self = Self::Position(offset.y.unwrap());
    }
    fn scroll_by(&mut self, _: AbsoluteOffset, _: Rectangle, _: Rectangle) {
        panic!("log synchronization must use a stable position, never a delta");
    }
}
fn native_scroll_effect(task: Task<Message>) -> ScrollEffect {
    let mut effect = ScrollEffect::default();
    Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual native viewport operation");
            while let Some(action) = stream.next().await {
                let Action::Widget(mut operation) = action else {
                    panic!("viewport task must only operate on the native scroll surface");
                };
                operation.scrollable(
                    Some(&Id::new("log_scroller")),
                    Rectangle::default(),
                    Rectangle::default(),
                    Vector::ZERO,
                    &mut effect,
                );
            }
        });
    effect
}
