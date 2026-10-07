//! test-intent: behavior
//! Native TEA source observation and actual atomic shared command terminals.
use super::rules_tracer::{publish, run, setup};
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_application::shell_readout_projection::count_copy;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rule_edit::RuleMoveDirection;
use infiltrator_contract::surface_snapshot::PageId;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;

fn complete(state: &mut AppState, task: Task<Message>) {
    let message = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual rule-list command");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal message");
            };
            message
        });
    let _ = state.update(message);
}

#[test]
fn tea_rule_hit_counts_replay_shared_rows_and_ignore_unrelated_connection_snapshots() {
    let (mut state, reader, _) = setup();
    publish(&mut state, &reader);
    let mut snapshot = state.surface.latest().cloned().unwrap();
    snapshot.revision += 1;
    let rows = &mut snapshot.pages.rules.data.as_mut().unwrap().rules;
    rows[0].hit_count = Some(5);
    rows[1].hit_count = Some(0);
    rows[3].hit_count = Some(12);
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(snapshot)));
    let first = state.editor.rule_list.row_id(0).unwrap();
    let _ = state.update(Message::ToggleRuleEnabled(first));
    assert_eq!(state.editor.rules_render_cache[0].hit_count, Some(5));
    assert_eq!(state.editor.rules_render_cache[3].hit_count, Some(12));
    assert_eq!(state.editor.rules_render_cache[1].hit_count, Some(0));
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    assert_eq!(state.editor.rules_render_cache[0].hit_count, Some(5));
    assert_eq!(state.editor.rules_render_cache[3].hit_count, Some(12));
    state.surface = Default::default();
    let _ = state.update(Message::ToggleRuleEnabled(first));
    assert_eq!(
        state.editor.rules_render_cache[0].hit_count, None,
        "a missing rule readout never borrows unrelated connection counts"
    );
}

#[test]
fn tea_queued_row_actions_follow_identity_and_old_source_controls_cannot_mutate_new_rows() {
    let (mut state, reader, store) = setup();
    let original = store.content();
    let first = state.editor.rule_list.row_id(0).unwrap();
    let next = state.editor.rule_list.row_id(1).unwrap();
    let first_rule = state.editor.rule_list.draft[0].rule.clone();
    let next_rule = state.editor.rule_list.draft[1].rule.clone();
    let queued_toggle = Message::ToggleRuleEnabled(first);
    let _ = state.update(Message::MoveRuleUp(next));
    let _ = state.update(queued_toggle.clone());
    assert_eq!(state.editor.rule_list.draft[0].rule, next_rule);
    assert!(state.editor.rule_list.draft[0].enabled);
    assert_eq!(state.editor.rule_list.draft[1].rule, first_rule);
    assert!(!state.editor.rule_list.draft[1].enabled);
    assert_eq!(store.content(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::RuleList(RuleListAction::Discard));
    let _ = state.update(queued_toggle);
    assert!(!state.editor.rule_list.dirty());
    assert!(
        !state
            .editor
            .rule_list
            .move_rule(first, RuleMoveDirection::Down)
    );
    let retired = state.editor.rule_list.row_id(0).unwrap();
    store.select_profile("next.yaml", "rules:\n  - MATCH,REJECT\n");
    publish(&mut state, &reader);
    let _ = state.update(Message::ToggleRuleEnabled(retired));
    assert_eq!(state.editor.rule_list.draft.len(), 1);
    assert_eq!(state.editor.rule_list.draft[0].rule, "MATCH,REJECT");
    assert!(state.editor.rule_list.draft[0].enabled);
    assert!(!state.editor.rule_list.dirty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn tea_unsent_form_retains_old_source_until_explicit_discard_and_never_stages_into_new_profile() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateNewRulePayload("old-form.test".into()));
    let _ = state.update(Message::UpdateNewRuleTarget("DIRECT".into()));
    store.select_profile("new.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut state, &reader);
    assert_eq!(state.editor.new_rule_payload, "old-form.test");
    assert!(
        !state
            .editor
            .rule_form_binding
            .current(&state.editor.rule_list)
    );
    let _ = state.update(Message::AddCustomRule);
    let _ = state.update(Message::ApplyGameRoutingPresets);
    assert_eq!(state.editor.rule_list.draft.len(), 1);
    assert!(!state.editor.rule_list.dirty());
    assert_eq!(
        state.editor.rule_list.failure.as_ref().unwrap().code,
        ErrorCode::NotReady
    );
    let _ = state.update(Message::RuleList(RuleListAction::DiscardForm));
    assert!(state.editor.new_rule_payload.is_empty());
    assert!(
        state
            .editor
            .rule_form_binding
            .current(&state.editor.rule_list)
    );
    let _ = state.update(Message::UpdateNewRulePayload("new-form.test".into()));
    let _ = state.update(Message::AddCustomRule);
    assert_eq!(
        state.editor.rule_list.draft[0].rule,
        "DOMAIN-SUFFIX,new-form.test,DIRECT"
    );
    assert_eq!(
        state.editor.rule_list.base.as_ref().unwrap().source.profile,
        "new.yaml"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(
        store.content_for("new.yaml").as_deref(),
        Some("rules:\n  - MATCH,DIRECT\n")
    );
}
#[test]
fn tea_list_draft_discard_permission_retry_and_changed_source_never_rewrite_another_profile() {
    let (mut state, reader, store) = setup();
    let original = store.content();
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    assert!(state.editor.rule_list.dirty());
    assert_eq!(store.content(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::RuleList(RuleListAction::Discard));
    assert!(!state.editor.rule_list.dirty());
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    store.deny_write.store(true, Ordering::SeqCst);
    let task = state.update(Message::SaveRules);
    assert_eq!(state.update(Message::SaveRules).units(), 0);
    complete(&mut state, task);
    assert_eq!(
        state.editor.rule_list.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(store.content(), original);
    assert!(state.editor.rule_list.dirty());
    let retained = state.editor.rule_list.draft.clone();
    let _ = state.update(Message::RuleList(RuleListAction::Settings));
    assert_eq!(state.shell.current_route, Route::Settings);
    assert_eq!(state.editor.rule_list.draft, retained);
    store.deny_write.store(false, Ordering::SeqCst);
    let task = state.update(Message::SaveRules);
    complete(&mut state, task);
    assert!(state.editor.rule_list.awaiting_read);
    // Running the separate tracer command also drains its real reader; it does not save the draft.
    let _ = state.update(Message::UpdateRulesTracerInput("example.org".into()));
    run(&mut state, &reader);
    assert!(!state.editor.rule_list.dirty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    let updated = store.content();
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(1).unwrap(),
    ));
    let draft = state.editor.rule_list.draft.clone();
    store.select_profile("second.yaml", "rules:\n  - MATCH,REJECT\n");
    run(&mut state, &reader);
    assert!(state.editor.rule_list.source_changed());
    assert_eq!(state.editor.rule_list.draft, draft);
    assert_eq!(state.update(Message::SaveRules).units(), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.content(), updated);
    let _ = state.update(Message::RuleList(RuleListAction::Discard));
    assert_eq!(
        state.editor.rule_list.base.as_ref().unwrap().source.profile,
        "second.yaml"
    );
    assert_eq!(state.editor.rule_list.draft[0].rule, "MATCH,REJECT");
}

#[test]
fn tea_shell_count_replays_actual_complete_rule_facts_while_an_old_source_draft_is_retained() {
    let (mut state, reader, store) = setup();
    assert_eq!(
        count_copy(&state.shell.readout, PageId::Rules, "en-US"),
        "4"
    );
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    store.select_profile("second.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut state, &reader);
    assert_eq!(state.editor.rule_list.draft.len(), 4);
    assert_eq!(
        count_copy(&state.shell.readout, PageId::Rules, "en-US"),
        "1"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn demo_list_save_runs_the_injected_isolated_owner_and_blocks_without_a_service() {
    let (mut state, _, store) = setup();
    state.shell.demo = true;
    store.deny_write.store(true, Ordering::SeqCst);
    let original = store.content();
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    let task = state.update(Message::SaveRules);
    assert!(state.editor.rule_list.pending.is_some());
    complete(&mut state, task);
    assert_eq!(
        state.editor.rule_list.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert!(state.editor.rule_list.pending.is_none());
    assert!(state.editor.rule_list.can_save());
    assert_eq!(store.content(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    state.commands = None;
    assert_eq!(state.update(Message::SaveRules).units(), 0);
    assert!(state.editor.rule_list.pending.is_none());
}

#[test]
fn tea_rule_rows_retain_source_parameters_invalid_raw_and_exact_search_without_inventing_an_outbound()
 {
    let (mut state, reader, store) = setup();
    store.replace("rules:\n  - IP-CIDR,192.0.2.0/24,DIRECT,src,no-resolve\n  - 'AND,((DOMAIN,example.com)),DIRECT,no-resolve'\n  - MATCH,REJECT\n");
    let _ = state.update(Message::UpdateRulesTracerInput("192.0.2.1".into()));
    run(&mut state, &reader);
    assert!(
        state.editor.rules_render_cache[0].source_ip
            && state.editor.rules_render_cache[0].no_resolve
    );
    assert_eq!(
        state.editor.rule_list.draft[1].rule,
        "AND,((DOMAIN,example.com)),DIRECT,no-resolve"
    );
    assert_eq!(
        state.editor.rules_render_cache[1]
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Configuration
    );
    assert!(state.editor.rules_render_cache[1].target.is_empty());
    let _ = state.update(Message::FilterRules("src,no-resolve".into()));
    assert_eq!(state.editor.rules_filtered_indices, vec![0]);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
