//! Behavior cases for set.
//! test-intent: behavior

use super::*;

#[test]
fn test_set_advanced_mode_updates_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::SetAdvancedMode(
        DnsTab::Dns,
        AdvancedEditMode::Json,
    ));
    assert_eq!(state.editor.dns_mode, AdvancedEditMode::Json);
}
