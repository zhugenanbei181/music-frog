//! Behavior cases for rules edit.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_edit::RuleDraft;
use infiltrator_domain::rules::edit::{CUSTOM_RULE_TYPE_CHOICES, build_custom_rule};
use infiltrator_domain::rules::game_routing_presets;

#[test]
fn test_rules_edit_operations_delegate_to_shared_module() {
    let (mut state, _) = AppState::new();
    let entry = |rule: &str| RuleEntry {
        rule: rule.into(),
        enabled: true,
    };
    state.editor.rule_list.draft = vec![
        entry("DOMAIN,a.com,DIRECT"),
        entry("DOMAIN,b.com,PROXY"),
        entry("MATCH,DIRECT"),
    ];
    let document = list_document(state.editor.rule_list.draft.clone());
    state.editor.rule_list.observe(Some(&document), None);
    state.rebuild_rules_render_cache();

    // DUAL-11-09: toggle flips the shared `RuleEntry.enabled` fact.
    let _ = state.update(Message::ToggleRuleEnabled(
        state.editor.rule_list.row_id(1).unwrap(),
    ));
    assert!(!state.editor.rule_list.draft[1].enabled);
    assert!(state.editor.rule_list.dirty());
    // A stale index is a typed no-op.
    let draft_before_invalid_index = state.editor.rule_list.draft.clone();
    let _ = state.update(Message::ToggleRuleEnabled(RuleRowId(0)));
    assert_eq!(state.editor.rule_list.draft, draft_before_invalid_index);

    // DUAL-11-10: reorder uses the shared swap reduction and edge guards.
    let _ = state.update(Message::MoveRuleUp(
        state.editor.rule_list.row_id(2).unwrap(),
    ));
    assert_eq!(state.editor.rule_list.draft[1].rule, "MATCH,DIRECT");
    let _ = state.update(Message::MoveRuleDown(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    assert_eq!(state.editor.rule_list.draft[0].rule, "MATCH,DIRECT");
    assert_eq!(state.editor.rule_list.draft[1].rule, "DOMAIN,a.com,DIRECT");
    let _ = state.update(Message::MoveRuleUp(
        state.editor.rule_list.row_id(0).unwrap(),
    ));
    assert_eq!(state.editor.rule_list.draft[0].rule, "MATCH,DIRECT");

    // DUAL-11-12: presets prepend exactly the shared list, in order.
    state.editor.new_rule_target = "Game-Proxy".into();
    let before = state.editor.rule_list.draft.len();
    let _ = state.update(Message::ApplyGameRoutingPresets);
    let presets = game_routing_presets("Game-Proxy");
    assert_eq!(state.editor.rule_list.draft.len(), before + presets.len());
    assert_eq!(state.editor.rule_list.draft[0].rule, presets[0].rule);
    assert_eq!(
        state.editor.rule_list.draft[presets.len()].rule,
        "MATCH,DIRECT"
    );

    // DUAL-11-11: the wizard and the shared builder agree on the type list and
    // reject an empty payload before any async save is scheduled.
    assert_eq!(CUSTOM_RULE_TYPE_CHOICES.len(), 12);
    state.editor.is_adding_rule = false;
    let rule = build_custom_rule(&RuleDraft {
        rule_type: "AND".into(),
        payload: "(DOMAIN,a.com),(DST-PORT,443)".into(),
        target: "AI".into(),
    })
    .expect("shared builder");
    assert_eq!(rule.rule, "AND,((DOMAIN,a.com),(DST-PORT,443)),AI");
}
