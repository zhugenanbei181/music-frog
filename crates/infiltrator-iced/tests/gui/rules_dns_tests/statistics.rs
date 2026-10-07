//! test-intent: behavior
//! Native inspector and modal widgets replay the shared source-qualified workflow.
use super::rules_tracer::{publish, run, setup};
use crate::state::AppState;
use crate::test_mounts::native_widgets;
use crate::types::message::Message;
use crate::view::rule_hit_card::rule_hit_card;
use crate::view_root::interaction_regions::InteractionRegion;
use crate::view_root::modals::rule_statistics::statistics_confirmation;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::rule_statistics_inspector_projection::project_inspector;
use infiltrator_application::rule_statistics_workbench::StatisticsTab;
use infiltrator_contract::error::ErrorCode;
use infiltrator_shared::locales::Lang;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;

fn native(state: &mut AppState, region: InteractionRegion, modal: bool) -> Vec<Task<Message>> {
    let element = if modal {
        statistics_confirmation(state)
    } else {
        rule_hit_card(state, &Lang(&state.shell.lang))
    };
    let messages = native_widgets::native(element, region.id(), None);
    assert!(!messages.is_empty(), "real native operation");
    messages
        .into_iter()
        .map(|message| state.update(message))
        .collect()
}
fn complete(state: &mut AppState, tasks: Vec<Task<Message>>) {
    for task in tasks {
        let message = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut stream = into_stream(task).expect("actual command task");
                let Some(Action::Output(message)) = stream.next().await else {
                    panic!("actual terminal");
                };
                message
            });
        let _ = state.update(message);
    }
}

#[test]
fn native_statistics_cleanup_cancel_and_confirm_require_independent_review_and_never_persist() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("special.com".into()));
    run(&mut state, &reader);
    let before = state.editor.rule_list.draft.clone();
    native(&mut state, InteractionRegion::StatisticsInactive, false);
    assert_eq!(state.editor.rule_hit_audit.tab, StatisticsTab::Inactive);
    native(&mut state, InteractionRegion::StatisticsInspect, false);
    native(&mut state, InteractionRegion::StatisticsCleanup, false);
    let review = state.editor.rule_hit_audit.confirmation.clone().unwrap();
    assert_eq!(state.editor.rule_list.draft, before);
    native(&mut state, InteractionRegion::ConfirmationCancel, true);
    assert_eq!(state.editor.rule_list.draft, before);
    assert!(!state.editor.rule_list.dirty());
    native(&mut state, InteractionRegion::StatisticsCleanup, false);
    native(&mut state, InteractionRegion::ConfirmationAccept, true);
    assert!(state.editor.rule_hit_audit.confirmation.is_none());
    assert!(state.shell.confirmation.is_none());
    for target in review.targets {
        assert!(
            !state.editor.rule_list.draft[state.editor.rule_list.row_index(target.id).unwrap()]
                .enabled
        );
    }
    assert!(state.editor.rule_list.dirty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_reset_permission_retry_requires_matching_receipt_and_actual_readback() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("special.com".into()));
    run(&mut state, &reader);
    let before = state.editor.rule_hit_audit.audit.clone().unwrap();
    native(&mut state, InteractionRegion::StatisticsTop, false);
    assert_eq!(state.editor.rule_hit_audit.tab, StatisticsTab::TopHits);
    store.deny_read.store(true, Ordering::SeqCst);
    let tasks = native(&mut state, InteractionRegion::StatisticsReset, false);
    assert!(state.editor.rule_hit_audit.clear_pending.is_some());
    complete(&mut state, tasks);
    assert_eq!(
        state
            .editor
            .rule_hit_audit
            .clear_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(state.editor.rule_hit_audit.audit.as_ref(), Some(&before));
    store.deny_read.store(false, Ordering::SeqCst);
    let tasks = native(&mut state, InteractionRegion::StatisticsReset, false);
    complete(&mut state, tasks);
    assert!(state.editor.rule_hit_audit.awaiting_revision.is_some());
    assert_eq!(state.editor.rule_hit_audit.audit.as_ref(), Some(&before));
    publish(&mut state, &reader);
    assert!(!state.editor.rule_hit_audit.busy());
    assert_eq!(
        state
            .editor
            .rule_hit_audit
            .audit
            .as_ref()
            .unwrap()
            .total_hits,
        0
    );
    assert!(state.editor.rule_hit_audit.audit.as_ref().unwrap().revision > before.revision);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_changed_source_retains_review_but_disabled_confirm_publishes_no_action() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("special.com".into()));
    run(&mut state, &reader);
    native(&mut state, InteractionRegion::StatisticsInspect, false);
    native(&mut state, InteractionRegion::StatisticsCleanup, false);
    let review = state.editor.rule_hit_audit.confirmation.clone().unwrap();
    store.select_profile("other.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut state, &reader);
    assert_eq!(state.editor.rule_hit_audit.audit, None);
    assert_eq!(state.editor.rule_hit_audit.confirmation, Some(review));
    let messages = native_widgets::native(
        statistics_confirmation(&state),
        InteractionRegion::ConfirmationAccept.id(),
        None,
    );
    assert!(
        messages
            .iter()
            .all(|message| matches!(message, Message::Noop))
    );
    for message in messages {
        let _ = state.update(message);
    }
    assert!(state.editor.rule_hit_audit.confirmation.is_some());
    assert!(!state.editor.rule_list.dirty());
    native(&mut state, InteractionRegion::ConfirmationCancel, true);
    assert!(state.editor.rule_hit_audit.confirmation.is_none());
    assert!(state.shell.confirmation.is_none());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_unknown_cannot_emit_a_reset_and_locale_replays_shared_copy() {
    let (mut state, _, store) = setup();
    assert!(state.editor.rule_hit_audit.audit.is_none());
    let messages = native_widgets::native(
        rule_hit_card(&state, &Lang(&state.shell.lang)),
        InteractionRegion::StatisticsReset.id(),
        None,
    );
    assert!(messages.is_empty());
    state.shell.lang = "en-US".into();
    let projection = project_inspector(
        &state.editor.rule_hit_audit,
        &state.editor.rule_list,
        &state.shell.lang,
    );
    assert_eq!(projection.metrics.total_hits, "Not observed");
    assert!(!projection.can_reset);
    state.shell.lang = "zh-CN".into();
    let projection = project_inspector(
        &state.editor.rule_hit_audit,
        &state.editor.rule_list,
        &state.shell.lang,
    );
    assert_eq!(projection.metrics.total_hits, "未观测");
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
