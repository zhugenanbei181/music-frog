//! Explicit serialized source documents for isolated UI and editor behavior fixtures.
use crate::rule_list_application::document_snapshot;
use crate::rule_source_identity::rule_workspace;
use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_domain::rules::{RuleEntry, format_rule_entry};
pub fn list_document(rules: Vec<RuleEntry>) -> RuleDocumentSnapshot {
    let entries = rules.iter().map(format_rule_entry).collect::<Vec<_>>();
    let mut content = if entries.is_empty() {
        String::from("rules: []\n")
    } else {
        String::from("rules:\n")
    };
    for entry in entries {
        content.push_str("  - ");
        content.push_str(&serde_json::to_string(&entry).expect("fixture scalar"));
        content.push('\n');
    }
    document_snapshot(
        &rule_workspace("rules-fixture.yaml".into(), &content)
            .expect("explicit rule source fixture"),
    )
}
