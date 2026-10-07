//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::subscription_filter_fixture::{FIXTURE_DOCUMENT, observation};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use std::path::PathBuf;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("filter terminal task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("filter command must produce a terminal message");
            };
            assert!(stream.next().await.is_none());
            message
        })
}

fn request(state: &mut AppState, profile: &str) -> u64 {
    state.editor.editor_path = Some(PathBuf::from(format!("/isolated/{profile}.yaml")));
    let task = state.update(Message::LoadProfileFilter);
    assert_eq!(task.units(), 1);
    state.editor.filter_load.as_ref().unwrap().0
}
fn loaded(state: &mut AppState, token: u64, profile: &str, include: &str) {
    let _ = state.update(Message::ProfileFilterLoaded {
        token,
        profile: profile.into(),
        result: observation(
            profile,
            FIXTURE_DOCUMENT,
            SubscriptionFilterDraft {
                include: include.into(),
                ..Default::default()
            },
        ),
    });
}
#[test]
fn tea_filter_load_requests_reject_old_profile_receipts_and_preserve_a_changed_source_draft() {
    let (mut state, _) = AppState::new();
    let alpha = request(&mut state, "alpha");
    let beta = request(&mut state, "beta");
    assert!(beta > alpha);
    loaded(&mut state, alpha, "alpha", "wrong");
    assert!(state.editor.filter_editor.source_profile().is_none());
    loaded(&mut state, beta, "beta", "base");
    let _ = state.update(Message::UpdateFilterInclude("draft {source}".into()));
    let refresh = request(&mut state, "beta");
    loaded(&mut state, refresh, "beta", "remote");
    assert_eq!(state.editor.filter_editor.draft.include, "draft {source}");
    assert!(state.editor.filter_editor.stale());
    assert_eq!(state.update(Message::SaveProfileFilter).units(), 0);
    let _ = state.update(Message::DiscardProfileFilter);
    assert_eq!(state.editor.filter_editor.draft.include, "remote");
    assert!(state.editor.filter_editor.current());
}
#[test]
fn tea_filter_typed_failure_and_tokens_keep_error_in_place_and_discard_has_no_write() {
    let (mut state, _) = AppState::new();
    let read = request(&mut state, "alpha");
    loaded(&mut state, read, "alpha", "base");
    let _ = state.update(Message::UpdateFilterExclude("draft".into()));
    assert_eq!(state.update(Message::SaveProfileFilter).units(), 1);
    let token = state.editor.filter_editor.pending.as_ref().unwrap().token;
    assert_eq!(state.update(Message::DiscardProfileFilter).units(), 0);
    assert!(state.editor.filter_editor.pending.is_some());
    let failure = Failure::new(ErrorCode::Storage, "write denied", true);
    let _ = state.update(Message::ProfileFilterSaved {
        token,
        result: Err(failure.clone()),
    });
    assert_eq!(state.editor.filter_editor.failure, Some(failure));
    assert_eq!(state.editor.filter_editor.draft.exclude, "draft");
    assert_eq!(state.update(Message::SaveProfileFilter).units(), 1);
    let second = state.editor.filter_editor.pending.as_ref().unwrap().token;
    assert!(second > token);
    let _ = state.update(Message::ProfileFilterSaved {
        token,
        result: Ok(SubscriptionFilterApplied {
            source: observation("alpha", FIXTURE_DOCUMENT, Default::default())
                .unwrap()
                .source,
            report: FilterReport::default(),
        }),
    });
    assert_eq!(
        state.editor.filter_editor.pending.as_ref().unwrap().token,
        second
    );
    let _ = state.update(Message::ProfileFilterSaved {
        token: second,
        result: Err(Failure::unsupported("host cannot apply")),
    });
    assert_eq!(state.update(Message::DiscardProfileFilter).units(), 0);
    assert_eq!(state.editor.filter_editor.draft.exclude, "");
    assert!(state.editor.filter_editor.failure.is_none());
    assert!(state.editor.filter_editor.pending.is_none());
}

#[test]
fn tea_filter_missing_command_service_returns_a_terminal_failure_without_host_fallback() {
    let (mut state, _) = AppState::new();
    let read = request(&mut state, "alpha");
    loaded(&mut state, read, "alpha", "base");
    let _ = state.update(Message::UpdateFilterExclude("unsaved".into()));
    let message = terminal(state.update(Message::SaveProfileFilter));
    let Message::ProfileFilterSaved {
        token,
        result: Err(failure),
    } = message
    else {
        panic!("missing command service must fail explicitly");
    };
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        token,
        state.editor.filter_editor.pending.as_ref().unwrap().token
    );
    let _ = state.update(Message::ProfileFilterSaved {
        token,
        result: Err(failure.clone()),
    });
    assert!(state.editor.filter_editor.pending.is_none());
    assert_eq!(state.editor.filter_editor.failure, Some(failure));
    assert_eq!(state.editor.filter_editor.draft.exclude, "unsaved");
    assert!(!state.editor.filter_editor.applied);
    assert!(state.editor.filter_editor.report.is_none());
}
