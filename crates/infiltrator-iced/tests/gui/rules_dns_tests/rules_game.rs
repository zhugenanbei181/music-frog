//! Behavior cases for rules game.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;

#[test]
fn test_rules_game_presets_and_geo_update() {
    let (mut state, _) = AppState::new();
    state.editor.rule_list.draft = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];
    let document = list_document(state.editor.rule_list.draft.clone());
    state.editor.rule_list.observe(Some(&document), None);
    state.editor.new_rule_target = "Game-Proxy".into();

    let _ = state.update(Message::ApplyGameRoutingPresets);
    assert!(state.editor.rule_list.draft.len() > 5);
    assert!(
        state.editor.rule_list.draft[0]
            .rule
            .contains("PROCESS-NAME")
    );
    assert!(state.editor.rule_list.draft[0].rule.contains("Game-Proxy"));
    assert!(state.editor.rule_list.dirty());

    // Without a composed runtime the geo update is refused with a typed
    // toast instead of a fabricated sleep-then-success.
    let _ = state.update(Message::UpdateGeoDatabases);
    assert!(!state.editor.is_updating_geo_databases);

    // The finished handler still owns the busy-flag lifecycle.
    state.editor.is_updating_geo_databases = true;
    let _ = state.update(Message::GeoDatabasesUpdated(Ok(())));
    assert!(!state.editor.is_updating_geo_databases);
}
