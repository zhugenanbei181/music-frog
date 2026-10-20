//! Behavior cases for set.
//! test-intent: behavior

use super::*;

#[test]
fn test_set_advanced_mode_updates_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::SetAdvancedMode(DnsPageTab::Dns, DnsEditMode::Json));
    assert_eq!(state.editor.dns_model.edit_mode(), DnsEditMode::Json);
}
