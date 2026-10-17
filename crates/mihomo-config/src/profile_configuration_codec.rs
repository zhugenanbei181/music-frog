//! Typed Mihomo configuration reads and pure document transformations.
//!
//! This adapter owns the YAML grammar and domain-specific section mapping.
//! It never persists bytes or contacts a live core; the application validates
//! the returned mutation against its source-bound profile workspace before
//! committing it.

use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use crate::profile_protocol_codec;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::validate_hosts;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_domain::yaml_edit::rule_location::replace_rule_at_location;
use infiltrator_domain::{dns, dns_hosts, fake_ip, proxy_providers, rules, sniffer, tun};
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationQuery, ProfileConfigurationReadout,
    ProfileConfigurationUpdate, ProfileRuleWorkspaceMutation, ProfileRuleWorkspaceProjection,
    ProfileRuleWorkspaceUpdate,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use serde_yaml_ng::Value;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::str::from_utf8;

pub(crate) fn read(
    identity: &ProfileDocumentIdentity,
    raw_bytes: &[u8],
    query: ProfileConfigurationQuery,
) -> Result<ProfileConfigurationReadout, PortError> {
    ensure_scope(identity)?;
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    let readout = match query {
        ProfileConfigurationQuery::Dns => ProfileConfigurationReadout::Dns(
            dns::extract_dns_config_from_doc(&document).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::FakeIp => ProfileConfigurationReadout::FakeIp(
            fake_ip::extract_fake_ip_config_from_doc(&document).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::Tun => ProfileConfigurationReadout::Tun(
            tun::extract_tun_config_from_doc(&document).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::Rules => ProfileConfigurationReadout::Rules(
            rules::load_rules_from_yaml(content).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::RuleProviders => ProfileConfigurationReadout::RuleProviders(
            rules::extract_rule_providers_from_doc(&document).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::ProxyProviders => ProfileConfigurationReadout::ProxyProviders(
            proxy_providers::extract_proxy_providers_from_doc(&document)
                .map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::Sniffer => ProfileConfigurationReadout::Sniffer(
            sniffer::extract_sniffer_config(&document).map_err(configuration_error)?,
        ),
        ProfileConfigurationQuery::ProxyEndpoint => {
            ProfileConfigurationReadout::ProxyEndpoint(proxy_endpoint_from_document(&document))
        }
        ProfileConfigurationQuery::EtagSupport => {
            ProfileConfigurationReadout::EtagSupport(KernelEtagSupportSnapshot::from_declared(
                document.get("etag-support").and_then(Value::as_bool),
            ))
        }
        ProfileConfigurationQuery::Hosts => ProfileConfigurationReadout::Hosts {
            entries: dns_hosts::entries_from_document(&document, false)
                .map_err(configuration_error)?,
            legacy_entries: dns_hosts::entries_from_document(&document, true)
                .map_err(configuration_error)?,
        },
        ProfileConfigurationQuery::ProtocolNodes => ProfileConfigurationReadout::ProtocolNodes(
            profile_protocol_codec::read_protocol_nodes(raw_bytes)?,
        ),
        ProfileConfigurationQuery::TrustAnchors => ProfileConfigurationReadout::TrustAnchors(
            profile_protocol_codec::read_trust_anchors(raw_bytes)?,
        ),
    };
    Ok(readout)
}

fn proxy_endpoint_from_document(document: &Value) -> Option<String> {
    for (key, scheme) in [
        ("mixed-port", "socks5"),
        ("socks-port", "socks5"),
        ("port", "http"),
    ] {
        if let Some(port) = document.get(key).and_then(proxy_port_from_value) {
            return Some(format!("{scheme}://127.0.0.1:{port}"));
        }
    }
    None
}

fn proxy_port_from_value(value: &Value) -> Option<u16> {
    let port = match value {
        Value::Number(number) => number.as_u64().and_then(|port| u16::try_from(port).ok()),
        Value::String(raw) => raw.trim().parse::<u16>().ok(),
        _ => None,
    }?;
    (port > 0).then_some(port)
}

pub(crate) fn read_rule_workspace(
    workspace: &ProfileWorkspaceIdentity,
    raw_bytes: &[u8],
) -> Result<ProfileRuleWorkspaceProjection, PortError> {
    ensure_scope(&workspace.document)?;
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    let entries = rules::load_rules_from_yaml(content).map_err(configuration_error)?;
    let mut targets = vec!["DIRECT".to_owned(), "REJECT".to_owned()];
    for section in ["proxies", "proxy-groups"] {
        for entry in document
            .get(section)
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                targets.push(name.to_owned());
            }
        }
    }
    let mut sub_rules = BTreeMap::new();
    if let Some(tables) = document.get("sub-rules") {
        let tables = tables
            .as_mapping()
            .ok_or_else(|| configuration_error("sub-rules must be a mapping"))?;
        for (name, raw_entries) in tables {
            let name = name
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    configuration_error("sub-rule table needs a nonempty string name")
                })?;
            let raw_entries = raw_entries.as_sequence().ok_or_else(|| {
                configuration_error(format!("sub-rule table {name} must be a sequence"))
            })?;
            let entries = raw_entries
                .iter()
                .map(|entry| {
                    entry.as_str().map(rules::parse_rule_entry).ok_or_else(|| {
                        configuration_error(format!(
                            "sub-rule table {name} contains a non-string rule"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            sub_rules.insert(name.to_owned(), entries);
        }
    }
    Ok(ProfileRuleWorkspaceProjection {
        source: identify_rules_document(workspace.document.profile_id.as_str().to_owned(), content),
        rules: entries,
        targets,
        sub_rules,
    })
}

pub(crate) fn apply_rule_workspace(
    workspace: &ProfileWorkspaceIdentity,
    raw_bytes: &[u8],
    update: ProfileRuleWorkspaceUpdate,
) -> Result<ProfileRuleWorkspaceMutation, PortError> {
    ensure_scope(&workspace.document)?;
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let updated = match update {
        ProfileRuleWorkspaceUpdate::ReplaceRules(entries) => {
            rules::validate_rules(&entries).map_err(invalid_input)?;
            rules::apply_rules_to_yaml(content, &entries).map_err(configuration_error)?
        }
        ProfileRuleWorkspaceUpdate::ReplaceNamedRule {
            location,
            expected_rule,
            replacement,
        } => replace_rule_at_location(content, &location, &expected_rule, &replacement).map_err(
            |error| PortError::Rejected(Failure::new(ErrorCode::NotReady, error.to_string(), true)),
        )?,
    };
    let document_bytes = updated.into_bytes();
    let readback = read_rule_workspace(workspace, &document_bytes)?;
    Ok(ProfileRuleWorkspaceMutation {
        document_bytes,
        readback,
    })
}

pub(crate) fn apply(
    identity: &ProfileDocumentIdentity,
    raw_bytes: &[u8],
    update: ProfileConfigurationUpdate,
) -> Result<ProfileConfigurationMutation, PortError> {
    ensure_scope(identity)?;
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    // Reject an invalid source before deriving any replacement bytes.
    serde_yaml_ng::from_str::<Value>(content).map_err(configuration_error)?;

    let (document, query) = match update {
        ProfileConfigurationUpdate::Dns(patch) => (
            dns::apply_dns_patch_to_yaml(content, patch).map_err(configuration_error)?,
            ProfileConfigurationQuery::Dns,
        ),
        ProfileConfigurationUpdate::DnsSettings(patch) => (
            apply_dns_settings(content, patch)?,
            ProfileConfigurationQuery::Dns,
        ),
        ProfileConfigurationUpdate::FakeIp(patch) => (
            fake_ip::apply_fake_ip_patch_to_yaml(content, patch).map_err(configuration_error)?,
            ProfileConfigurationQuery::FakeIp,
        ),
        ProfileConfigurationUpdate::Tun(patch) => (
            tun::apply_tun_patch_to_yaml(content, patch).map_err(configuration_error)?,
            ProfileConfigurationQuery::Tun,
        ),
        ProfileConfigurationUpdate::Rules(entries) => {
            rules::validate_rules(&entries).map_err(invalid_input)?;
            (
                rules::apply_rules_to_yaml(content, &entries).map_err(configuration_error)?,
                ProfileConfigurationQuery::Rules,
            )
        }
        ProfileConfigurationUpdate::RuleProviders(providers) => (
            rules::apply_rule_providers_to_yaml(content, &providers)
                .map_err(configuration_error)?,
            ProfileConfigurationQuery::RuleProviders,
        ),
        ProfileConfigurationUpdate::ProxyProviders(providers) => (
            proxy_providers::apply_proxy_providers_to_yaml(content, &providers)
                .map_err(configuration_error)?,
            ProfileConfigurationQuery::ProxyProviders,
        ),
        ProfileConfigurationUpdate::Sniffer(config) => (
            sniffer::apply_typed_sniffer_to_yaml(content, &config).map_err(configuration_error)?,
            ProfileConfigurationQuery::Sniffer,
        ),
        ProfileConfigurationUpdate::UpsertProtocolNode(draft) => {
            return profile_protocol_codec::upsert_protocol_node(raw_bytes, draft);
        }
        ProfileConfigurationUpdate::TrustAnchors(params) => {
            return profile_protocol_codec::apply_trust_anchors(raw_bytes, params);
        }
    };
    let document_bytes = document.into_bytes();
    let readback = read(identity, &document_bytes, query)?;
    Ok(ProfileConfigurationMutation {
        document_bytes,
        readback,
    })
}

fn apply_dns_settings(content: &str, patch: DnsSettingsPatch) -> Result<String, PortError> {
    if patch.clear_hosts && patch.hosts.is_some()
        || patch.remove_legacy_hosts && patch.hosts.is_none() && !patch.clear_hosts
    {
        return Err(invalid_input("ambiguous Hosts patch"));
    }
    if let Some(rows) = &patch.hosts
        && let Some(issue) = validate_hosts(rows).first()
    {
        return Err(invalid_input(format!("invalid Hosts mapping: {issue:?}")));
    }

    let hosts = if patch.clear_hosts {
        Some(Vec::new())
    } else {
        patch.hosts.clone()
    };
    let remove_legacy = patch.remove_legacy_hosts;
    let dns_patch = dns_patch_from_settings(patch);
    let mut updated = if dns_patch == dns::DnsConfigPatch::default() {
        content.to_owned()
    } else {
        dns::apply_dns_patch_to_yaml(content, dns_patch).map_err(configuration_error)?
    };
    if let Some(rows) = hosts {
        updated = dns_hosts::apply_hosts_patch_to_yaml(&updated, &rows, remove_legacy)
            .map_err(configuration_error)?;
    }
    Ok(updated)
}

fn dns_patch_from_settings(patch: DnsSettingsPatch) -> dns::DnsConfigPatch {
    let mut domain_patch = dns::DnsConfigPatch::default();
    if let Some(switches) = patch.switches {
        domain_patch.enable = Some(switches.enable);
        domain_patch.ipv6 = Some(switches.ipv6);
        domain_patch.cache = Some(switches.cache);
        domain_patch.use_hosts = Some(switches.use_hosts);
        domain_patch.use_system_hosts = Some(switches.use_system_hosts);
        domain_patch.respect_rules = Some(switches.respect_rules);
    }
    if let Some(mode) = patch.enhanced_mode {
        match mode.config_value() {
            Some(value) => domain_patch.enhanced_mode = Some(value.to_owned()),
            None => domain_patch.clear_enhanced_mode = true,
        }
    }
    if let Some(mode) = patch.filter_mode {
        domain_patch.fake_ip_filter_mode = Some(mode.config_value().to_owned());
    }
    if let Some(nameserver) = patch.nameserver {
        domain_patch.nameserver = Some(nameserver);
    }
    if let Some(fallback) = patch.fallback {
        domain_patch.fallback = Some(fallback);
    }
    if let Some(policy) = patch.fallback_policy {
        domain_patch.fallback_filter_partial = Some(dns::FallbackFilterPatch {
            geoip: Some(policy.geoip),
            // Empty geoip-code preserves the current value because the host
            // rejects an empty code; an empty trigger list clears the list.
            geoip_code: (!policy.geoip_code.is_empty()).then_some(policy.geoip_code),
            ipcidr: Some(policy.trigger_ipcidr),
        });
    }
    if let Some(range) = patch.fake_ip_range {
        domain_patch.fake_ip_range = Some(range);
    }
    if patch.clear_fake_ip_range {
        domain_patch.clear_fake_ip_range = true;
    }
    if let Some(filter) = patch.fake_ip_filter {
        domain_patch.fake_ip_filter = Some(filter);
    }
    if let Some(servers) = patch.proxy_server_nameserver {
        domain_patch.proxy_server_nameserver = Some(servers);
    }
    if let Some(servers) = patch.direct_nameserver {
        domain_patch.direct_nameserver = Some(servers);
    }
    domain_patch
}

fn ensure_scope(identity: &ProfileDocumentIdentity) -> Result<(), PortError> {
    if identity.engine_id != crate::engine_id()
        || identity.format_id.as_str() != PROFILE_FORMAT
        || identity.schema_version.value() != PROFILE_SCHEMA_VERSION
    {
        return Err(PortError::unsupported(
            Capability::Profiles,
            format!(
                "Mihomo configuration codec does not support engine `{}`, format `{}`, schema {}",
                identity.engine_id.as_str(),
                identity.format_id.as_str(),
                identity.schema_version.value()
            ),
        ));
    }
    Ok(())
}

fn invalid_input(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        error.to_string(),
        false,
    ))
}

fn configuration_error(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        error.to_string(),
        false,
    ))
}

#[cfg(test)]
#[path = "profile_configuration_codec_tests.rs"]
mod tests;
