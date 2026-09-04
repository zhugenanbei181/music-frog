//! Tokio/filesystem adapter for rules and rule-provider documents.
//!
//! Rule models, parsing, validation and in-memory YAML transforms live in
//! `infiltrator-domain::rules`. This module keeps only the current-profile
//! lookup and persistence side effects used by legacy inbound adapters while
//! the application command surface is migrated.

use anyhow::Context;
use infiltrator_domain::rules::{
    RuleEntry, RuleProviders, apply_rule_providers_to_yaml, apply_rules_to_yaml,
    extract_rule_providers_from_doc, extract_rules_from_doc, validate_rules,
};
use serde_yaml_ng::Value;

async fn load_profile_doc() -> anyhow::Result<Value> {
    let manager = crate::settings::app_config_manager()
        .await
        .context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    serde_yaml_ng::from_str(&content).context("parse profile yaml")
}

async fn load_current_profile() -> anyhow::Result<(
    mihomo_config::manager::ConfigManager<mihomo_platform::defaults::DefaultCredentialStore>,
    String,
    String,
)> {
    let manager = crate::settings::app_config_manager()
        .await
        .context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    Ok((manager, profile, content))
}

pub async fn load_rule_providers() -> anyhow::Result<RuleProviders> {
    let doc = load_profile_doc().await?;
    extract_rule_providers_from_doc(&doc)
}

pub async fn save_rule_providers(providers: RuleProviders) -> anyhow::Result<RuleProviders> {
    let (manager, profile, content) = load_current_profile().await?;
    let updated = apply_rule_providers_to_yaml(&content, &providers)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(providers)
}

pub async fn load_rules() -> anyhow::Result<Vec<RuleEntry>> {
    let doc = load_profile_doc().await?;
    extract_rules_from_doc(&doc)
}

pub async fn save_rules(rules: Vec<RuleEntry>) -> anyhow::Result<Vec<RuleEntry>> {
    validate_rules(&rules)?;
    let (manager, profile, content) = load_current_profile().await?;
    let updated = apply_rules_to_yaml(&content, &rules)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(rules)
}

#[cfg(test)]
#[path = "rules_test.rs"]
mod rules_test;
