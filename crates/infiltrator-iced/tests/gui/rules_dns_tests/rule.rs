//! Behavior cases for rule.
//! test-intent: behavior

use super::*;
use crate::view::rules::providers::{
    format_provider_behavior, format_rule_provider_format, providers_view, rule_provider_row,
    total_external_rules,
};
use infiltrator_domain::rules::{RuleProviderDiff, apply_rules_to_yaml, load_rules_from_yaml};
use infiltrator_domain::runtime::RuleProvider;
use infiltrator_shared::locales::Lang;

#[test]
fn test_rule_provider_diff_and_unpack_flow() {
    let (mut state, _) = AppState::new();
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    let diff = RuleProviderDiff {
        provider_name: "GoogleRules".into(),
        local_count: 5,
        remote_count: 7,
        added_rules: vec!["DOMAIN-SUFFIX,googlevideo.com".into()],
        removed_rules: vec![],
        unchanged_count: 5,
    };

    let _ = state.update(Message::RuleProviderDiffLoaded(Ok(diff.clone())));
    assert_eq!(
        state
            .editor
            .inspecting_rule_provider_diff
            .as_ref()
            .unwrap()
            .provider_name,
        "GoogleRules"
    );

    let _ = state.update(Message::InspectRuleProviderDiff(None));
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    // DUAL-11-06: without a declaration in the loaded profile nothing is
    // unpacked and no sample rule is invented.
    let prev_len = state.editor.rule_list.draft.len();
    let _ = state.update(Message::UnpackRuleProvider("GoogleRules".into()));
    assert_eq!(state.editor.rule_list.draft.len(), prev_len);
    assert!(!state.editor.rule_list.dirty());
    assert!(
        state
            .editor
            .provider_unpack
            .status_message
            .as_deref()
            .is_some_and(|status| status.contains("not declared")),
        "undeclared providers must be reported honestly"
    );
    // A diff for an undeclared provider is refused for the same reason.
    let _ = state.update(Message::InspectRuleProviderDiff(Some("GoogleRules".into())));
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    // Verify rule provider row formatting (Domain, IPCIDR, Classical), badges, format chips, and rendering
    let lang = Lang("en");
    let domain_provider = RuleProvider {
        name: "GoogleRules".into(),
        provider_type: "http".into(),
        behavior: "domain".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-09-02 06:00:00".into(),
        rule_count: 1420,
    };
    let ipcidr_provider = RuleProvider {
        name: "geoip-cn.mrs".into(),
        provider_type: "mrs".into(),
        behavior: "ipcidr".into(),
        vehicle_type: "File".into(),
        updated_at: "2026-09-01 12:00:00".into(),
        rule_count: 850,
    };
    let classical_provider = RuleProvider {
        name: "custom-ads.yaml".into(),
        provider_type: "yaml".into(),
        behavior: "classical".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-08-30 18:30:00".into(),
        rule_count: 572,
    };

    assert_eq!(
        format_provider_behavior(&domain_provider.behavior),
        "Domain"
    );
    assert_eq!(
        format_provider_behavior(&ipcidr_provider.behavior),
        "IPCIDR"
    );
    assert_eq!(
        format_provider_behavior(&classical_provider.behavior),
        "Classical"
    );

    assert_eq!(format_rule_provider_format(&domain_provider), "HTTP");
    assert_eq!(format_rule_provider_format(&ipcidr_provider), "MRS");
    assert_eq!(format_rule_provider_format(&classical_provider), "YAML");

    let providers = vec![
        domain_provider.clone(),
        ipcidr_provider.clone(),
        classical_provider.clone(),
    ];
    assert_eq!(total_external_rules(&providers), 1420 + 850 + 572);

    let _dom_elem = rule_provider_row(
        &domain_provider,
        Some("https://example.com/domain.mrs"),
        Some(86_400),
        None,
        &lang,
    );
    let _ipc_elem = rule_provider_row(&ipcidr_provider, None, None, None, &lang);
    let _cls_elem = rule_provider_row(&classical_provider, None, None, None, &lang);

    state.editor.rule_providers = providers;
    let _providers_elem = providers_view(&state, &lang);
}

/// The exact function `Message::SaveRules` runs: comment preservation is a
/// property of the shared domain writer, so the Iced editor inherits it.
#[test]
fn test_rule_save_path_preserves_comments_through_the_shared_fidelity_writer() {
    let source = "\
# 顶层手写注释
mode: rule
rules:
  # 规则块说明
  - MATCH,DIRECT   # 兜底规则
";
    let mut rules = load_rules_from_yaml(source).expect("rules");
    rules.insert(
        0,
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,PROXY".to_string(),
            enabled: false,
        },
    );
    let saved = apply_rules_to_yaml(source, &rules).expect("save");
    assert!(
        saved.contains("# 顶层手写注释"),
        "top comment kept: {saved}"
    );
    assert!(
        saved.contains("# 规则块说明"),
        "block comment kept: {saved}"
    );
    assert!(saved.contains("# 兜底规则"), "inline comment kept: {saved}");
    assert_eq!(
        load_rules_from_yaml(&saved).expect("reload"),
        rules,
        "the saved document still parses to the edited rule list"
    );
}
