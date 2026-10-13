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
