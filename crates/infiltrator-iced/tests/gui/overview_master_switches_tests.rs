use super::*;

#[test]
fn unsupported_master_control_is_not_offered_as_an_action() {
    let snapshot = SystemToggleState::Unsupported {
        failure: infiltrator_contract::error::Failure::unsupported("host missing"),
    };
    let lang = Lang("en-US");
    assert_eq!(status_label(&snapshot, &lang), "Unavailable");
    assert_eq!(action_label(&snapshot, &lang), "Unavailable");
    assert!(!snapshot.can_toggle());
}
