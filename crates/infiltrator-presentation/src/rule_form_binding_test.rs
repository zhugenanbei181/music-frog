//! test-intent: behavior
//! An edited unbound form cannot silently adopt a later source readback.

use super::*;
use crate::rule_fixtures::list_document;
use crate::rule_list_editor::RuleListReadback;
use infiltrator_domain::rules::RuleEntry;

#[test]
fn edited_unbound_form_requires_explicit_reset_before_binding_to_a_source() {
    let mut editor = RuleListEditor::default();
    let mut form = RuleFormBinding::default();

    form.edit(&editor);
    assert_eq!(form, RuleFormBinding::EditedWithoutSource);

    let document = list_document(vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }]);
    editor.observe_readback(RuleListReadback::Observed(document));
    assert!(!form.observe(&editor));
    assert_eq!(form, RuleFormBinding::EditedWithoutSource);
    assert!(!form.current(&editor));

    form.reset(&editor);
    assert!(form.current(&editor));
    assert!(matches!(form, RuleFormBinding::Clean { .. }));
}

#[test]
fn form_targets_publish_only_the_observed_document_catalogue() {
    let mut editor = RuleListEditor::default();
    assert!(form_targets(&editor).is_empty());

    let mut document = list_document(vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }]);
    document.targets = vec![
        "PROXY-GROUP-B".into(),
        "DIRECT".into(),
        "  ".into(),
        "PROXY-GROUP-B".into(),
        "REJECT".into(),
    ];
    editor.observe_readback(RuleListReadback::Observed(document));

    assert_eq!(
        form_targets(&editor),
        vec!["PROXY-GROUP-B", "DIRECT", "REJECT"]
    );
    assert_eq!(default_form_target(&editor), "DIRECT");
}

#[test]
fn default_form_target_keeps_the_owner_precedence() {
    let mut editor = RuleListEditor::default();
    let mut document = list_document(Vec::new());
    document.targets = vec!["REJECT".into(), "PROXY".into(), "DIRECT".into()];
    editor.observe_readback(RuleListReadback::Observed(document));
    assert_eq!(default_form_target(&editor), "PROXY");
}
