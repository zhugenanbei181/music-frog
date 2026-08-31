//! Rules management: the persisted custom rule list and rule provider
//! enables, exchanged with Kotlin as JSON strings.

use std::collections::BTreeMap;

use infiltrator_core::rules::{ load_rule_providers,
    load_rules, save_rule_providers, save_rules};


use super::support::{get_runtime, map_anyhow_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- Rules API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct RuleEntryRecord {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RulesResult {
    pub status: FfiStatus,
    pub rules: Vec<RuleEntryRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RuleProvidersResult {
    pub status: FfiStatus,
    pub json: String,
}

#[uniffi::export]
pub async fn rules_list() -> RulesResult {
    get_runtime()
        .spawn(async move {
            match load_rules().await.map_err(map_anyhow_error) {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(status) => RulesResult {
                    status,
                    rules: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RulesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            rules: Vec::new(),
        })
}

#[uniffi::export]
pub async fn rules_save(rules: Vec<RuleEntryRecord>) -> RulesResult {
    get_runtime()
        .spawn(async move {
            let core_rules: Vec<infiltrator_core::rules::RuleEntry> = rules.iter().map(record_to_core_rule).collect();
            match save_rules(core_rules).await.map_err(map_anyhow_error) {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(status) => RulesResult {
                    status,
                    rules: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RulesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            rules: Vec::new(),
        })
}

#[uniffi::export]
pub async fn rule_providers() -> RuleProvidersResult {
    get_runtime()
        .spawn(async move {
            match load_rule_providers().await.map_err(map_anyhow_error) {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(status) => RuleProvidersResult {
                    status,
                    json: "{}".to_string(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RuleProvidersResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            json: "{}".to_string(),
        })
}

#[uniffi::export]
pub async fn rule_providers_save(json: String) -> RuleProvidersResult {
    get_runtime()
        .spawn(async move {
            let providers = match parse_rule_providers_json(&json) {
                Ok(value) => value,
                Err(status) => {
                    return RuleProvidersResult {
                        status,
                        json: "{}".to_string(),
                    };
                }
            };
            match save_rule_providers(providers)
                .await
                .map_err(map_anyhow_error)
            {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(status) => RuleProvidersResult {
                    status,
                    json: "{}".to_string(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RuleProvidersResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            json: "{}".to_string(),
        })
}

fn core_rule_to_record(entry: infiltrator_core::rules::RuleEntry) -> RuleEntryRecord {
    RuleEntryRecord {
        rule: entry.rule,
        enabled: entry.enabled,
    }
}

fn record_to_core_rule(entry: &RuleEntryRecord) -> infiltrator_core::rules::RuleEntry {
    infiltrator_core::rules::RuleEntry {
        rule: entry.rule.trim().to_string(),
        enabled: entry.enabled,
    }
}

fn rule_providers_to_json(providers: &infiltrator_core::rules::RuleProviders) -> String {
    let value = serde_json::Value::Object(
        providers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn parse_rule_providers_json(value: &str) -> Result<infiltrator_core::rules::RuleProviders, FfiStatus> {
    let parsed: serde_json::Value = serde_json::from_str(value).map_err(|err| {
        FfiStatus::err(FfiErrorCode::InvalidInput, format!("invalid JSON: {err}"))
    })?;
    let object = parsed.as_object().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "rule providers JSON must be an object",
        )
    })?;
    let mut providers: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (key, value) in object {
        providers.insert(key.clone(), value.clone());
    }
    Ok(providers)
}
