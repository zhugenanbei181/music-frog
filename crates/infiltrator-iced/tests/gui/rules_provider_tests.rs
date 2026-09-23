use super::*;
use async_trait::async_trait;
use infiltrator_contract::provider_cache::{
    ProviderCachePurge, ProviderContentOrigin, RuleProviderCacheSnapshot,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::{ProviderCacheEntry, RuleProviderCachePort};
use std::sync::Arc;

struct FakeCache {
    bytes: Option<Vec<u8>>,
}

#[async_trait]
impl RuleProviderCachePort for FakeCache {
    async fn read_provider(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError> {
        Ok(self.bytes.as_ref().map(|bytes| ProviderCacheEntry {
            origin: ProviderContentOrigin::KernelCacheFile,
            path: None,
            bytes: bytes.clone(),
        }))
    }

    async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
        Ok(ProviderCachePurge {
            directory: Some("/kernel/rules".to_owned()),
            files_removed: 3,
            bytes_freed: 300,
        })
    }

    async fn fingerprint(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<infiltrator_ports::rule_provider_cache::ProviderFileFact>, PortError> {
        Ok(None)
    }

    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
        Ok(RuleProviderCacheSnapshot::ready("/kernel/rules", 0, 0))
    }
}

#[test]
fn declarations_come_from_the_loaded_profile_json_only() {
    let (mut state, _) = AppState::new();
    assert!(state.declared_rule_providers().is_empty());
    state.editor.rule_providers_json_cache = "{}".to_owned();
    assert!(state.declared_rule_providers().is_empty());
    state.editor.rule_providers_json_cache =
        r#"{"ads":{"type":"inline","behavior":"domain","payload":["ads.com"]}}"#.to_owned();
    let declarations = state.declared_rule_providers();
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0].name, "ads");
    assert!(declarations[0].is_inline());
}

#[test]
fn unpack_without_a_declaration_reports_an_honest_status() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UnpackRuleProviderToCustom("absent".to_owned()));
    assert!(state.editor.rules.is_empty());
    assert!(!state.editor.rules_dirty);
    assert!(!state.editor.provider_unpack.is_unpacking);
    let status = state
        .editor
        .provider_unpack
        .status_message
        .clone()
        .expect("status");
    assert!(status.contains("absent"), "{status}");
}

#[test]
fn purge_without_a_port_reports_a_typed_error() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::PurgeRuleProviderCache);
    assert!(!state.editor.provider_unpack.is_purging_cache);
    let status = state
        .editor
        .provider_unpack
        .status_message
        .clone()
        .expect("status");
    assert!(status.contains("cache location"), "{status}");
}

#[tokio::test]
async fn unpack_reads_the_host_cache_through_the_injected_port() {
    let (mut state, _) = AppState::new();
    state.editor.rule_providers_json_cache = r#"{"cn":{"type":"http","behavior":"domain","format":"text","url":"https://example.com/cn.txt"}}"#.to_owned();
    state.runtime.rule_provider_cache_port = Some(Arc::new(FakeCache {
        bytes: Some(b"cached.cn\n".to_vec()),
    }));
    state.editor.rules = vec![infiltrator_domain::rules::RuleEntry {
        rule: "MATCH,DIRECT".to_owned(),
        enabled: true,
    }];
    // The shared application service is exactly what the handler drives;
    // the async result then lands in `finish_rule_provider_unpack`.
    let declaration = state.declared_rule_provider("cn").expect("declaration");
    let plan = RuleProviderApplication::new(state.runtime.rule_provider_cache_port.clone())
        .deconstruct(&declaration, "PROXY", None)
        .await
        .expect("plan");
    assert_eq!(plan.origin, ProviderContentOrigin::KernelCacheFile);
    assert_eq!(plan.entries[0].rule, "DOMAIN-SUFFIX,cached.cn,PROXY");

    let _ = state.finish_rule_provider_unpack(Ok(plan));
    assert_eq!(state.editor.rules[0].rule, "DOMAIN-SUFFIX,cached.cn,PROXY");
    assert_eq!(state.editor.rules[1].rule, "MATCH,DIRECT");
    assert!(state.editor.rules_dirty);
    assert_eq!(state.editor.provider_unpack.unpacked_rules_count, 1);
    assert!(
        state
            .editor
            .provider_unpack
            .status_message
            .as_deref()
            .is_some_and(|status| status.contains("kernel-cache-file"))
    );
}
