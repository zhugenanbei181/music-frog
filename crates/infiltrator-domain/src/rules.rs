//! Rule management, static analysis, and tracer simulation for Mihomo / Clash.Meta.
//!
//! Provides typed access to the `rules:` and `rule-providers:` sections of a
//! profile document, comprehensive rule parsing, shadow rule detection, and
//! traffic routing simulation.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::{BTreeMap, HashSet};

pub mod analyzer;
pub mod tracer;
pub mod types;

pub type RuleProviders = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProvidersPayload {
    pub providers: RuleProviders,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEntry {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesPayload {
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleProviderDiff {
    pub provider_name: String,
    pub local_count: usize,
    pub remote_count: usize,
    pub added_rules: Vec<String>,
    pub removed_rules: Vec<String>,
    pub unchanged_count: usize,
}

pub type TrafficContext = tracer::TrafficContext;
pub type RuleTraceMatch = tracer::RuleTraceMatch;

pub fn trace_rules(
    rules: &[RuleEntry],
    context: &tracer::TrafficContext,
) -> Option<tracer::RuleTraceMatch> {
    tracer::trace_rules(rules, context)
}

pub fn diff_rule_provider_contents(
    provider_name: &str,
    local: &[String],
    remote: &[String],
) -> RuleProviderDiff {
    let local_set: HashSet<&str> = local
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let remote_set: HashSet<&str> = remote
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let added_rules: Vec<String> = remote_set
        .difference(&local_set)
        .map(|s| s.to_string())
        .collect();
    let removed_rules: Vec<String> = local_set
        .difference(&remote_set)
        .map(|s| s.to_string())
        .collect();
    let unchanged_count = local_set.intersection(&remote_set).count();
    RuleProviderDiff {
        provider_name: provider_name.to_string(),
        local_count: local_set.len(),
        remote_count: remote_set.len(),
        added_rules,
        removed_rules,
        unchanged_count,
    }
}

pub fn unpack_provider_rules_to_custom(rules: &[String], target: &str) -> Vec<RuleEntry> {
    rules
        .iter()
        .map(|r| r.trim())
        .filter(|r| !r.is_empty() && !r.starts_with('#'))
        .map(|r| {
            if r.contains(',') {
                let parts: Vec<&str> = r.split(',').collect();
                if parts.len() == 2 {
                    RuleEntry {
                        rule: format!("{},{},{}", parts[0], parts[1], target),
                        enabled: true,
                    }
                } else {
                    RuleEntry {
                        rule: r.to_string(),
                        enabled: true,
                    }
                }
            } else {
                RuleEntry {
                    rule: format!("DOMAIN-SUFFIX,{},{}", r, target),
                    enabled: true,
                }
            }
        })
        .collect()
}

pub fn game_routing_presets(target: &str) -> Vec<RuleEntry> {
    vec![
        RuleEntry {
            rule: format!("PROCESS-NAME,steam.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,steamwebhelper.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,EpicGamesLauncher.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,RiotClientServices.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,Battle.net.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,Origin.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("PROCESS-NAME,EADesktop.exe,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("DOMAIN-SUFFIX,steamcommunity.com,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("DOMAIN-SUFFIX,steampowered.com,{}", target),
            enabled: true,
        },
        RuleEntry {
            rule: format!("DOMAIN-SUFFIX,epicgames.com,{}", target),
            enabled: true,
        },
    ]
}

/// Apply rule-provider changes to an in-memory profile document.
pub fn apply_rule_providers_to_yaml(content: &str, providers: &RuleProviders) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_rule_providers(&mut doc, providers)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

/// Extract the rule list from an already-loaded profile document.
pub fn load_rules_from_yaml(content: &str) -> Result<Vec<RuleEntry>> {
    let doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    extract_rules_from_doc(&doc)
}

/// Apply a complete rule list to an in-memory profile document.
pub fn apply_rules_to_yaml(content: &str, rules: &[RuleEntry]) -> Result<String> {
    validate_rules(rules)?;
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_rules(&mut doc, rules)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

pub fn extract_rule_providers_from_doc(doc: &Value) -> Result<RuleProviders> {
    let value = doc
        .get("rule-providers")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow!("rule-providers is not a mapping"))?;
    let mut providers = BTreeMap::new();
    for (key, val) in mapping {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow!("rule-providers contains non-string key"))?;
        let json_value = serde_json::to_value(val).context("encode rule provider")?;
        providers.insert(name.to_string(), json_value);
    }
    Ok(providers)
}

fn apply_rule_providers(doc: &mut Value, providers: &RuleProviders) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if providers.is_empty() {
        map.remove(Value::String("rule-providers".to_string()));
        return Ok(());
    }

    let mut yaml_map = Mapping::new();
    for (name, value) in providers {
        let yaml_value = serde_yaml_ng::to_value(value).context("decode rule provider")?;
        yaml_map.insert(Value::String(name.to_string()), yaml_value);
    }
    map.insert(
        Value::String("rule-providers".to_string()),
        Value::Mapping(yaml_map),
    );
    Ok(())
}

pub fn extract_rules_from_doc(doc: &Value) -> Result<Vec<RuleEntry>> {
    let value = match doc.get("rules") {
        Some(v) => v.clone(),
        None => return Ok(Vec::new()),
    };
    let seq = value
        .as_sequence()
        .ok_or_else(|| anyhow!("rules is not a list"))?;
    let mut rules = Vec::with_capacity(seq.len());
    for item in seq {
        if let Some(rule) = item.as_str() {
            rules.push(parse_rule_entry(rule));
        }
    }
    Ok(rules)
}

fn apply_rules(doc: &mut Value, rules: &[RuleEntry]) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if rules.is_empty() {
        map.remove(Value::String("rules".to_string()));
        return Ok(());
    }
    let entries: Vec<Value> = rules
        .iter()
        .map(|entry| Value::String(format_rule_entry(entry)))
        .collect();
    map.insert(Value::String("rules".to_string()), Value::Sequence(entries));
    Ok(())
}

pub fn parse_rule_entry(value: &str) -> RuleEntry {
    let trimmed = value.trim_start();
    if let Some(rest) = trimmed.strip_prefix('#') {
        RuleEntry {
            rule: rest.trim_start().to_string(),
            enabled: false,
        }
    } else {
        RuleEntry {
            rule: trimmed.to_string(),
            enabled: true,
        }
    }
}

pub fn format_rule_entry(entry: &RuleEntry) -> String {
    let rule = entry.rule.trim();
    if entry.enabled {
        rule.to_string()
    } else {
        format!("# {rule}")
    }
}

pub fn validate_rules(rules: &[RuleEntry]) -> Result<()> {
    for entry in rules {
        if entry.rule.trim().is_empty() {
            return Err(anyhow!("rule entry is empty"));
        }
    }
    Ok(())
}
