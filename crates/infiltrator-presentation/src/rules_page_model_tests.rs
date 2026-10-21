//! test-intent: behavior
use crate::rules_page_model::{RulesOperation, RulesPageModel};
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_domain::rules::RuleEntry;
use lurker_core_contract::error::{ErrorCode, Failure};

fn rule(expression: &str) -> RuleEntry {
    RuleEntry {
        rule: expression.to_owned(),
        enabled: true,
    }
}

#[test]
fn the_tab_selection_is_one_shared_value() {
    let mut model = RulesPageModel::default();
    assert_eq!(model.tab(), RulesTab::List);
    assert_eq!(model.tab_index(), 0);

    model.select_tab(RulesTab::JsonEditors);
    assert_eq!(model.tab(), RulesTab::JsonEditors);
    assert_eq!(model.tab_index(), 2);

    model.select_tab_at(3);
    assert_eq!(model.tab(), RulesTab::Tracer);

    // A position beyond the strip is the list partition, never a new one.
    model.select_tab_at(99);
    assert_eq!(model.tab(), RulesTab::List);
}

#[test]
fn every_operation_has_its_own_phase_and_one_operation_never_moves_another() {
    let mut model = RulesPageModel::default();
    assert!(!model.is_busy());
    assert!(model.running_operations().is_empty());

    let save = model.operation_mut(RulesOperation::Save).begin();
    assert!(model.is_busy());
    assert_eq!(model.running_operations(), vec![RulesOperation::Save]);

    // The load phase is untouched by the save phase.
    assert!(!model.operation(RulesOperation::Load).is_running());

    let failure = Failure::new(ErrorCode::Configuration, "rules rejected", false);
    assert!(
        model
            .operation_mut(RulesOperation::Save)
            .fail(save, failure)
    );
    assert!(!model.is_busy());
    assert_eq!(
        model
            .operation(RulesOperation::Save)
            .failure()
            .map(|failure| failure.message.as_str()),
        Some("rules rejected")
    );
    assert!(model.operation(RulesOperation::Load).failure().is_none());
}

#[test]
fn the_operation_list_is_one_shared_inventory() {
    assert_eq!(RulesOperation::ALL.len(), 8);
    for (index, operation) in RulesOperation::ALL.iter().enumerate() {
        assert_eq!(operation.index(), index);
    }
    assert_eq!(
        RulesOperation::SaveSnifferJson.json_section(),
        Some(RulesJsonSection::Sniffer)
    );
    assert_eq!(
        RulesOperation::SaveRuleProvidersJson.json_section(),
        Some(RulesJsonSection::RuleProviders)
    );
    assert!(RulesOperation::Save.json_section().is_none());
}

#[test]
fn a_json_buffer_carries_its_own_baseline_instead_of_a_dirty_flag() {
    let mut model = RulesPageModel::default();
    model.observe_json_document(RulesJsonSection::RuleProviders, "{}".to_owned());
    assert_eq!(model.json_text(RulesJsonSection::RuleProviders), "{}");
    assert!(!model.json_modified(RulesJsonSection::RuleProviders));

    model
        .json_buffer_mut(RulesJsonSection::RuleProviders)
        .edit("{\"a\":1}".to_owned());
    assert!(model.json_modified(RulesJsonSection::RuleProviders));
    // The other buffers are untouched.
    assert!(!model.json_modified(RulesJsonSection::ProxyProviders));
    assert!(!model.json_modified(RulesJsonSection::Sniffer));
}

#[test]
fn a_published_document_does_not_clobber_an_unaccepted_edit() {
    let mut model = RulesPageModel::default();
    model.observe_json_document(RulesJsonSection::Sniffer, "{}".to_owned());
    model
        .json_buffer_mut(RulesJsonSection::Sniffer)
        .edit("{\"sniff\":true}".to_owned());

    model.observe_json_document(RulesJsonSection::Sniffer, "{}".to_owned());
    assert!(model.json_modified(RulesJsonSection::Sniffer));
    assert_eq!(
        model.json_text(RulesJsonSection::Sniffer),
        "{\"sniff\":true}"
    );

    // The owner published the edit: the buffer is the committed value again.
    model.observe_json_document(RulesJsonSection::Sniffer, "{\"sniff\":true}".to_owned());
    assert!(!model.json_modified(RulesJsonSection::Sniffer));
}

#[test]
fn the_json_section_selection_is_one_shared_value() {
    let mut model = RulesPageModel::default();
    assert_eq!(model.json_section(), RulesJsonSection::RuleProviders);

    model.select_json_section(RulesJsonSection::ProxyProviders);
    assert_eq!(model.json_section(), RulesJsonSection::ProxyProviders);
}

#[test]
fn the_rule_list_filter_is_owned_by_the_shared_model() {
    let mut model = RulesPageModel::default();
    let rules = vec![
        rule("DOMAIN-SUFFIX,github.com,PROXY"),
        rule("GEOIP,CN,DIRECT"),
        rule("DOMAIN-KEYWORD,google,GLOBAL"),
    ];

    // An empty filter keeps every rule.
    assert_eq!(model.filter_text(), "");
    assert_eq!(model.filtered_indices(&rules), vec![0, 1, 2]);
    assert!(model.matches_rule(&rules[0]));

    model.edit_filter("github");
    assert_eq!(model.filter_text(), "github");
    assert_eq!(model.filtered_indices(&rules), vec![0]);
    assert!(model.matches_rule(&rules[0]));
    assert!(!model.matches_rule(&rules[1]));

    // The match rule is the shared domain reduction, so the target and the
    // payload both answer the same query.
    model.edit_filter("DIRECT");
    assert_eq!(model.filtered_indices(&rules), vec![1]);

    model.edit_filter("  ");
    assert_eq!(model.filter_text(), "");
    assert_eq!(model.filtered_indices(&rules), vec![0, 1, 2]);
}

#[test]
fn a_latency_bound_is_free_text_on_the_rule_list() {
    let mut model = RulesPageModel::default();
    let rules = vec![rule("DOMAIN-SUFFIX,<100,PROXY")];

    // A rule carries no latency, so the bound never empties the list.
    model.edit_filter("<100");
    assert_eq!(model.filter_text(), "<100");
    assert_eq!(model.filtered_indices(&rules), vec![0]);
    assert!(model.matches_rule(&rules[0]));
}
