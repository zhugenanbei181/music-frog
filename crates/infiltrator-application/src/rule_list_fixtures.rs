//! Explicit serialized source documents for isolated UI and editor behavior fixtures.
use crate::profile_workspace_test_support::workspace_with_raw_sidecar;
use crate::rule_list_application::document_snapshot;
use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_domain::rules::{RuleEntry, format_rule_entry};
use infiltrator_ports::rule_tracer::RuleWorkspace;

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
    let profile = "rules-fixture.yaml".to_owned();
    let workspace = workspace_with_raw_sidecar(profile.as_str(), content.as_bytes(), None, None)
        .expect("explicit rule source fixture identity");
    document_snapshot(&RuleWorkspace {
        source: identify_rules_document(profile, &content),
        profile_identity: workspace.identity(),
        rules,
        targets: vec!["DIRECT".into(), "REJECT".into()],
        sub_rules: Default::default(),
    })
}
