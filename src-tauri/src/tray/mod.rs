// Mod file for tray module
pub mod handlers;
pub mod menu;

use tauri::{include_image, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, AppHandle};
use crate::app_state::AppState;
use crate::frontend::open_frontend;
use log::warn;

pub(crate) use menu::{
    refresh_core_versions_submenu,
    refresh_profile_switch_submenu,
    refresh_proxy_groups_submenu,
    refresh_tray_menu,
    refresh_tun_menu_item,
};

pub(crate) fn create_tray(app: &AppHandle, state: AppState) -> tauri::Result<()> {
    let tray_menu = tauri::async_runtime::block_on(async { menu::build_tray_menu(app, &state).await });
    let (menu, items) = match tray_menu {
        Ok(result) => result,
        Err(err) => {
            warn!("failed to build tray menu: {err}");
            crate::platform::show_error_dialog("托盘初始化失败，已启用精简托盘菜单".to_string());
            return menu::build_fallback_tray(app, state);
        }
    };
    let is_admin = crate::platform::is_running_as_admin();
    tauri::async_runtime::block_on(async {
        state.set_tray_info_items(items).await;
        state.refresh_system_proxy_state().await;
        state.update_admin_privilege_text(is_admin).await;
        state.refresh_core_version_info().await;
    });
    let state_for_menu = state.clone();
    let state_for_tray_click = state.clone();

    TrayIconBuilder::with_id("metacube-tray")
        .tooltip("MusicFrog Despicable Infiltrator")
        .icon(include_image!("icons/tray.ico"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            handlers::handle_menu_event(app, event, &state_for_menu);
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_frontend(state_for_tray_click.clone());
            } else if let TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } = event {
                // Refresh TUN state when menu is about to be shown
                let state = state_for_tray_click.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = crate::tray::refresh_tun_menu_item(&state).await {
                        warn!("failed to refresh tun state on tray click: {err}");
                    }
                });
            }
        })
        .build(app)?;

    // fire off a summary refresh when tray is ready
    let app_handle = app.clone();
    let summary_state = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(runtime) = summary_state.runtime().await {
            if let Ok(summary) = runtime.summary().await {
                if let Err(err) = tauri::Emitter::emit(&app_handle, "mihomo://summary", &summary) {
                    warn!("failed to emit summary event: {err}");
                }
            }
        }
    });

    Ok(())
}
