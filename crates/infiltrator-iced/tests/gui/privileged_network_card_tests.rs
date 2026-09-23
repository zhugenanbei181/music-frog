use super::*;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;

#[test]
fn card_consumes_cleaned_readback_without_fabricating_active_state() {
    let (mut state, _) = AppState::new();
    let snapshot = PrivilegedNetworkSnapshot::cleaned(2, 3, false);
    let _ = state.update(Message::PrivilegedNetworkRegressionUpdated(Ok(
        snapshot.clone()
    )));
    assert_eq!(state.runtime.privileged_network, snapshot);
    let lang = Lang(&state.shell.lang);
    let _ = privileged_network_card(&state, &lang);
}
