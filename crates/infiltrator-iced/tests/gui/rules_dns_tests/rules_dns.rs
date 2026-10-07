//! Behavior cases for rules dns.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;

#[test]
fn test_rules_dns_lazy_editor_state() {
    let (mut state, _) = AppState::new();

    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Unloaded
    );
    state.editor.rule_providers_json_cache = "{\"a\":1}".into();
    let _ = state.update(Message::EnsureRuleProvidersEditorLoaded);
    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Loaded
    );
    assert_eq!(state.editor.rule_providers_json_content.text(), "{\"a\":1}");

    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Unloaded);
    state.editor.dns_json_cache = "{\"enable\":true}".into();
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.dns_json_content.text(), "{\"enable\":true}");
}

#[test]
fn test_rules_dns_large_sample_smoke() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..3200)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));
    assert_eq!(state.editor.rules_render_cache.len(), 3200);

    let large_json = "a".repeat(1024 * 1024);
    state.editor.dns_json_cache = format!("{{\"dns\":\"{}\"}}", large_json);
    state.editor.fake_ip_json_cache = format!("{{\"fake\":\"{}\"}}", large_json);
    state.editor.tun_json_cache = format!("{{\"tun\":\"{}\"}}", large_json);
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    let _ = state.update(Message::EnsureFakeIpEditorLoaded);
    let _ = state.update(Message::EnsureTunEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.fake_ip_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.tun_editor_state, EditorLazyState::Loaded);
}
