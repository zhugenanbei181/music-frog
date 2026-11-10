use super::*;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::Failure;
use infiltrator_contract::system_toggle::{SystemToggleSnapshot, SystemToggleState};

/// The card the user clicks and the owner that answers share one no-reentry
/// rule: while a request is in flight a second click cannot enqueue a
/// conflicting one. The rule used to be pinned from the sidebar's own toggle
/// cards; those cards are gone, so the test lives with the switches now.
#[test]
fn a_second_toggle_click_never_re_enters_the_pending_request() {
    let (mut state, _) = AppState::new();
    state.runtime.system_toggles = SystemToggleSnapshot::from_legacy(false, Some(false), 1);

    let _ = state.update(Message::SetSystemProxy(true));
    assert!(matches!(
        state.runtime.system_toggles.system_proxy,
        SystemToggleState::Pending { desired: true }
    ));

    let _ = state.update(Message::SetSystemProxy(false));
    assert!(matches!(
        state.runtime.system_toggles.system_proxy,
        SystemToggleState::Pending { desired: true }
    ));
}

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
