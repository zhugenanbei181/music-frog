//! Atomic source-bound root-list commits shared by both product surfaces.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_document::{RuleDefinition, RuleDocumentSnapshot, RuleListCommit};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::types::{RuleType, parse_rule_str};
use infiltrator_domain::sub_rules::LogicalRuleAst;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct RuleListApplication {
    port: Arc<Mutex<Option<Arc<dyn RuleOverridePort>>>>,
}
impl RuleListApplication {
    pub fn new(port: Arc<dyn RuleOverridePort>) -> Self {
        Self {
            port: Arc::new(Mutex::new(Some(port))),
        }
    }
    pub fn set_port(&self, port: Arc<dyn RuleOverridePort>) {
        *self.port.lock().expect("rule-list host slot") = Some(port);
    }
    pub async fn commit(&self, request: &RuleListCommit) -> Result<(), Failure> {
        let port = self
            .port
            .lock()
            .expect("rule-list host slot")
            .clone()
            .ok_or_else(|| Failure::unsupported("Atomic rule-list persistence is not composed"))?;
        let workspace = port.load_rule_workspace().await.map_err(Failure::from)?;
        if workspace.source != request.expected_source {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The rule source document changed; retain or discard the draft before editing the current document",
                true,
            ));
        }
        let rules = domain_rules(&request.rules);
        for entry in &rules {
            if !entry.enabled {
                continue;
            }
            let parsed = parse_rule_str(&entry.rule).map_err(|error| {
                Failure::new(ErrorCode::Configuration, error.to_string(), false)
            })?;
            let named = matches!(&parsed.rule_type, RuleType::Logical(logical) if matches!(logical.payload, LogicalRuleAst::SubRule(_)));
            let valid_target = if named {
                workspace.sub_rules.contains_key(&parsed.target)
            } else {
                workspace.targets.contains(&parsed.target)
            };
            if !valid_target {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("Unknown rule target: {}", parsed.target),
                    false,
                ));
            }
        }
        port.compare_and_apply_rules(&workspace, &rules)
            .await
            .map_err(Failure::from)
    }
}
pub fn domain_rules(rules: &[RuleDefinition]) -> Vec<RuleEntry> {
    rules
        .iter()
        .map(|rule| RuleEntry {
            rule: rule.rule.clone(),
            enabled: rule.enabled,
        })
        .collect()
}
pub fn rule_definitions(rules: &[RuleEntry]) -> Vec<RuleDefinition> {
    rules
        .iter()
        .map(|rule| RuleDefinition {
            rule: rule.rule.clone(),
            enabled: rule.enabled,
        })
        .collect()
}
pub fn document_snapshot(workspace: &RuleWorkspace) -> RuleDocumentSnapshot {
    RuleDocumentSnapshot {
        source: workspace.source.clone(),
        rules: rule_definitions(&workspace.rules),
        sub_rules: workspace
            .sub_rules
            .iter()
            .map(|(name, rules)| (name.clone(), rule_definitions(rules)))
            .collect(),
        targets: workspace.targets.clone(),
    }
}

#[cfg(test)]
#[path = "rule_list_application_test.rs"]
mod tests;
