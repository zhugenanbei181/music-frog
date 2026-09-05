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
pub mod configs_dir;
pub mod configuration;
pub mod demo;
mod notify;
pub mod network;
pub mod routing_application;
pub mod state;
pub mod subscription;
pub mod settings_store;
pub mod toast_state;
pub mod tray;
pub mod types;
pub mod update;
pub mod utils;
pub mod view;
pub mod view_root;

/// Registry value name for this app's Windows autostart entry (distinct from
/// the legacy Tauri client's entry so both can coexist).
pub const AUTOSTART_REG_NAME: &str = "MusicFrogInfiltrator";

#[cfg(test)]
mod test_mounts;

use crate::state::AppState;
use iced::{application, window};
use single_instance::SingleInstance;
use std::fs::File;
use std::io::Write;
use std::panic;
use std::path::Path;

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

    let log_dir = infiltrator_desktop::storage::home_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&log_dir);
    let crash_log_path = log_dir.join("infiltrator_crash.log");

    let instance = match SingleInstance::new("com.musicfrog.infiltrator") {
        Ok(i) => i,
        Err(e) => {
            if let Ok(mut file) = File::create(log_dir.join("startup_critical.log")) {
                let _ = file.write_all(format!("Mutex failure: {}\n", e).as_bytes());
            }
            return Ok(());
        }
    };
    if !instance.is_single() {
        return Ok(());
    }

    panic::set_hook(Box::new(move |info| {
        let _ = infiltrator_desktop::proxy::apply_system_proxy(None);
        let msg = info.to_string();
        // 1) 既有原始 crash log（先行落盘，行为不变）。
        if let Ok(mut file) = File::create(&crash_log_path) {
            let _ = file.write_all(msg.as_bytes());
        }
        eprintln!("PANIC: {}", msg);
        // 2) 结构化脱敏上报（平台契约 §7c）：mihomo-platform 的
        //    CrashReporter 只收集到本地 JSON（模块本身无任何网络路径），
        //    整体 best-effort——这里任何失败都不允许影响崩溃路径本身。
        write_sanitized_crash_report(&log_dir, &msg);
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

/// Innermost frames kept in the sanitized report's backtrace summary.
pub(crate) const BACKTRACE_SUMMARY_LINES: usize = 32;

/// Best-effort structured crash report delegated to the desktop host adapter.
fn write_sanitized_crash_report(log_dir: &Path, panic_message: &str) {
    let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        infiltrator_desktop::crash::write_sanitized_report(
            log_dir,
            panic_message,
            env!("CARGO_PKG_VERSION"),
            &backtrace_summary(&backtrace),
        );
    }));
}

/// Cap a captured backtrace to a compact summary; std prints innermost
/// frames first, so the head carries the useful stack.
pub(crate) fn backtrace_summary(backtrace: &str) -> String {
    backtrace
        .lines()
        .take(BACKTRACE_SUMMARY_LINES)
        .collect::<Vec<_>>()
        .join("\n")
}
