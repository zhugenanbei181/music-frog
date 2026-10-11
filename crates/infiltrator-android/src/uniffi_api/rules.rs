//! Owner-local Rules/Profile document application operations and typed Rules
//! DTOs. Android UI requests use the shared AndroidOwnerOperation enum.

use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_configuration_application, map_application_failure};
use infiltrator_domain::rules::{RuleEntry, RuleProviders};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidRuleEntry {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidRulesResult {
    pub status: FfiStatus,
    pub rules: Vec<AndroidRuleEntry>,
}

/// A lossless, recursive JSON value exposed as typed UniFFI variants. The
/// provider editor may retain arbitrary provider fields without sending a
/// free-form JSON command or provider blob across IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum AndroidJsonValue {
    Null,
    BoolValue { value: bool },
    NumberValue { value: String },
    StringValue { value: String },
    ArrayValue { value: Vec<AndroidJsonValue> },
    ObjectValue { value: Vec<AndroidJsonEntry> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidJsonEntry {
    pub key: String,
    pub value: AndroidJsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidRuleProviderEntry {
    pub name: String,
    pub configuration: AndroidJsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidRuleProvidersResult {
    pub status: FfiStatus,
    pub providers: Vec<AndroidRuleProviderEntry>,
}

const MAX_PROVIDER_JSON_DEPTH: usize = 128;

pub(super) fn rule_provider_entries_to_map(
    entries: Vec<AndroidRuleProviderEntry>,
) -> Result<RuleProviders, FfiStatus> {
    let mut providers = BTreeMap::new();
    for entry in entries {
        let value = android_json_to_value(entry.configuration, 0)?;
        if providers.insert(entry.name.clone(), value).is_some() {
            return Err(FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("duplicate rule provider name: {}", entry.name),
            ));
        }
    }
    Ok(providers)
}

pub(super) fn rule_provider_map_to_entries(
    providers: RuleProviders,
) -> Vec<AndroidRuleProviderEntry> {
    providers
        .into_iter()
        .map(|(name, configuration)| AndroidRuleProviderEntry {
            name,
            configuration: value_to_android_json(configuration),
        })
        .collect()
}

fn android_json_to_value(
    value: AndroidJsonValue,
    depth: usize,
) -> Result<serde_json::Value, FfiStatus> {
    if depth > MAX_PROVIDER_JSON_DEPTH {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "rule provider JSON nesting exceeds the supported depth",
        ));
    }
    match value {
        AndroidJsonValue::Null => Ok(serde_json::Value::Null),
        AndroidJsonValue::BoolValue { value } => Ok(serde_json::Value::Bool(value)),
        AndroidJsonValue::NumberValue { value } => value
            .parse::<serde_json::Number>()
            .map(serde_json::Value::Number)
            .map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    format!("invalid rule provider number: {error}"),
                )
            }),
        AndroidJsonValue::StringValue { value } => Ok(serde_json::Value::String(value)),
        AndroidJsonValue::ArrayValue { value } => value
            .into_iter()
            .map(|item| android_json_to_value(item, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        AndroidJsonValue::ObjectValue { value } => {
            let mut object = serde_json::Map::new();
            for entry in value {
                let child = android_json_to_value(entry.value, depth + 1)?;
                if object.insert(entry.key.clone(), child).is_some() {
                    return Err(FfiStatus::err(
                        FfiErrorCode::InvalidInput,
                        format!("duplicate rule provider field: {}", entry.key),
                    ));
                }
            }
            Ok(serde_json::Value::Object(object))
        }
    }
}

fn value_to_android_json(value: serde_json::Value) -> AndroidJsonValue {
    match value {
        serde_json::Value::Null => AndroidJsonValue::Null,
        serde_json::Value::Bool(value) => AndroidJsonValue::BoolValue { value },
        serde_json::Value::Number(value) => AndroidJsonValue::NumberValue {
            value: value.to_string(),
        },
        serde_json::Value::String(value) => AndroidJsonValue::StringValue { value },
        serde_json::Value::Array(value) => AndroidJsonValue::ArrayValue {
            value: value.into_iter().map(value_to_android_json).collect(),
        },
        serde_json::Value::Object(value) => AndroidJsonValue::ObjectValue {
            value: value
                .into_iter()
                .map(|(key, value)| AndroidJsonEntry {
                    key,
                    value: value_to_android_json(value),
                })
                .collect(),
        },
    }
}

pub(super) async fn load_rules_local() -> AndroidRulesResult {
    let application = match build_configuration_application().await {
        Ok(application) => application,
        Err(status) => {
            return AndroidRulesResult {
                status,
                rules: Vec::new(),
            };
        }
    };
    match application.load_rules().await {
        Ok(rules) => AndroidRulesResult {
            status: FfiStatus::ok(),
            rules: rules.into_iter().map(core_rule_to_record).collect(),
        },
        Err(failure) => AndroidRulesResult {
            status: map_application_failure(failure),
            rules: Vec::new(),
        },
    }
}

pub(super) async fn save_rules_local(rules: Vec<AndroidRuleEntry>) -> AndroidRulesResult {
    let core_rules: Vec<RuleEntry> = rules.iter().map(record_to_core_rule).collect();
    let application = match build_configuration_application().await {
        Ok(application) => application,
        Err(status) => {
            return AndroidRulesResult {
                status,
                rules: Vec::new(),
            };
        }
    };
    match application.save_rules(core_rules).await {
        Ok(rules) => AndroidRulesResult {
            status: FfiStatus::ok(),
            rules: rules.into_iter().map(core_rule_to_record).collect(),
        },
        Err(failure) => AndroidRulesResult {
            status: map_application_failure(failure),
            rules: Vec::new(),
        },
    }
}

pub(super) async fn load_rule_providers_local() -> AndroidRuleProvidersResult {
    let application = match build_configuration_application().await {
        Ok(application) => application,
        Err(status) => {
            return AndroidRuleProvidersResult {
                status,
                providers: Vec::new(),
            };
        }
    };
    match application.load_rule_providers().await {
        Ok(providers) => AndroidRuleProvidersResult {
            status: FfiStatus::ok(),
            providers: rule_provider_map_to_entries(providers),
        },
        Err(failure) => AndroidRuleProvidersResult {
            status: map_application_failure(failure),
            providers: Vec::new(),
        },
    }
}

pub(super) async fn save_rule_providers_local(
    entries: Vec<AndroidRuleProviderEntry>,
) -> AndroidRuleProvidersResult {
    let providers = match rule_provider_entries_to_map(entries) {
        Ok(providers) => providers,
        Err(status) => {
            return AndroidRuleProvidersResult {
                status,
                providers: Vec::new(),
            };
        }
    };
    let application = match build_configuration_application().await {
        Ok(application) => application,
        Err(status) => {
            return AndroidRuleProvidersResult {
                status,
                providers: Vec::new(),
            };
        }
    };
    match application.save_rule_providers(providers).await {
        Ok(providers) => AndroidRuleProvidersResult {
            status: FfiStatus::ok(),
            providers: rule_provider_map_to_entries(providers),
        },
        Err(failure) => AndroidRuleProvidersResult {
            status: map_application_failure(failure),
            providers: Vec::new(),
        },
    }
}

fn core_rule_to_record(entry: RuleEntry) -> AndroidRuleEntry {
    AndroidRuleEntry {
        rule: entry.rule,
        enabled: entry.enabled,
    }
}

fn record_to_core_rule(entry: &AndroidRuleEntry) -> RuleEntry {
    RuleEntry {
        rule: entry.rule.trim().to_string(),
        enabled: entry.enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_json_tree_roundtrips_nested_typed_values() {
        let original = serde_json::json!({
            "type": "http",
            "enabled": true,
            "interval": 86400,
            "metadata": {"tags": ["a", "b"], "nullable": null}
        });
        let typed = value_to_android_json(original.clone());
        assert_eq!(
            android_json_to_value(typed, 0).expect("valid JSON tree"),
            original
        );
    }

    #[test]
    fn provider_json_tree_rejects_duplicate_fields_and_invalid_numbers() {
        let duplicate = AndroidJsonValue::ObjectValue {
            value: vec![
                AndroidJsonEntry {
                    key: "type".to_owned(),
                    value: AndroidJsonValue::StringValue {
                        value: "http".to_owned(),
                    },
                },
                AndroidJsonEntry {
                    key: "type".to_owned(),
                    value: AndroidJsonValue::StringValue {
                        value: "file".to_owned(),
                    },
                },
            ],
        };
        assert_eq!(
            android_json_to_value(duplicate, 0)
                .expect_err("duplicate typed object keys are ambiguous")
                .code,
            FfiErrorCode::InvalidInput,
        );
        assert_eq!(
            android_json_to_value(
                AndroidJsonValue::NumberValue {
                    value: "not-a-number".to_owned(),
                },
                0,
            )
            .expect_err("numbers are parsed before config mutation")
            .code,
            FfiErrorCode::InvalidInput,
        );
    }
}
