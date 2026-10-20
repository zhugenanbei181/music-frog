//! Behavior cases for tun.
//! test-intent: behavior

use super::*;
use infiltrator_presentation::dns_page_model::DnsOperation;

#[test]
fn test_tun_form_invalid_mtu_blocks_save() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateTunFormMtu("abc".to_string()));
    let _ = state.update(Message::SaveTunConfig);
    assert!(!state.editor.dns_operation_running(DnsOperation::SaveTun));
    assert!(matches!(
        state.runtime.configuration_save_state,
        ConfigurationSaveState::Failed { .. }
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
