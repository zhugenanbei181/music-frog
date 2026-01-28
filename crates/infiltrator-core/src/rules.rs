use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use mihomo_config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

pub type RuleProviders = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProvidersPayload {
    pub providers: RuleProviders,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesPayload {
    pub rules: Vec<RuleEntry>,
}

pub async fn load_rule_providers() -> Result<RuleProviders> {
    let doc = load_profile_doc().await?;
    extract_rule_providers(&doc)
}

pub async fn save_rule_providers(providers: RuleProviders) -> Result<RuleProviders> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let mut doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;

    apply_rule_providers(&mut doc, &providers)?;

    let updated = serde_yaml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(providers)
}

pub async fn load_rules() -> Result<Vec<RuleEntry>> {
    let doc = load_profile_doc().await?;
    extract_rules(&doc)
}

pub async fn save_rules(rules: Vec<RuleEntry>) -> Result<Vec<RuleEntry>> {
    validate_rules(&rules)?;
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let mut doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;

    apply_rules(&mut doc, &rules)?;

    let updated = serde_yaml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(rules)
}

async fn load_profile_doc() -> Result<Value> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    serde_yaml::from_str(&content).context("parse profile yaml")
}

fn extract_rule_providers(doc: &Value) -> Result<RuleProviders> {
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
        let yaml_value = serde_yaml::to_value(value).context("decode rule provider")?;
        yaml_map.insert(Value::String(name.to_string()), yaml_value);
    }
    map.insert(
        Value::String("rule-providers".to_string()),
        Value::Mapping(yaml_map),
    );
    Ok(())
}

fn extract_rules(doc: &Value) -> Result<Vec<RuleEntry>> {
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
    map.insert(
        Value::String("rules".to_string()),
        Value::Sequence(entries),
    );
    Ok(())
}

fn parse_rule_entry(value: &str) -> RuleEntry {
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

fn format_rule_entry(entry: &RuleEntry) -> String {
    let rule = entry.rule.trim();
    if entry.enabled {
        rule.to_string()
    } else {
        format!("# {rule}")
    }
}

fn validate_rules(rules: &[RuleEntry]) -> Result<()> {
    for entry in rules {
        if entry.rule.trim().is_empty() {
            return Err(anyhow!("rule entry is empty"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rule_entry() {
        let entry = parse_rule_entry("DOMAIN,example.com");
        assert!(entry.enabled);
        let entry = parse_rule_entry("# DIRECT");
        assert!(!entry.enabled);
        assert_eq!(entry.rule, "DIRECT");
    }

    #[test]
    fn test_format_rule_entry() {
        let entry = RuleEntry {
            rule: "DIRECT".to_string(),
            enabled: false,
        };
        assert_eq!(format_rule_entry(&entry), "# DIRECT");
    }

    #[test]
    fn test_apply_rule_providers_empty_removes() {
        let mut doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let providers = RuleProviders::new();
        apply_rule_providers(&mut doc, &providers).expect("apply providers");
        let map = doc.as_mapping().expect("mapping");
        assert!(map
            .get(Value::String("rule-providers".to_string()))
            .is_none());
    }

    #[test]
    fn test_apply_rules_empty_removes() {
        let mut doc: Value = serde_yaml::from_str("rules:\n  - DIRECT\n").expect("yaml");
        apply_rules(&mut doc, &[]).expect("apply rules");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(Value::String("rules".to_string())).is_none());
    }
}
