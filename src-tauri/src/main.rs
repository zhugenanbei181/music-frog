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
use mihomo_platform::set_home_dir_override;
use tauri::{Manager, RunEvent};

use crate::{
    admin_context::TauriAdminContext,
    app_state::AppState,
    frontend::{open_frontend, spawn_frontends},
    paths::app_data_dir,
    platform::show_error_dialog,
    runtime::spawn_runtime,
    settings::load_settings,
    tray::create_tray,
    utils::parse_launch_ports,
};
use infiltrator_admin::{EVENT_CORE_CHANGED, EVENT_PROFILES_CHANGED, SubscriptionScheduler};

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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let state = app.state::<AppState>().inner().clone();
            open_frontend(state);
        }))
        .setup(move |app| {
            match app_data_dir(app.app_handle()) {
                Ok(base_dir) => {
                    if !set_home_dir_override(base_dir.clone()) {
                        warn!("data dir override already set: {}", base_dir.display());
                    }
                }
                Err(err) => {
                    warn!("failed to resolve data dir for mihomo config: {err}");
                }
            }
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::block_on(async {
                state.set_app_handle(app.app_handle().clone()).await;
                if let Err(err) = load_settings(&state).await {
                    warn!("failed to load settings: {err}");
                }
            });
            create_tray(app.app_handle(), state.clone())?;
            spawn_runtime(app.app_handle().clone(), state.clone());
            spawn_frontends(
                app.app_handle().clone(),
                state,
                launch_ports.static_port,
                launch_ports.admin_port,
            );
            {
                let app_handle = app.app_handle().clone();
                let state_for_events = app.state::<AppState>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let mut receiver = state_for_events.admin_event_bus().subscribe();
                    loop {
                        match receiver.recv().await {
                            Ok(event) => {
                                if event.kind == EVENT_PROFILES_CHANGED {
                                    if let Err(err) = crate::tray::refresh_profile_switch_submenu(
                                        &app_handle,
                                        &state_for_events,
                                    )
                                    .await
                                    {
                                        warn!("failed to refresh profile switch submenu: {err:#}");
                                    }
                                } else if event.kind == EVENT_CORE_CHANGED {
                                    if let Err(err) = crate::tray::refresh_core_versions_submenu(
                                        &app_handle,
                                        &state_for_events,
                                    )
                                    .await
                                    {
                                        warn!("failed to refresh core versions submenu: {err:#}");
                                    }
                                    state_for_events.refresh_core_version_info().await;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                continue;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                });
            }
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
                "初始化 MusicFrog Despicable Infiltrator 失败: {err:#}"
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
