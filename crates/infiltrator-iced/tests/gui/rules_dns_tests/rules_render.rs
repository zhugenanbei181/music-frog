//! Behavior cases for rules render.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;

#[test]
fn test_rules_render_cache_and_filter() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(vec![
        RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,10.0.0.0/8,REJECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,example.net,GLOBAL".into(),
            enabled: true,
        },
    ]))));

    assert_eq!(state.editor.rules_render_cache.len(), 3);
    assert_eq!(state.editor.rules_filtered_indices.len(), 3);
    assert_eq!(state.editor.rules_render_cache[0].payload, "example.com");

    let _ = state.update(Message::FilterRules("example.net".into()));
    assert_eq!(state.editor.rules_page, 0);
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
}
