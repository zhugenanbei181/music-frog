//! Behavior cases for dns.
//! test-intent: behavior

use super::*;
use infiltrator_domain::dns::DnsConfigPatch;

#[test]
fn test_dns_form_dirty_and_json_sync() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateDnsFormNameserver(
        "1.1.1.1, 8.8.8.8".to_string(),
    ));
    assert!(state.editor.dns_form_dirty);
    let patch: DnsConfigPatch =
        serde_json::from_str(&state.editor.dns_json_cache).expect("dns patch json");
    assert_eq!(
        patch.nameserver,
        Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
    );
}
