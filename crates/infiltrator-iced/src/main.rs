pub mod app;
pub mod autostart;
pub mod locales;
pub mod state;
pub mod subscription;
pub mod tray;
pub mod types;
pub mod update;
pub mod utils;
pub mod view;
pub mod view_root;

#[cfg(test)]
mod tests;

pub use state::AppState;
pub use types::{InfiltratorError, Message, Route, RuntimeStatus};

use iced::{application, window};
use single_instance::SingleInstance;
use std::fs::File;
use std::io::Write;
use std::panic;

fn main() -> iced::Result {
    let log_dir = mihomo_platform::get_home_dir().unwrap_or_else(|_| std::env::temp_dir());
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
        .window(window::Settings {
            size: (1000.0, 700.0).into(),
            exit_on_close_request: false,
            ..Default::default()
        })
        .run()
}
