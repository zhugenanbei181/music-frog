//! Demo mode: render the REAL application against in-memory fixture data.
//!
//! Enabled by `--demo` on the command line or `INFILTRATOR_DEMO=1` in the
//! environment. A demo session shares every view/update code path with the
//! production app but:
//!
//! * never spawns mihomo, the admin server, the system tray or any process,
//! * never writes settings / profiles / rules files, never touches the
//!   system proxy, autostart or the network,
//! * pre-populates [`AppState`] with realistic Chinese-locale fixtures so
//!   every page renders fully (see [`AppState::demo`]).
//!
//! Environment contract used by the visual-capture tooling (names are part
//! of the contract — do not rename):
//!
//! | Variable                    | Meaning                                        |
//! |-----------------------------|------------------------------------------------|
//! | `INFILTRATOR_DEMO=1`        | enable demo mode (`--demo` argv also works)    |
//! | `INFILTRATOR_PAGE`          | initial route: overview\|proxies\|runtime\|rules\|dns\|profiles\|sync\|editor\|settings |
//! | `INFILTRATOR_SKIN`          | `light` or `dark` (default dark)               |
//! | `INFILTRATOR_WINDOW_SIZE`   | `WxH` (default 1180x780)                       |
//! | `INFILTRATOR_CAPTURE_MARKER`| file getting `CAPTURE_READY page=<p> skin=<s>` appended after the first rendered frame |
//!
//! Fixture tables live in [`fixtures`] / [`proxy_fixtures`], the demo
//! [`AppState`] constructor in [`state`] and the capture-marker plumbing in
//! [`capture`].

mod capture;
mod fixtures;
mod proxy_fixtures;
mod state;

use crate::state::AppState;
use crate::types::Route;
// Test-only re-export so the path-mounted demo tests can glob `super::*`
// for the status type they assert on.
#[cfg(test)]
pub(crate) use crate::types::RuntimeStatus;
use iced::{application, window};
use std::path::PathBuf;

/// Default window size, mirrors the production window in `main.rs`.
const DEFAULT_WINDOW: (f32, f32) = (1180.0, 780.0);

/// Everything demo mode needs to know, resolved once from argv + environment.
#[derive(Debug, Clone)]
pub struct DemoEnv {
    pub enabled: bool,
    pub page: Route,
    pub skin: iced::Theme,
    pub window_size: (f32, f32),
    pub capture_marker: Option<PathBuf>,
}

impl DemoEnv {
    /// Resolve demo settings from `--demo` argv and the `INFILTRATOR_*`
    /// environment variables. Invalid values fall back to defaults.
    pub fn from_environment() -> Self {
        let enabled = std::env::args().any(|arg| arg == "--demo")
            || std::env::var("INFILTRATOR_DEMO").is_ok_and(|v| v.trim() == "1");
        Self {
            enabled,
            page: std::env::var("INFILTRATOR_PAGE")
                .ok()
                .and_then(|v| parse_page(&v))
                .unwrap_or(Route::Overview),
            skin: std::env::var("INFILTRATOR_SKIN")
                .ok()
                .map(|v| parse_skin(&v))
                .unwrap_or(iced::Theme::Dark),
            window_size: std::env::var("INFILTRATOR_WINDOW_SIZE")
                .ok()
                .map(|v| parse_window_size(&v))
                .unwrap_or(DEFAULT_WINDOW),
            capture_marker: std::env::var("INFILTRATOR_CAPTURE_MARKER")
                .ok()
                .map(PathBuf::from)
                .filter(|p| !p.as_os_str().is_empty()),
        }
    }
}

/// `overview|proxies|runtime|rules|dns|profiles|sync|editor|settings` -> Route.
/// Unknown values yield `None` (callers fall back to [`Route::Overview`]).
pub fn parse_page(value: &str) -> Option<Route> {
    match value.trim().to_ascii_lowercase().as_str() {
        "overview" => Some(Route::Overview),
        "proxies" => Some(Route::Proxies),
        "runtime" => Some(Route::Runtime),
        "rules" => Some(Route::Rules),
        "dns" => Some(Route::Dns),
        "profiles" => Some(Route::Profiles),
        "sync" => Some(Route::Sync),
        "editor" => Some(Route::Editor),
        "settings" => Some(Route::Settings),
        _ => None,
    }
}

/// Inverse of [`parse_page`] — the canonical env name of a route.
pub fn route_env_name(route: Route) -> &'static str {
    match route {
        Route::Overview => "overview",
        Route::Proxies => "proxies",
        Route::Runtime => "runtime",
        Route::Rules => "rules",
        Route::Dns => "dns",
        Route::Profiles => "profiles",
        Route::Sync => "sync",
        Route::Editor => "editor",
        Route::Settings => "settings",
    }
}

/// `light|dark` -> iced theme; unknown values fall back to dark.
pub fn parse_skin(value: &str) -> iced::Theme {
    if value.trim().eq_ignore_ascii_case("light") {
        iced::Theme::Light
    } else {
        iced::Theme::Dark
    }
}

/// Canonical `light` / `dark` name of an iced theme (for the capture marker).
pub fn skin_name(theme: &iced::Theme) -> &'static str {
    if matches!(theme, iced::Theme::Light) {
        "light"
    } else {
        "dark"
    }
}

/// `WxH` -> `(w, h)`; anything unparsable or non-positive falls back to the
/// default production window size.
pub fn parse_window_size(value: &str) -> (f32, f32) {
    let parts: Option<Vec<f32>> = value
        .trim()
        .split(['x', 'X'])
        .map(|part| part.trim().parse::<f32>().ok())
        .collect();
    match parts.as_deref() {
        Some([w, h]) if *w > 0.0 && *h > 0.0 => (*w, *h),
        _ => DEFAULT_WINDOW,
    }
}

/// Run the iced application in demo mode. Same views/update/subscription as
/// the production entry point, but booting from [`AppState::demo`], sized
/// from `INFILTRATOR_WINDOW_SIZE` and without any system integration
/// (no single-instance mutex, no tray, no settings bootstrap).
pub fn run(env: DemoEnv) -> iced::Result {
    let window_size = env.window_size;
    application(
        move || AppState::demo(&env),
        AppState::update,
        AppState::view,
    )
    .title(AppState::title)
    .theme(AppState::theme)
    .subscription(AppState::subscription)
    // Bundled typography: identical to the production window (see main.rs).
    .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
    .default_font(iced::Font::with_name("Inter"))
    .window(window::Settings {
        size: window_size.into(),
        min_size: Some((960.0, 640.0).into()),
        exit_on_close_request: false,
        ..Default::default()
    })
    .run()
}

#[cfg(test)]
#[path = "../tests/gui/demo_tests.rs"]
mod tests;
