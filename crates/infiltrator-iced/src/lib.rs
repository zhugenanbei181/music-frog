//! Library root of the MusicFrog Infiltrator iced frontend.
//!
//! The crate is a lib + thin binary: [`run`] owns the full application
//! bootstrap (demo-mode dispatch, single-instance guard, panic hook, bundled
//! fonts and window settings) and the binary in `main.rs` is only a wrapper
//! around it. Keeping the modules in a lib lets the `tests/` integration
//! harnesses (see `tests/common`, `tests/headless`, `tests/gui`) exercise the
//! public surface directly.

pub mod admin_server;
pub mod app;
pub mod demo;
pub mod state;
pub mod subscription;
pub mod toast_state;
pub mod tray;
pub mod types;
pub mod update;
pub mod utils;
pub mod view;
pub mod view_root;

pub use infiltrator_shared::{autostart, locales};

/// Registry value name for this app's Windows autostart entry (distinct from
/// the legacy Tauri client's entry so both can coexist).
pub const AUTOSTART_REG_NAME: &str = "MusicFrogInfiltrator";

#[cfg(test)]
mod test_mounts;

pub use state::AppState;
pub use types::{InfiltratorError, Message, Route, RuntimeStatus};

use iced::{application, window};
use single_instance::SingleInstance;
use std::fs::File;
use std::io::Write;
use std::panic;

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
    let demo_env = demo::DemoEnv::from_environment();
    if demo_env.enabled {
        return demo::run(demo_env);
    }

    let log_dir = mihomo_platform::paths::get_home_dir().unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&log_dir);
    let crash_log_path = log_dir.join("infiltrator_crash.log");

    let instance = match SingleInstance::new("com.musicfrog.infiltrator") {
        Ok(i) => i,
        Err(e) => {
            if let Ok(mut file) = File::create(&log_dir.join("startup_critical.log")) {
                let _ = file.write_all(format!("Mutex failure: {}\n", e).as_bytes());
            }
            return Ok(());
        }
    };
    if !instance.is_single() {
        return Ok(());
    }

    panic::set_hook(Box::new(move |info| {
        let msg = info.to_string();
        if let Ok(mut file) = File::create(&crash_log_path) {
            let _ = file.write_all(msg.as_bytes());
        }
        eprintln!("PANIC: {}", msg);
    }));

    application(AppState::new, AppState::update, AppState::view)
        .title(AppState::title)
        .theme(AppState::theme)
        .subscription(AppState::subscription)
        // Bundled typography: Inter (Regular/Medium/SemiBold, SIL OFL 1.1)
        // as the default UI face, JetBrains Mono (SIL OFL 1.1) for latency /
        // bytes numerals — see view::theme::MONO and THIRD-PARTY-NOTICES.md.
        .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
        .default_font(iced::Font::with_name("Inter"))
        .window(window::Settings {
            size: (1180.0, 780.0).into(),
            min_size: Some((960.0, 640.0).into()),
            exit_on_close_request: false,
            ..Default::default()
        })
        .run()
}
