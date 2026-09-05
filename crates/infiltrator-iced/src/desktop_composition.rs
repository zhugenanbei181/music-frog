//! Iced desktop composition root.
//!
//! This module owns desktop-only process concerns: single-instance locking,
//! crash recovery, filesystem setup, and native window composition. The
//! `view`, `update`, and shared surface model modules remain inbound UI code;
//! they are not the place to decide how a desktop host is bootstrapped.

use crate::state::AppState;
use crate::surface::SurfaceBridge;
use iced::{application, window};
use infiltrator_application::surface_application::SurfacePump;
use single_instance::SingleInstance;
use std::fs::File;
use std::io::Write;
use std::panic;
use std::path::Path;

/// Run the desktop product composition.
pub fn run() -> iced::Result {
    let demo_env = crate::demo::DemoEnv::from_environment();
    if demo_env.enabled {
        return crate::demo::run(demo_env);
    }

    run_production(None)
}

/// Run the Iced desktop product with the application-owned shared surface
/// pump. The pump is retained by the Iced bridge, while all reads and runtime
/// details remain below the desktop/application composition boundary.
pub fn run_with_surface_pump(pump: SurfacePump) -> iced::Result {
    run_production(Some(SurfaceBridge::from_pump(pump)))
}

fn run_production(surface_bridge: Option<SurfaceBridge>) -> iced::Result {
    let _termination_handler = match infiltrator_desktop::exit_cleanup::install() {
        Ok(handler) => Some(handler),
        Err(error) => {
            eprintln!("failed to install process termination cleanup: {error}");
            None
        }
    };
    let exit_cleanup: std::sync::Arc<dyn Fn() + Send + Sync> =
        std::sync::Arc::new(infiltrator_desktop::exit_cleanup::run_now);
    let log_dir = infiltrator_desktop::storage::home_dir().unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&log_dir);
    let crash_log_path = log_dir.join("infiltrator_crash.log");

    let instance = match SingleInstance::new("com.musicfrog.infiltrator") {
        Ok(instance) => instance,
        Err(error) => {
            if let Ok(mut file) = File::create(log_dir.join("startup_critical.log")) {
                let _ = file.write_all(format!("Mutex failure: {error}\n").as_bytes());
            }
            return Ok(());
        }
    };
    if !instance.is_single() {
        return Ok(());
    }

    panic::set_hook(Box::new(move |info| {
        let _ = infiltrator_desktop::proxy::apply_system_proxy(None);
        infiltrator_desktop::exit_cleanup::run_now();
        let message = info.to_string();
        if let Ok(mut file) = File::create(&crash_log_path) {
            let _ = file.write_all(message.as_bytes());
        }
        eprintln!("PANIC: {message}");
        write_sanitized_crash_report(&log_dir, &message);
    }));

    let initializer = move || {
        let (mut state, task) = AppState::new();
        if let Some(bridge) = surface_bridge.clone() {
            state.attach_surface_bridge(bridge);
        }
        state.attach_exit_cleanup(exit_cleanup.clone());
        (state, task)
    };

    application(initializer, AppState::update, AppState::view)
        .title(AppState::title)
        .theme(AppState::theme)
        .subscription(AppState::subscription)
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

fn write_sanitized_crash_report(log_dir: &Path, panic_message: &str) {
    let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        infiltrator_desktop::crash::write_sanitized_report(
            log_dir,
            panic_message,
            env!("CARGO_PKG_VERSION"),
            &crate::backtrace_summary(&backtrace),
        );
    }));
}
