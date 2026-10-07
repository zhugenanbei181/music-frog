//! Behavior cases for tun.
//! test-intent: behavior

use super::*;

#[test]
fn test_tun_form_invalid_mtu_blocks_save() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateTunFormMtu("abc".to_string()));
    let _ = state.update(Message::SaveTunConfig);
    assert!(!state.editor.is_saving_tun);
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));
    assert!(
        state
            .editor
            .advanced_validation
            .tun
            .as_ref()
            .is_some_and(|msg| msg.to_ascii_lowercase().contains("mtu"))
    );
}
