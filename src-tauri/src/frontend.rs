use log::{error, info, warn};
use tauri::AppHandle;

use crate::{
    admin_context::TauriAdminContext,
    app_state::AppState,
    paths::{resolve_admin_dir, resolve_main_dir},
    platform::open_in_browser,
};
use infiltrator_core::servers::{AdminServerHandle, StaticServerHandle};
use infiltrator_core::servers as core_servers;

pub(crate) fn spawn_frontends(
    app: AppHandle,
    state: AppState,
    static_port: Option<u16>,
    admin_port: Option<u16>,
) {
    let app_for_main = app.clone();
    let state_for_main = state.clone();
    tauri::async_runtime::spawn(async move {
        match start_main_frontend(&app_for_main, static_port).await {
            Ok(handle) => {
                let url = handle.url.clone();
                info!("Zashboard 静态界面已托管在 {}", url);
                state_for_main.set_static_server(handle).await;
                state_for_main
                    .update_static_info_text(format!("静态站点: {}", url))
                    .await;
                if state_for_main.open_webui_on_startup().await {
                    if let Err(err) = open_in_browser(&url) {
                        warn!("无法自动打开浏览器: {err}");
                    }
                }
            }
            Err(err) => {
                error!("启动静态站点失败: {err:#}");
                crate::platform::show_error_dialog(format!("静态站点启动失败: {err:#}"));
                state_for_main
                    .update_static_info_text(format!("静态站点: 启动失败 ({err})"))
                    .await;
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        match start_admin_frontend(&app, state.clone(), admin_port).await {
            Ok(handle) => {
                let url = handle.url.clone();
                state.set_admin_server(handle).await;
                state
                    .update_admin_info_text(format!("配置管理: {}", url))
                    .await;
            }
            Err(err) => {
                error!("启动配置管理界面失败: {err:#}");
                state
                    .update_admin_info_text(format!("配置管理: 启动失败 ({err})"))
                    .await;
            }
        }
    });
}

async fn start_main_frontend(
    app: &AppHandle,
    preferred_port: Option<u16>,
) -> anyhow::Result<StaticServerHandle> {
    let main_dir = resolve_main_dir(app)?;
    info!("Zashboard 静态目录: {}", main_dir.display());
    core_servers::start_static_server(main_dir, preferred_port, 4173).await
}

async fn start_admin_frontend(
    app: &AppHandle,
    state: AppState,
    preferred_port: Option<u16>,
) -> anyhow::Result<AdminServerHandle> {
    let admin_dir = resolve_admin_dir(app)?;
    info!("配置管理静态目录: {}", admin_dir.display());
    let event_bus = state.admin_event_bus();
    let ctx = TauriAdminContext {
        app: app.clone(),
        app_state: state,
    };
    core_servers::start_admin_server(
        admin_dir,
        ctx,
        preferred_port,
        5210,
        event_bus,
    )
    .await
}

pub(crate) fn open_frontend(state: AppState) {
    tauri::async_runtime::spawn(async move {
        match state.static_server_url().await {
            Some(url) => {
                if let Err(err) = open_in_browser(&url) {
                    error!("打开浏览器失败: {err}");
                }
            }
            None => warn!("静态站点尚未就绪"),
        }
    });
}

pub(crate) fn open_admin_frontend(state: AppState) {
    open_admin_frontend_anchor(state, None);
}

pub(crate) fn open_admin_frontend_anchor(state: AppState, anchor: Option<String>) {
    tauri::async_runtime::spawn(async move {
        match state.admin_server_url().await {
            Some(url) => {
                let target = build_admin_url(&url, anchor.as_deref());
                if let Err(err) = open_in_browser(&target) {
                    error!("打开配置管理界面失败: {err}");
                }
            }
            None => warn!("配置管理界面尚未就绪"),
        }
    });
}

fn build_admin_url(base: &str, anchor: Option<&str>) -> String {
    match anchor {
        Some(value) if !value.trim().is_empty() => format!("{base}#{value}"),
        _ => base.to_string(),
    }
}
