//! Lossless value mapping between contract rule definitions and domain entries.

use super::RuleEntry;
use infiltrator_contract::rule_document::RuleDefinition;

pub fn entries_from_definitions(definitions: &[RuleDefinition]) -> Vec<RuleEntry> {
    definitions
        .iter()
        .map(|definition| RuleEntry {
            rule: definition.rule.clone(),
            enabled: definition.enabled,
        })
        .collect()
}

pub fn definitions_from_entries(entries: &[RuleEntry]) -> Vec<RuleDefinition> {
    entries
        .iter()
        .map(|entry| RuleDefinition {
            rule: entry.rule.clone(),
            enabled: entry.enabled,
        })
        .collect()
}
