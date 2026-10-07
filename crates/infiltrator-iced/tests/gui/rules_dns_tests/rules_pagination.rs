//! Behavior cases for rules pagination.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;

#[test]
fn test_rules_pagination_bounds() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..450)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));
    assert_eq!(state.editor.rules_page_size, 200);

    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage); // should clamp to last
    assert_eq!(state.editor.rules_page, 2);

    let _ = state.update(Message::RulesPrevPage);
    assert_eq!(state.editor.rules_page, 1);

    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(state.editor.rules_page, 2);
}
