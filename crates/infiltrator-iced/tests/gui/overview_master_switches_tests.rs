use super::*;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::Failure;

#[test]
fn unsupported_master_control_is_not_offered_as_an_action() {
    let snapshot = SystemToggleState::Unsupported {
        failure: Failure::unsupported("host missing"),
    };
    let lang = Lang("en-US");
    assert_eq!(
        status_label(&snapshot, &Lang(lang.0)),
        "Unavailable · host missing"
    );
    assert_eq!(action_label(&snapshot, &Lang(lang.0)), "Unavailable");
    assert!(!snapshot.can_toggle());
}
