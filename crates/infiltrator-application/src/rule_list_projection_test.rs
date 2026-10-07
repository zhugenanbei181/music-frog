//! test-intent: behavior
//! A draft cannot inherit trace observations from another source or unpublished rows.
use super::*;
use crate::rule_list_fixtures::list_document;
use infiltrator_contract::rule_edit::RuleMoveDirection;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;

fn observed_page() -> RulesPageSnapshot {
    let rule = RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    };
    let document = list_document(vec![rule.clone()]);
    RulesPageSnapshot {
        document: Some(document.clone()),
        total_rules: 1,
        default_action: "DIRECT".into(),
        providers: Vec::new(),
        rules: vec![rule_snapshot(1, &rule, Some(7), Some(42), None)],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: Some(RuleHitAuditSnapshot {
            source: Some(document.source),
            total_hits: 7,
            ..Default::default()
        }),
        rule_publish_limit: 0,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    }
}

#[test]
fn draft_row_trace_observations_are_bound_to_the_exact_document_source() {
    let page = observed_page();
    let mut editor = RuleListEditor::default();
    editor.observe(page.document.as_ref(), None);
    editor.toggle(editor.row_id(0).unwrap());
    let draft = editor.draft.clone();
    let same = draft_page(&page, &editor);
    assert_eq!(same.rules[0].hit_count, Some(7));
    assert_eq!(same.rules[0].last_hit_secs, Some(42));
    assert!(!same.rules[0].is_enabled);

    for change_profile in [false, true] {
        let mut other = page.clone();
        let source = &mut other.document.as_mut().unwrap().source;
        if change_profile {
            source.profile = "other.yaml".into();
        } else {
            source.document_hash = "different-content-hash".into();
        }
        other.rules[0].hit_count = Some(99);
        other.rules[0].last_hit_secs = Some(88);
        editor.observe(other.document.as_ref(), None);
        assert!(editor.source_changed());
        assert_eq!(editor.draft, draft);
        let projected = draft_page(&other, &editor);
        assert_eq!(projected.document, page.document);
        assert_eq!(projected.rules[0].hit_count, None);
        assert_eq!(projected.rules[0].last_hit_secs, None);
        assert_eq!(projected.rules[0].raw, "MATCH,DIRECT");
        assert_eq!(projected.hit_audit, None);
    }
}

#[test]
fn duplicate_rule_text_keeps_each_original_rows_observation_after_reorder() {
    let mut page = observed_page();
    let rule = RuleEntry {
        rule: "DOMAIN,example.com,DIRECT".into(),
        enabled: true,
    };
    page.document = Some(list_document(vec![rule.clone(), rule.clone()]));
    page.rules = vec![
        rule_snapshot(1, &rule, Some(0), None, None),
        rule_snapshot(2, &rule, Some(7), Some(42), None),
    ];
    let mut editor = RuleListEditor::default();
    editor.observe(page.document.as_ref(), None);
    let second = editor.row_id(1).unwrap();
    assert_eq!(draft_page(&page, &editor).rules[0].hit_count, Some(0));
    assert_eq!(draft_page(&page, &editor).rules[1].hit_count, Some(7));
    assert!(editor.move_rule(second, RuleMoveDirection::Up));
    let moved = draft_page(&page, &editor);
    assert_eq!(moved.rules[0].edit_id, Some(second));
    assert_eq!(moved.rules[0].hit_count, Some(7));
    assert_eq!(moved.rules[1].hit_count, Some(0));
    assert_eq!(moved.rules[1].last_hit_secs, None);
}

#[test]
fn published_zero_unknown_and_unpublished_rows_remain_distinct() {
    let mut page = observed_page();
    let mut editor = RuleListEditor::default();
    editor.observe(page.document.as_ref(), None);
    page.rules[0].hit_count = Some(0);
    assert_eq!(draft_page(&page, &editor).rules[0].hit_count, Some(0));
    page.rules[0].hit_count = None;
    assert_eq!(draft_page(&page, &editor).rules[0].hit_count, None);
    page.rules.clear();
    let unpublished = draft_page(&page, &editor);
    assert_eq!(unpublished.rules.len(), 1);
    assert_eq!(unpublished.rules[0].hit_count, None);
    assert_eq!(unpublished.rules[0].last_hit_secs, None);
}
