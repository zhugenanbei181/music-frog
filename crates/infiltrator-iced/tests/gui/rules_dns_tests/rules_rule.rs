//! Behavior cases for rules rule.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::provider_cache::ProviderContentOrigin;
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;

/// DUAL-11-06: the surface never invents a provider payload. The shared
/// application reads the declaration's inline payload and the accepted plan
/// lands in the local draft through the same edit seam as the other rule
/// edits; an unreadable source stays a typed failure.
#[tokio::test]
async fn test_rules_rule_provider_unpack_reads_shared_application() {
    use infiltrator_application::rule_provider_application::RuleProviderApplication;
    use infiltrator_domain::rules::RuleProviders;
    use infiltrator_domain::rules::provider_store::parse_rule_provider_declarations;

    let (mut state, _) = AppState::new();
    state.editor.rule_list.draft = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];
    let document = list_document(state.editor.rule_list.draft.clone());
    state.editor.rule_list.observe(Some(&document), None);
    state.editor.rule_providers_json_cache =
        r#"{"ads":{"type":"inline","behavior":"domain","payload":["ads.com","tracker.net"]}}"#
            .to_owned();

    let providers: RuleProviders =
        serde_json::from_str(&state.editor.rule_providers_json_cache).expect("declarations");
    let declaration = parse_rule_provider_declarations(&providers)
        .into_iter()
        .next()
        .expect("declaration");
    let plan = RuleProviderApplication::default()
        .deconstruct(&declaration, "PROXY", None)
        .await
        .expect("inline plan");
    assert_eq!(plan.origin, ProviderContentOrigin::InlinePayload);
    assert_eq!(plan.imported(), 2);

    let _ = state.update(Message::RuleProviderUnpacked(Ok(plan)));
    assert_eq!(state.editor.rule_list.draft.len(), 3);
    // Same shared reduction as the application path: unpacked rules prepend.
    assert_eq!(
        state.editor.rule_list.draft[0].rule,
        "DOMAIN-SUFFIX,ads.com,PROXY"
    );
    assert_eq!(
        state.editor.rule_list.draft[1].rule,
        "DOMAIN-SUFFIX,tracker.net,PROXY"
    );
    assert_eq!(state.editor.rule_list.draft[2].rule, "MATCH,DIRECT");
    assert!(state.editor.rule_list.dirty());
    assert_eq!(state.editor.provider_unpack.unpacked_rules_count, 2);
    let status = state
        .editor
        .provider_unpack
        .status_message
        .clone()
        .expect("status");
    assert!(status.contains("inline-payload"), "{status}");
    assert!(!state.editor.provider_unpack.is_unpacking);

    // A host without a readable source reports the typed failure verbatim and
    // leaves the draft untouched.
    let failure = RuleProviderApplication::default()
        .deconstruct(
            &RuleProviderDeclaration::from_value(
                "cn",
                &serde_json::json!({
                    "type": "http",
                    "behavior": "domain",
                    "format": "text",
                    "url": "https://example.com/cn.txt"
                }),
            ),
            "PROXY",
            None,
        )
        .await
        .expect_err("no source");
    let _ = state.update(Message::RuleProviderUnpacked(Err(
        InfiltratorError::Config(failure.message.clone()),
    )));
    assert_eq!(state.editor.rule_list.draft.len(), 3);
    assert!(
        state
            .editor
            .provider_unpack
            .status_message
            .as_deref()
            .is_some_and(|status| status.contains("no readable rule list"))
    );
}
