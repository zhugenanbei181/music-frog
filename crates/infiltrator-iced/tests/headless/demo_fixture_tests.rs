//! Demo fixture contract, driven exclusively through the public
//! `infiltrator_iced` API (integration harness, external-crate view).
//! test-intent: behavior

use crate::test_support::{DEFAULT_WINDOW, demo_state};
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_iced::types::runtime::RuntimeStatus;

#[test]
fn demo_fixture_inventory_matches_the_demo_contract() {
    let state = demo_state();

    // Demo gate: fixture state, never a live runtime.
    assert!(state.shell.demo, "demo flag must be set");
    assert!(
        state.runtime.runtime.is_none(),
        "demo mode must not own a runtime"
    );

    // Proxy inventory: at least one selectable group.
    let groups = state
        .runtime
        .proxies
        .values()
        .filter(|p| p.is_group())
        .count();
    assert!(
        groups > 0,
        "expected non-empty group inventory, got {groups}"
    );

    // Profiles: the demo subscription shelf carries three entries.
    assert!(
        state.profile.profiles.len() >= 3,
        "expected >= 3 demo profiles, got {}",
        state.profile.profiles.len()
    );
    assert!(state.profile.profiles.iter().any(|p| p.active));

    // Connections: the runtime page ships a full snapshot.
    let snapshot = state
        .diag
        .connections
        .as_ref()
        .expect("demo seeds a connection snapshot");
    assert!(
        snapshot.connections.len() >= 10,
        "expected >= 10 demo connections, got {}",
        snapshot.connections.len()
    );

    // Locale: demo boots in Chinese.
    assert_eq!(state.shell.lang, "zh-CN");
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
        assert_eq!(
            state.shell.current_route, route,
            "Navigate must retarget route"
        );
        // Demo gate: navigation stays pure state — no runtime is ever
        // created, status keeps reporting the fixture's running demo.
        assert!(
            state.runtime.runtime.is_none(),
            "demo gate: no live runtime"
        );
        assert!(matches!(state.runtime.status, RuntimeStatus::Running));
    }
}

#[test]
fn demo_env_helper_matches_the_production_window() {
    // The fixture window mirrors the production window defaults used by
    // `run()` and the visual-capture pipeline.
    assert_eq!(DEFAULT_WINDOW, (1180.0, 780.0));
}
