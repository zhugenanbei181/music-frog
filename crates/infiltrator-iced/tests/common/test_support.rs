//! Shared fixture helpers for the `infiltrator-iced` integration harnesses.
//!
//! This module is support code only: it must never contain test cases. It is
//! mounted by the standalone test entries (e.g. `tests/headless.rs`) via
//! `#[path = "common/test_support.rs"]`, so it sees the crate's *public* API
//! exactly like any external consumer.
//! test-intent: behavior

use infiltrator_iced::demo::{DemoEnv, parse_skin};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;

/// Production window size, mirroring the production window and the demo
/// fixture default.
pub const DEFAULT_WINDOW: (f32, f32) = (1180.0, 780.0);

/// A demo `DemoEnv` pinned to `page`, default dark skin, no capture marker.
pub fn demo_env(page: Route) -> DemoEnv {
    DemoEnv {
        enabled: true,
        page,
        pane: infiltrator_iced::types::options::EditorPane::Profile,
        providers_tab: false,
        lang: "zh-CN".to_string(),
        skin: parse_skin("dark"),
        window_size: DEFAULT_WINDOW,
        capture_marker: None,
    }
}

/// Fully populated demo [`AppState`] (Overview page): groups, profiles,
/// connections, traffic, rules and zh-CN locale — with `runtime == None`,
/// so every live-runtime guard in the update paths no-ops naturally.
pub fn demo_state() -> AppState {
    let (state, _task) = AppState::demo(&demo_env(Route::Overview));
    state
}
