//! Rules-page projection for the surface reader.
//!
//! Owns the rules tracer replay inputs, the JSON-document serialisation and
//! the pure assembly of `RulesPageSnapshot` from the loaded sections.

use super::projections::missing;
use super::*;

/// Inputs the live rule tracer replays against: the shared engine plus the
/// runtime facts that resolve the outbound stage of the decision chain.
pub(super) struct RulesTracerReplay<'a> {
    pub(super) application: &'a crate::rule_tracer_application::RuleTracerApplication,
    pub(super) core: &'a infiltrator_contract::snapshot::CoreSnapshot,
    pub(super) active_exit: Option<&'a infiltrator_contract::active_exit::ActiveExitSnapshot>,
    pub(super) proxies: Option<&'a HashMap<String, Proxy>>,
}

/// DUAL-11-14: serialise the rules-workspace documents the shared reader
/// publishes. Pure over the loaded sections, so a section the host cannot read
/// (`None`) is omitted rather than published as an empty document.
pub(super) fn rules_json_documents(
    rule_providers: Option<infiltrator_domain::rules::RuleProviders>,
    proxy_providers: Option<infiltrator_domain::proxy_providers::ProxyProviders>,
    sniffer: Option<serde_json::Value>,
) -> Vec<infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot> {
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
    rule_provider: &crate::rule_provider_application::RuleProviderApplication,
    runtime_providers: Option<Result<Vec<infiltrator_domain::runtime::RuleProvider>, PortError>>,
    tracer_replay: RulesTracerReplay<'_>,
    mrs_acceleration: infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot,
    provider_cache: infiltrator_contract::provider_cache::RuleProviderCacheSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::RulesPageSnapshot> {
    let Some(configuration) = configuration else {
        return surface_snapshot::PageData::unavailable(missing("configuration application"));
    };
    let rules = match configuration.load_rules().await {
        Ok(rules) => rules,
        Err(error) => return surface_snapshot::PageData::failed(error),
    };
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
    let tracer = tracer_replay.application.project(
        tracer_replay.core,
        &rules,
        tracer_replay.active_exit,
        tracer_replay.proxies,
    );
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
                        let resolved = infiltrator_domain::rules::provider_store::RuleProviderDeclaration::from_value(&provider.name, value);
                        rule_provider.observe_fingerprint(&resolved).await
                    }
                    None => None,
                };
                snapshots.push(surface_snapshot::RuleProviderSnapshot {
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

    let shadow_warnings = infiltrator_domain::rules::analyzer::find_shadowed_rules(&rules);
    let shadow_map: HashMap<usize, &infiltrator_domain::rules::analyzer::ShadowedRuleWarning> =
        shadow_warnings.iter().map(|w| (w.index, w)).collect();

    let mut entries = rules
        .into_iter()
        .enumerate()
        .map(|(id, rule)| {
            let hit = tracer_replay.application.hit_count_for(&rule.rule);
            let last_hit = tracer_replay.application.last_hit_for(&rule.rule);
            rule_snapshot(id + 1, rule, hit, last_hit, shadow_map.get(&id).copied())
        })
        .collect::<Vec<_>>();

    let total_hits = tracer.hit_audit.total_hits;

    let default_action = entries
        .last()
        .map(|entry| entry.proxy.clone())
        .unwrap_or_else(|| "—".to_owned());
    // DUAL-11-08: publish both the profile's rule count and the honest cap on
    // the rendered list, so a truncated view is never reported as complete.
    let total_rules = entries.len();
    entries.truncate(infiltrator_domain::rules::view::published_rule_count(
        total_rules,
    ));
    let data = surface_snapshot::RulesPageSnapshot {
        total_rules,
        default_action,
        providers,
        rules: entries,
        tracer,
        mrs_acceleration,
        total_hits,
        rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
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

fn rule_snapshot(
    id: usize,
    rule: RuleEntry,
    hit_count: u64,
    last_hit_secs: Option<u64>,
    shadow_warning: Option<&infiltrator_domain::rules::analyzer::ShadowedRuleWarning>,
) -> surface_snapshot::RuleSnapshot {
    let (rule_type, payload, proxy, no_resolve) = if let Ok(parsed) = parse_rule_str(&rule.rule) {
        let name = parsed.rule_type.name().to_string();
        let payload = parsed.rule_type.payload().unwrap_or("").to_string();
        (name, payload, parsed.target, parsed.no_resolve)
    } else {
        let mut parts = rule.rule.splitn(3, ',');
        let r_type = parts.next().unwrap_or_default().to_owned();
        let r_payload = parts.next().unwrap_or_default().to_owned();
        let r_proxy = parts.next().unwrap_or_default().to_owned();
        (r_type, r_payload, r_proxy, false)
    };

    surface_snapshot::RuleSnapshot {
        id,
        rule_type,
        payload,
        proxy,
        hit_count,
        is_enabled: rule.enabled,
        no_resolve,
        last_hit_secs,
        is_shadowed: shadow_warning.is_some(),
        shadow_reason: shadow_warning.map(|w| w.reason.to_string()),
    }
}
