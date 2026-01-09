#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admin_context;
mod app_state;
mod autostart;
mod core_update;
mod factory_reset;
mod frontend;
mod locales;
mod paths;
mod platform;
mod runtime;
mod settings;
mod system_proxy;
mod tray;
mod utils;

use log::warn;
use tauri::{Manager, RunEvent};

use crate::{
    admin_context::TauriAdminContext,
    app_state::AppState,
    frontend::spawn_frontends,
    platform::show_error_dialog,
    runtime::spawn_runtime,
    settings::load_settings,
    tray::create_tray,
    utils::parse_launch_ports,
};
use despicable_infiltrator_core::SubscriptionScheduler;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("Critical Panic: {info}");
        // Attempt to log if logger is ready, but definitely show dialog
        log::error!("{msg}");
        crate::platform::show_error_dialog(msg);
    }));

    let launch_ports = parse_launch_ports();
    let builder = tauri::Builder::default()
        .manage(AppState::default())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::block_on(async {
                state.set_app_handle(app.app_handle().clone()).await;
                if let Err(err) = load_settings(&state).await {
                    warn!("failed to load settings: {err}");
                }
            });
            create_tray(&app.app_handle(), state.clone())?;
            spawn_runtime(app.app_handle().clone(), state.clone());
            spawn_frontends(
                app.app_handle().clone(),
                state,
                launch_ports.static_port,
                launch_ports.admin_port,
            );
            tauri::async_runtime::block_on(async {
                let scheduler = SubscriptionScheduler::start(TauriAdminContext {
                    app: app.app_handle().clone(),
                    app_state: app.state::<AppState>().inner().clone(),
                });
                app.state::<AppState>()
                    .inner()
                    .set_subscription_scheduler(scheduler)
                    .await;
            });
            Ok(())
        });

    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(err) => {
            show_error_dialog(format!(
                "初始化 Mihomo Despicable Infiltrator 失败: {err:#}"
            ));
            return;
        }
    };

    let state_for_run = app.state::<AppState>().inner().clone();

    app.run(move |_app_handle, event| match event {
        RunEvent::ExitRequested { .. } | RunEvent::Exit => {
            let state = state_for_run.clone();
            tauri::async_runtime::block_on(async move {
                state.shutdown_all().await;
            });
        }
        _ => {}
    });
}
