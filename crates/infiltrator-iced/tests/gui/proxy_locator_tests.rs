//! test-intent: behavior
//! The locator names the node the owner published as selected, and a
//! locate is served only when that node is on the page.

use crate::state::AppState;
use crate::state::proxy_locator::ProxyLocator;

#[test]
fn a_request_names_one_group_and_one_node_and_idle_names_none() {
    let mut locator = ProxyLocator::default();
    assert_eq!(locator.target(), None);
    locator = ProxyLocator::Pending {
        group: "GLOBAL".into(),
        node: "proxy-a".into(),
    };
    assert_eq!(locator.target(), Some(("GLOBAL", "proxy-a")));
}

#[test]
fn a_request_for_a_group_without_a_published_selection_has_nothing_to_locate() {
    let state = AppState::empty();
    assert_eq!(state.outbound_group_current_node("GLOBAL"), None);
    assert!(!state.outbound_group_locate_is_ready());
}
