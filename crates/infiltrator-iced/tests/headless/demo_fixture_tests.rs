//! Demo fixture contract, driven exclusively through the public
//! `infiltrator_iced` API (integration harness, external-crate view).
//! test-intent: behavior

use crate::test_support::{demo_state, DEFAULT_WINDOW};
use infiltrator_iced::{Message, Route, RuntimeStatus};

#[test]
fn demo_fixture_inventory_matches_the_demo_contract() {
    let state = demo_state();

    // Demo gate: fixture state, never a live runtime.
    assert!(state.demo, "demo flag must be set");
    assert!(state.runtime.is_none(), "demo mode must not own a runtime");

    // Proxy inventory: at least one selectable group.
    let groups = state.proxies.values().filter(|p| p.is_group()).count();
    assert!(groups > 0, "expected non-empty group inventory, got {groups}");

    // Profiles: the demo subscription shelf carries three entries.
    assert!(
        state.profiles.len() >= 3,
        "expected >= 3 demo profiles, got {}",
        state.profiles.len()
    );
    assert!(state.profiles.iter().any(|p| p.active));

    // Connections: the runtime page ships a full snapshot.
    let snapshot = state
        .connections
        .as_ref()
        .expect("demo seeds a connection snapshot");
    assert!(
        snapshot.connections.len() >= 10,
        "expected >= 10 demo connections, got {}",
        snapshot.connections.len()
    );

    // Locale: demo boots in Chinese.
    assert_eq!(state.lang, "zh-CN");
}

#[test]
fn navigate_messages_retarget_current_route_without_touching_runtime() {
    let mut state = demo_state();

    for route in [
        Route::Proxies,
        Route::Runtime,
        Route::Rules,
        Route::Settings,
    ] {
        let _ = state.update(Message::Navigate(route));
        assert_eq!(state.current_route, route, "Navigate must retarget route");
        // Demo gate: navigation stays pure state — no runtime is ever
        // created, status keeps reporting the fixture's running demo.
        assert!(state.runtime.is_none(), "demo gate: no live runtime");
        assert!(matches!(state.status, RuntimeStatus::Running));
    }
}

#[test]
fn demo_env_helper_matches_the_production_window() {
    // The fixture window mirrors the production window defaults used by
    // `run()` and the visual-capture pipeline.
    assert_eq!(DEFAULT_WINDOW, (1180.0, 780.0));
}
