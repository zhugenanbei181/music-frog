//! Behavior cases for rules game.
//! test-intent: behavior

use infiltrator_iced::state::editor::GeoDatabasesUpdatePhase;
use infiltrator_presentation::rule_list_editor::RuleListReadback;
use super::*;
use infiltrator_composition::capture_fixtures::list_document;

#[test]
fn test_rules_game_presets_and_geo_update() {
    let (mut state, _) = AppState::new();
    state.editor.rule_list.draft = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];
    let document = list_document(state.editor.rule_list.draft.clone());
    state.editor.rule_list.observe_readback(
        RuleListReadback::Observed(document),
    );
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
    assert!(!state.editor.geo_databases_update_phase.is_updating());

    // The finished handler still owns the busy-flag lifecycle.
    state.editor.geo_databases_update_phase =
        GeoDatabasesUpdatePhase::Updating;
    let _ = state.update(Message::GeoDatabasesUpdated(Ok(())));
    assert!(!state.editor.geo_databases_update_phase.is_updating());
}
