//! One document fingerprint used by reader and host compare-and-apply boundaries.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_domain::rules::{load_rules_from_yaml, parse_rule_entry};
use infiltrator_ports::rule_tracer::RuleWorkspace;
use serde_yaml_ng::Value;
use std::collections::BTreeMap;
pub fn rule_workspace(profile: String, content: &str) -> Result<RuleWorkspace, Failure> {
    let doc: Value = serde_yaml_ng::from_str(content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    let rules = load_rules_from_yaml(content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    let mut targets = vec!["DIRECT".to_string(), "REJECT".to_string()];
    for section in ["proxies", "proxy-groups"] {
        for entry in doc
            .get(section)
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                targets.push(name.to_string());
            }
        }
    }
    let mut sub_rules = BTreeMap::new();
    if let Some(tables) = doc.get("sub-rules") {
        let tables = tables.as_mapping().ok_or_else(|| {
            Failure::new(
                ErrorCode::Configuration,
                "sub-rules must be a mapping",
                false,
            )
        })?;
        for (name, entries) in tables {
            let name = name
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::Configuration,
                        "sub-rule table needs a nonempty string name",
                        false,
                    )
                })?;
            let entries = entries.as_sequence().ok_or_else(|| {
                Failure::new(
                    ErrorCode::Configuration,
                    format!("sub-rule table {name} must be a sequence"),
                    false,
                )
            })?;
            let rules = entries
                .iter()
                .map(|entry| {
                    entry.as_str().map(parse_rule_entry).ok_or_else(|| {
                        Failure::new(
                            ErrorCode::Configuration,
                            format!("sub-rule table {name} contains a non-string rule"),
                            false,
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            sub_rules.insert(name.to_owned(), rules);
        }
    }
    Ok(RuleWorkspace {
        source: identify_rules_document(profile, content),
        rules,
        targets,
        sub_rules,
    })
}
