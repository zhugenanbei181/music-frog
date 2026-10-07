//! Rules-page projection for the surface reader.
//!
//! Owns the rules tracer replay inputs, the JSON-document serialisation and
//! the pure assembly of `RulesPageSnapshot` from the loaded sections.

use super::projections::missing;
use super::*;
use crate::rule_list_application::document_snapshot;
use crate::rule_list_projection::rule_snapshot;
use crate::rule_provider_application::RuleProviderApplication;
use crate::rule_tracer_application::RuleTracerApplication;
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;
use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
use infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot;
use infiltrator_domain::proxy_providers::ProxyProviders;
use infiltrator_domain::rules::RuleProviders;
use infiltrator_domain::rules::analyzer::{ShadowedRuleWarning, find_shadowed_rules};
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;
use infiltrator_domain::rules::view::{RULE_PUBLISH_LIMIT, published_rule_count};
use infiltrator_domain::runtime::RuleProvider;

/// Inputs the live rule tracer replays against: the shared engine plus the
/// runtime facts that resolve the outbound stage of the decision chain.
pub(super) struct RulesTracerReplay<'a> {
    pub(super) application: &'a RuleTracerApplication,
}

/// DUAL-11-14: serialise the rules-workspace documents the shared reader
/// publishes. Pure over the loaded sections, so a section the host cannot read
/// (`None`) is omitted rather than published as an empty document.
pub(super) fn rules_json_documents(
    rule_providers: Option<RuleProviders>,
    proxy_providers: Option<ProxyProviders>,
    sniffer: Option<serde_json::Value>,
) -> Vec<RulesJsonDocumentSnapshot> {
    use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};

    let mut documents = Vec::new();
    if let Some(providers) = rule_providers
        && let Ok(json) = serde_json::to_string_pretty(&providers)
    {
        documents.push(RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            json,
        });
    }
    if let Some(providers) = proxy_providers
        && let Ok(json) = serde_json::to_string_pretty(&providers)
    {
        documents.push(RulesJsonDocumentSnapshot {
            section: RulesJsonSection::ProxyProviders,
            json,
        });
    }
    if let Some(config) = sniffer
        && let Ok(json) = serde_json::to_string_pretty(&config)
    {
        documents.push(RulesJsonDocumentSnapshot {
            section: RulesJsonSection::Sniffer,
            json,
        });
    }
    documents
}

pub(super) async fn build_rules_page(
    configuration: Option<&ConfigurationApplication>,
    rule_provider: &RuleProviderApplication,
    runtime_providers: Option<Result<Vec<RuleProvider>, PortError>>,
    tracer_replay: RulesTracerReplay<'_>,
    mrs_acceleration: MrsAccelerationSnapshot,
    provider_cache: RuleProviderCacheSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::RulesPageSnapshot> {
    let Some(configuration) = configuration else {
        return surface_snapshot::PageData::unavailable(missing("configuration application"));
    };
    let workspace = match configuration.load_rule_workspace().await {
        Ok(workspace) => workspace,
        Err(error) => return surface_snapshot::PageData::failed(error),
    };
    let document = document_snapshot(&workspace);
    let rules = workspace.rules;
    // DUAL-11-04: the runtime controller reports a provider's behavior and
    // update time but not its source address. Merge the active profile's
    // `rule-providers` declarations so config-backed providers publish their
    // real URL; runtime-only providers stay honestly `None`.
    let configured_providers = configuration
        .load_rule_providers()
        .await
        .unwrap_or_default();
    // DUAL-11-05: the kernel's real `etag-support` capability declared by the
    // active profile (a top-level key; mihomo defaults it to `true`). This is a
    // declaration fact: the per-request `304` outcome stays inside the kernel.
    let etag_support = configuration.load_etag_support().await.unwrap_or_default();
    // The tracer replays the exact rule list rendered below; the query comes
    // from the shared engine so both surfaces observe the same simulation.
    let tracer = tracer_replay.application.replay(&workspace.source);
    let providers = match runtime_providers {
        Some(Ok(providers)) => {
            let mut snapshots = Vec::with_capacity(providers.len());
            for provider in providers {
                let declaration = configured_providers.get(&provider.name);
                let source_url = declaration
                    .and_then(|value| value.get("url"))
                    .and_then(|url| url.as_str())
                    .map(str::to_owned);
                // DUAL-11-05: the declared automatic-refresh interval. The
                // kernel performs the scheduled refresh and owns the
                // `ETag`/`If-None-Match` cache behind it; the client publishes
                // only what the profile declares.
                let refresh_interval_secs = declaration
                    .and_then(|value| value.get("interval"))
                    .and_then(|interval| interval.as_u64());
                // DUAL-11-05: the client's own local-cache observation. This is
                // the file on disk, compared with the previous local read — not
                // an HTTP validator result, which the kernel never exposes.
                let cache_fingerprint = match declaration {
                    Some(value) => {
                        let resolved = RuleProviderDeclaration::from_value(&provider.name, value);
                        rule_provider.observe_fingerprint(&resolved).await
                    }
                    None => None,
                };
                snapshots.push(RuleProviderSnapshot {
                    name: provider.name,
                    rule_count: provider.rule_count as usize,
                    behavior: provider.behavior,
                    updated_at: provider.updated_at,
                    source_url,
                    refresh_interval_secs,
                    cache_fingerprint,
                });
            }
            snapshots
        }
        Some(Err(error)) => {
            return surface_snapshot::PageData::failed(Failure::new(
                ErrorCode::Network,
                error.to_string(),
                true,
            ));
        }
        None => Vec::new(),
    };

    // DUAL-11-14: publish the same JSON documents the Iced editors load, so
    // the Bevy JSON partition edits the identical text through the shared
    // `ApplyRulesJsonDocument` use-case. A section that cannot be read is
    // omitted instead of being published as a fabricated document.
    let json_documents = rules_json_documents(
        configuration.load_rule_providers().await.ok(),
        configuration.load_proxy_providers().await.ok(),
        configuration.load_sniffer_config().await.ok(),
    );

    let shadow_warnings = find_shadowed_rules(&rules);
    let shadow_map: HashMap<usize, &ShadowedRuleWarning> =
        shadow_warnings.iter().map(|w| (w.index, w)).collect();

    let statistics = tracer_replay
        .application
        .statistics_for(&workspace.source, &rules);
    let mut entries = rules
        .into_iter()
        .enumerate()
        .map(|(id, rule)| {
            let hit = statistics.as_ref().map(|readout| readout.row_count(id));
            let last_hit = statistics
                .as_ref()
                .and_then(|readout| readout.row_timestamp(id));
            rule_snapshot(id + 1, &rule, hit, last_hit, shadow_map.get(&id).copied())
        })
        .collect::<Vec<_>>();

    let hit_audit = statistics.map(|readout| readout.audit);

    let default_action = entries
        .last()
        .map(|entry| entry.proxy.clone())
        .unwrap_or_else(|| "—".to_owned());
    // DUAL-11-08: publish both the profile's rule count and the honest cap on
    // the rendered list, so a truncated view is never reported as complete.
    let total_rules = entries.len();
    entries.truncate(published_rule_count(total_rules));
    let data = surface_snapshot::RulesPageSnapshot {
        document: Some(document),
        total_rules,
        default_action,
        providers,
        rules: entries,
        tracer,
        mrs_acceleration,
        hit_audit,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache,
        etag_support,
        json_documents,
    };
    if total_rules == 0 {
        surface_snapshot::PageData::empty(data)
    } else {
        surface_snapshot::PageData::ready(data)
    }
}
