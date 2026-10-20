//! test-intent: behavior
use crate::editable_draft::EditableDraft;

#[test]
fn a_committed_draft_is_not_a_modification() {
    let draft = EditableDraft::committed(7_u32);
    assert!(!draft.is_modified());
    assert_eq!(draft.value(), &7);
    assert_eq!(draft.baseline(), Some(&7));
}

#[test]
fn an_edit_that_differs_from_the_baseline_is_a_modification() {
    let mut draft = EditableDraft::committed(7_u32);
    draft.edit(9);

    assert!(draft.is_modified());
    assert_eq!(draft.value(), &9);
    assert_eq!(draft.baseline(), Some(&7));
}

#[test]
fn an_edit_back_to_the_baseline_clears_the_modification() {
    let mut draft = EditableDraft::committed(7_u32);
    draft.edit(9);
    assert!(draft.is_modified());

    draft.edit(7);
    assert!(!draft.is_modified());
    assert_eq!(draft.value(), &7);
}

#[test]
fn committing_accepts_the_draft_and_discarding_drops_it() {
    let mut draft = EditableDraft::committed(7_u32);
    draft.edit(9);
    assert_eq!(draft.commit(), 9);
    assert!(!draft.is_modified());
    assert_eq!(draft.baseline(), Some(&9));

    draft.edit(11);
    assert!(draft.is_modified());
    draft.discard();
    assert!(!draft.is_modified());
    assert_eq!(draft.value(), &9);
}

#[test]
fn an_in_place_edit_never_touches_the_baseline() {
    let mut draft = EditableDraft::committed(vec![1_u8, 2]);
    draft.edit_with(|value| value.push(3));

    assert_eq!(draft.value(), &vec![1, 2, 3]);
    // The baseline is untouched: it is still the committed owner value.
    assert_eq!(draft.baseline(), Some(&vec![1, 2]));
    assert!(draft.is_modified());
}

#[test]
fn an_undrafted_draft_adopts_the_owner_value() {
    let mut draft = EditableDraft::committed(7_u32);
    draft.adopt(9);

    assert!(!draft.is_modified());
    assert_eq!(draft.value(), &9);
    assert_eq!(draft.baseline(), Some(&9));
}

#[test]
fn a_pending_edit_survives_an_owner_readback_and_dies_when_it_is_accepted() {
    let mut draft = EditableDraft::committed(7_u32);
    draft.edit(9);
    assert!(draft.is_modified());

    // A readback that still carries the old value keeps the edit.
    draft.adopt(7);
    assert!(draft.is_modified());
    assert_eq!(draft.value(), &9);
    assert_eq!(draft.baseline(), Some(&7));

    // A readback that carries the edit clears the modification.
    draft.adopt(9);
    assert!(!draft.is_modified());
    assert_eq!(draft.value(), &9);
}
