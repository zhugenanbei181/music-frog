//! test-intent: behavior
use crate::rules_page_model::{RulesOperation, RulesPageModel};
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use lurker_core_contract::error::{ErrorCode, Failure};

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
