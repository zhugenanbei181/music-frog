//! Iced desktop composition root.
//!
//! This module owns desktop-only process concerns: single-instance locking,
//! crash recovery, filesystem setup, and native window composition. The
//! `view`, `update`, and shared surface model modules remain inbound UI code;
//! they are not the place to decide how a desktop host is bootstrapped.

use crate::demo;
use crate::demo::DemoEnv;
use crate::state::AppState;
use crate::surface::SurfaceBridge;
use crate::window_chrome::{MIN_WINDOW_SIZE, window_settings};
use iced::application;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_desktop::crash::write_sanitized_report;
use infiltrator_desktop::exit_cleanup::{install, run_now};
use infiltrator_desktop::product::DesktopProductSession;
use infiltrator_desktop::storage::home_dir;
use single_instance::SingleInstance;
use std::backtrace::Backtrace;
use std::env::temp_dir;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::panic;
use std::path::Path;
use std::sync::Arc;

/// Run the desktop product composition.
pub fn run() -> iced::Result {
    let demo_env = DemoEnv::from_environment();
    if demo_env.enabled {
        return demo::run(demo_env);
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
    let _termination_handler = match install() {
        Ok(handler) => Some(handler),
        Err(error) => {
            eprintln!("failed to install process termination cleanup: {error}");
            None
        }
    };
    let exit_cleanup: Arc<dyn Fn() + Send + Sync> = Arc::new(run_now);
    let log_dir = home_dir().unwrap_or_else(|_| temp_dir());
    let _ = create_dir_all(&log_dir);
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
        run_now();
        let message = info.to_string();
        if let Ok(mut file) = File::create(&crash_log_path) {
            let _ = file.write_all(message.as_bytes());
        }
        eprintln!("PANIC: {message}");
        write_sanitized_crash_report(&log_dir, &message);
    }));

    let (session, composition_failure) = if surface_bridge.is_none() {
        match DesktopProductSession::open(SurfaceKind::IcedDesktop) {
            Ok(session) => (Some(session), None),
            Err(error) => (
                None,
                Some(Failure::new(ErrorCode::NotReady, error.to_string(), true)),
            ),
        }
    } else {
        (None, None)
    };
    let surface_bridge = surface_bridge.or_else(|| {
        session
            .as_ref()
            .map(|session| SurfaceBridge::from_pump(session.surface_pump()))
    });
    let core_application = session.as_ref().map(DesktopProductSession::application);
    let host_runtime = session.as_ref().map(DesktopProductSession::host_runtime);
    let initializer = move || {
        let (mut state, task) = AppState::new();
        if let Some(application) = core_application.clone() {
            let snapshot = application.snapshot();
            state.runtime.core_lifecycle = snapshot.lifecycle_snapshot();
            state.commands = Some(application.as_ref().clone());
        }
        if let Some(host) = host_runtime.clone() {
            state.sync_runtime_slot(Some(host));
        }
        state.runtime.host_composition_failure = composition_failure.clone();
        if let Some(failure) = composition_failure.clone() {
            state.runtime.lifecycle_failure = Some(failure.clone());
            state.shell.error_msg = Some(failure.message);
        }
        if let Some(bridge) = surface_bridge.clone() {
            state.attach_surface_bridge(bridge);
        }
        state.attach_exit_cleanup(exit_cleanup.clone());
        (state, task)
    };

    let result = application(initializer, AppState::update, AppState::view)
        .title(AppState::title)
        .theme(AppState::theme)
        .subscription(AppState::subscription)
        .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
        .default_font(iced::Font::with_name("Inter"))
        .window(window_settings(
            (1180.0, 780.0),
            // Kept below the 600px Compact boundary so every responsive tier
            // (Compact/Medium/Expanded/Ultra) is reachable by resizing.
            MIN_WINDOW_SIZE,
        ))
        .run();
    drop(session);
    result
}

fn write_sanitized_crash_report(log_dir: &Path, panic_message: &str) {
    let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let backtrace = Backtrace::force_capture().to_string();
        write_sanitized_report(
            log_dir,
            panic_message,
            env!("CARGO_PKG_VERSION"),
            &crate::backtrace_summary(&backtrace),
        );
    }));
}
