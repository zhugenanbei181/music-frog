use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::document::definitions_from_entries;

pub fn list_document(entries: Vec<RuleEntry>) -> RuleDocumentSnapshot {
    let rules = definitions_from_entries(&entries);
    let document_hash = serde_json::to_string(&rules).expect("rule fixture serializes");
    RuleDocumentSnapshot {
        source: RuleSourceIdentity {
            profile: "rules-fixture.yaml".into(),
            document_hash,
        },
        rules,
        sub_rules: Default::default(),
        targets: vec!["DIRECT".into(), "REJECT".into()],
    }
}
