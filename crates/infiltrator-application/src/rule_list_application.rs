//! Atomic source-bound root-list commits shared by both product surfaces.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_document::{RuleDocumentSnapshot, RuleListCommit};
use infiltrator_domain::rules::document::{definitions_from_entries, entries_from_definitions};
use infiltrator_domain::rules::types::{RuleType, parse_rule_str};
use infiltrator_domain::sub_rules::LogicalRuleAst;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct RuleListApplication {
    port: Arc<Mutex<RuleListPortBinding>>,
}

#[derive(Clone)]
enum RuleListPortBinding {
    Uncomposed,
    Composed(Arc<dyn RuleOverridePort>),
}

impl Default for RuleListApplication {
    fn default() -> Self {
        Self {
            port: Arc::new(Mutex::new(RuleListPortBinding::Uncomposed)),
        }
    }
}

impl RuleListApplication {
    pub fn new(port: Arc<dyn RuleOverridePort>) -> Self {
        Self {
            port: Arc::new(Mutex::new(RuleListPortBinding::Composed(port))),
        }
    }
    pub fn set_port(&self, port: Arc<dyn RuleOverridePort>) {
        *self.port.lock().expect("rule-list host slot") = RuleListPortBinding::Composed(port);
    }
    pub async fn commit(&self, request: &RuleListCommit) -> Result<(), Failure> {
        let port = match self.port.lock().expect("rule-list host slot").clone() {
            RuleListPortBinding::Uncomposed => {
                return Err(Failure::unsupported(
                    "Atomic rule-list persistence is not composed",
                ));
            }
            RuleListPortBinding::Composed(port) => port,
        };
        let workspace = port.load_rule_workspace().await.map_err(Failure::from)?;
        if workspace.source != request.expected_source {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The rule source document changed; retain or discard the draft before editing the current document",
                true,
            ));
        }
        let rules = entries_from_definitions(&request.rules);
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
pub fn document_snapshot(workspace: &RuleWorkspace) -> RuleDocumentSnapshot {
    RuleDocumentSnapshot {
        source: workspace.source.clone(),
        rules: definitions_from_entries(&workspace.rules),
        sub_rules: workspace
            .sub_rules
            .iter()
            .map(|(name, rules)| (name.clone(), definitions_from_entries(rules)))
            .collect(),
        targets: workspace.targets.clone(),
    }
}

#[cfg(test)]
#[path = "rule_list_application_test.rs"]
mod tests;
