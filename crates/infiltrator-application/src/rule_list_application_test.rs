//! test-intent: behavior
//! Real isolated profile bytes and full documents, never a truncated display save.
use super::*;
use crate::rule_form_binding::RuleFormBinding;
use crate::rule_list_editor::RuleListEditor;
use crate::rule_source_identity::rule_workspace;
use crate::rule_trace_fixtures::{NAMED_TRACE_DOCUMENT, RuleTraceStore};
use infiltrator_contract::rule_edit::RuleMoveDirection;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn staged_edits_cancel_permission_retry_and_atomic_source_change_keep_exact_profile_ownership()
 {
    let store = Arc::new(RuleTraceStore::default());
    store.replace(NAMED_TRACE_DOCUMENT);
    let application = RuleListApplication::new(store.clone());
    let document = document_snapshot(&store.load_rule_workspace().await.unwrap());
    let mut editor = RuleListEditor::default();
    assert!(editor.observe(Some(&document), None));
    assert!(editor.toggle(editor.row_id(0).unwrap()));
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert!(editor.dirty());
    assert!(editor.discard());
    assert!(!editor.dirty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert!(editor.move_rule(editor.row_id(1).unwrap(), RuleMoveDirection::Up));
    let (operation, request) = editor.begin().unwrap();
    assert!(!editor.toggle(editor.row_id(0).unwrap()));
    assert!(!editor.discard());
    store.deny_write.store(true, Ordering::SeqCst);
    let failure = application.commit(&request).await.unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    assert!(editor.finish(operation, Err(failure)));
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert!(editor.dirty());
    let (retry, request) = editor.begin().unwrap();
    assert!(
        !editor.finish(operation, Ok(())),
        "stale completion cannot unlock retry"
    );
    store.deny_write.store(false, Ordering::SeqCst);
    application.commit(&request).await.unwrap();
    assert!(editor.finish(retry, Ok(())));
    assert!(
        !editor.editable(),
        "success waits for an independent reader"
    );
    let updated = document_snapshot(&store.load_rule_workspace().await.unwrap());
    assert!(editor.observe(Some(&updated), None));
    assert!(!editor.dirty());
    assert_eq!(updated.sub_rules, document.sub_rules);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert!(editor.toggle(editor.row_id(0).unwrap()));
    let (_, stale) = editor.begin().unwrap();
    store.select_profile("second.yaml", "rules:\n  - MATCH,REJECT\n");
    assert_eq!(
        application.commit(&stale).await.unwrap_err().code,
        ErrorCode::NotReady
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    let second = document_snapshot(&store.load_rule_workspace().await.unwrap());
    assert_eq!(second.source.profile, "second.yaml");
    assert_eq!(second.rules[0].rule, "MATCH,REJECT");
}

#[tokio::test]
async fn full_document_edits_past_the_publish_cap_cannot_drop_rules_or_accept_invented_targets() {
    let store = Arc::new(RuleTraceStore::default());
    let content = format!(
        "rules:\n{}  - MATCH,DIRECT\n",
        (0..5100)
            .map(|i| format!("  - DOMAIN,node{i}.test,DIRECT\n"))
            .collect::<String>()
    );
    store.replace(&content);
    let document = document_snapshot(&store.load_rule_workspace().await.unwrap());
    assert_eq!(document.rules.len(), 5101);
    let mut editor = RuleListEditor::default();
    editor.observe(Some(&document), None);
    assert!(editor.toggle(editor.row_id(5050).unwrap()));
    let (_, request) = editor.begin().unwrap();
    let application = RuleListApplication::new(store.clone());
    application.commit(&request).await.unwrap();
    let updated = document_snapshot(&store.load_rule_workspace().await.unwrap());
    assert_eq!(updated.rules.len(), 5101);
    assert!(!updated.rules[5050].enabled);
    assert_eq!(updated.rules[5100], document.rules[5100]);
    let mut invalid = RuleListCommit {
        expected_source: updated.source,
        rules: updated.rules,
    };
    invalid.rules[0].rule = "DOMAIN,node0.test,not-a-real-group".into();
    assert_eq!(
        application.commit(&invalid).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_read_changed_source_and_unobserved_success_preserve_draft_until_explicit_discard() {
    let workspace = rule_workspace("first.yaml".into(), NAMED_TRACE_DOCUMENT).unwrap();
    let first = document_snapshot(&workspace);
    let mut editor = RuleListEditor::default();
    editor.observe(Some(&first), None);
    editor.toggle(editor.row_id(0).unwrap());
    let draft = editor.draft.clone();
    editor.observe(
        None,
        Some(Failure::new(ErrorCode::Storage, "unreadable", true)),
    );
    assert_eq!(editor.draft, draft);
    assert!(!editor.can_save());
    assert!(!editor.toggle(editor.row_id(0).unwrap()));
    let second = document_snapshot(
        &rule_workspace("second.yaml".into(), "rules:\n  - MATCH,REJECT\n").unwrap(),
    );
    editor.observe(Some(&second), None);
    assert_eq!(editor.draft, draft);
    assert!(editor.source_changed());
    assert!(!editor.can_save());
    assert!(editor.discard());
    assert_eq!(rule_definitions(&editor.draft), second.rules);
    editor.toggle(editor.row_id(0).unwrap());
    let (operation, _) = editor.begin().unwrap();
    editor.finish(operation, Ok(()));
    editor.observe(Some(&second), None);
    assert!(editor.awaiting_read);
    let changed = document_snapshot(
        &rule_workspace("second.yaml".into(), "rules:\n  - MATCH,DIRECT\n").unwrap(),
    );
    editor.observe(Some(&changed), None);
    assert!(!editor.awaiting_read);
    assert!(editor.source_changed());
    assert_eq!(editor.failure.as_ref().unwrap().code, ErrorCode::NotReady);
    assert!(editor.discard());
    assert_eq!(rule_definitions(&editor.draft), changed.rules);
}

#[test]
fn row_identity_survives_reorder_and_prepend_but_rejects_removed_discarded_and_replaced_rows() {
    let document = document_snapshot(
        &rule_workspace(
            "rows.yaml".into(),
            "rules:\n  - DOMAIN,same.test,DIRECT\n  - DOMAIN,same.test,DIRECT\n  - MATCH,DIRECT\n",
        )
        .unwrap(),
    );
    let mut editor = RuleListEditor::default();
    editor.observe(Some(&document), None);
    let first = editor.row_id(0).unwrap();
    let second = editor.row_id(1).unwrap();
    assert_ne!(
        first, second,
        "duplicate expressions are different editable rows"
    );
    assert!(editor.move_rule(second, RuleMoveDirection::Up));
    assert!(editor.toggle(first));
    assert!(editor.draft[0].enabled);
    assert!(!editor.draft[1].enabled);
    assert!(editor.prepend([RuleEntry {
        rule: "DOMAIN,new.test,DIRECT".into(),
        enabled: true
    }]));
    assert_eq!(editor.row_index(first), Some(2));
    assert!(editor.toggle(second));
    assert!(!editor.draft[1].enabled);
    assert!(editor.remove(second));
    let retained = editor.draft.clone();
    assert!(!editor.toggle(second));
    assert!(!editor.move_rule(second, RuleMoveDirection::Down));
    assert_eq!(editor.draft, retained);
    assert!(editor.discard());
    assert!(
        !editor.toggle(first),
        "discard retires previously rendered controls"
    );
    let replacement = document_snapshot(
        &rule_workspace("other.yaml".into(), "rules:\n  - MATCH,REJECT\n").unwrap(),
    );
    let old = editor.row_id(0).unwrap();
    assert!(editor.observe(Some(&replacement), None));
    assert!(!editor.toggle(old));
    assert_eq!(editor.draft[0].rule, "MATCH,REJECT");
    assert!(editor.draft[0].enabled);
    assert!(!editor.dirty());
}

#[test]
fn unsent_form_source_is_retained_across_refresh_and_only_explicit_reset_rebinds_it() {
    let first =
        document_snapshot(&rule_workspace("first.yaml".into(), NAMED_TRACE_DOCUMENT).unwrap());
    let second =
        document_snapshot(&rule_workspace("second.yaml".into(), NAMED_TRACE_DOCUMENT).unwrap());
    let mut editor = RuleListEditor::default();
    editor.observe(Some(&first), None);
    let mut form = RuleFormBinding::default();
    form.observe(&editor);
    assert!(form.current(&editor));
    form.edit(&editor);
    editor.observe(Some(&second), None);
    form.observe(&editor);
    assert!(!form.current(&editor));
    assert_eq!(
        form.require_current(&editor).unwrap_err().code,
        ErrorCode::NotReady
    );
    assert!(form.status(&editor, "en-US").contains("first.yaml"));
    form.edit(&editor);
    assert!(
        !form.current(&editor),
        "typing cannot silently rebind an old form"
    );
    form.reset(&editor);
    assert!(form.current(&editor));
    editor.observe(
        None,
        Some(Failure::new(ErrorCode::Storage, "unreadable", true)),
    );
    assert!(!form.current(&editor));
    editor.observe(Some(&second), None);
    form.observe(&editor);
    assert!(form.current(&editor));
    assert!(editor.toggle(editor.row_id(0).unwrap()));
    editor.begin().unwrap();
    assert!(!form.current(&editor));
    assert_eq!(
        form.status(&editor, "en-US"),
        "",
        "an in-flight save is not a changed form source"
    );
}
