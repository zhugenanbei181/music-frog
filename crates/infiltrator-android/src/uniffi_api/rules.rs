//! Rules management: the persisted custom rule list and rule provider
//! enables, exchanged with Kotlin as JSON strings.

use std::collections::BTreeMap;

use infiltrator_domain::rules::{RuleEntry as DomainRuleEntry, RuleProviders};

use crate::host_support::{
    build_configuration_application, get_runtime, map_application_failure,
};
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
            let application = match build_configuration_application().await {
                Ok(application) => application,
                Err(status) => {
                    return RulesResult {
                        status,
                        rules: Vec::new(),
                    };
                }
            };
            match application.load_rules().await {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(failure) => RulesResult {
                    status: map_application_failure(failure),
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
            let core_rules: Vec<DomainRuleEntry> =
                rules.iter().map(record_to_core_rule).collect();
            let application = match build_configuration_application().await {
                Ok(application) => application,
                Err(status) => {
                    return RulesResult {
                        status,
                        rules: Vec::new(),
                    };
                }
            };
            match application.save_rules(core_rules).await {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(failure) => RulesResult {
                    status: map_application_failure(failure),
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
            let application = match build_configuration_application().await {
                Ok(application) => application,
                Err(status) => {
                    return RuleProvidersResult {
                        status,
                        json: "{}".to_string(),
                    };
                }
            };
            match application.load_rule_providers().await {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(failure) => RuleProvidersResult {
                    status: map_application_failure(failure),
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
            let application = match build_configuration_application().await {
                Ok(application) => application,
                Err(status) => {
                    return RuleProvidersResult {
                        status,
                        json: "{}".to_string(),
                    };
                }
            };
            match application.save_rule_providers(providers).await {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(failure) => RuleProvidersResult {
                    status: map_application_failure(failure),
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

fn core_rule_to_record(entry: DomainRuleEntry) -> RuleEntryRecord {
    RuleEntryRecord {
        rule: entry.rule,
        enabled: entry.enabled,
    }
}

fn record_to_core_rule(entry: &RuleEntryRecord) -> DomainRuleEntry {
    DomainRuleEntry {
        rule: entry.rule.trim().to_string(),
        enabled: entry.enabled,
    }
}

fn rule_providers_to_json(providers: &RuleProviders) -> String {
    let value = serde_json::Value::Object(
        providers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn parse_rule_providers_json(
    value: &str,
) -> Result<RuleProviders, FfiStatus> {
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
