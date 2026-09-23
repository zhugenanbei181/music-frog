//! Library root of the MusicFrog Infiltrator iced frontend.
//!
//! The crate is a lib + thin binary: [`run`] owns the full application
//! bootstrap (demo-mode dispatch, single-instance guard, panic hook, bundled
//! fonts and window settings) and the binary in `main.rs` is only a wrapper
//! around it. Keeping the modules in a lib lets the `tests/` integration
//! harnesses (see `tests/common`, `tests/headless`, `tests/gui`) exercise the
//! public surface directly.

pub mod accessibility;
pub mod admin_server;
pub mod app;
pub mod configs_dir;
pub mod configuration;
pub mod demo;
pub mod desktop_composition;
pub mod gesture;
pub mod host;
pub mod ime;
pub mod mini_hud_store;
pub mod mini_hud_window;
pub mod network;
mod notify;
pub mod port_conflict_application;
pub mod routing_application;
pub mod settings_store;
pub mod shortcuts_store;
pub mod snapshot_application;
pub mod state;
pub(crate) mod state_ops;
pub mod subscription;
pub mod surface;
pub mod toast_state;
pub mod tray;
pub mod types;
pub mod update;
pub mod utils;
pub mod version_application;
pub mod view;
pub mod view_root;
pub mod window_chrome;

/// Registry value name for this app's Windows autostart entry (distinct from
/// the legacy Tauri client's entry so both can coexist).
pub const AUTOSTART_REG_NAME: &str = "MusicFrogInfiltrator";

#[cfg(test)]
mod test_mounts;

/// Bootstrap and run the iced application.
///
/// Behavior is identical to the former `main()` body:
///
/// * `--demo` / `INFILTRATOR_DEMO=1` renders the real app against fixture
///   data with zero production side effects (no mihomo spawn, no system
///   proxy changes, no tray, no admin server, no settings writes),
/// * otherwise a single-instance guard, a crash-log panic hook and the
///   production window (bundled Inter/JetBrains Mono typography) are set up.
pub fn run() -> iced::Result {
    desktop_composition::run()
}

/// Run Iced with a host-composed application surface pump. This is the
/// symmetric counterpart to Bevy's `run_with_application_surface_pump`.
pub fn run_with_surface_pump(
    pump: infiltrator_application::surface_application::SurfacePump,
) -> iced::Result {
    desktop_composition::run_with_surface_pump(pump)
}

/// Innermost frames kept in the sanitized report's backtrace summary.
pub(crate) const BACKTRACE_SUMMARY_LINES: usize = 32;

/// Best-effort structured crash report delegated to the desktop host adapter.
/// Cap a captured backtrace to a compact summary; std prints innermost
/// frames first, so the head carries the useful stack.
pub(crate) fn backtrace_summary(backtrace: &str) -> String {
    backtrace
        .lines()
        .take(BACKTRACE_SUMMARY_LINES)
        .collect::<Vec<_>>()
        .join("\n")
}
