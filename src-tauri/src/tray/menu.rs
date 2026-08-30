//! Tray menu module facade.
//!
//! Submodules are grouped by business domain:
//! - [`ids`]: hash-based menu item IDs and ID-to-entity maps
//! - [`helpers`]: label truncation and submenu manipulation
//! - [`modes`]: proxy mode submenu (rule/global/direct/script)
//! - [`tun`]: TUN mode menu item
//! - [`profiles`]: profile switch submenu
//! - [`proxies`]: proxy groups submenu
//! - [`core_versions`]: installed core versions submenu
//! - [`sections`]: static sections (About, Advanced, Sync & backup)
//!
//! This root owns the top-level assembly, the full refresh entry point, and
//! the error fallback tray.

mod core_versions;
mod helpers;
mod ids;
mod modes;
mod profiles;
mod proxies;
mod sections;
mod tun;

// Stable re-exports: `tray.rs` and `tray/handlers.rs` reach these through
// `tray::menu::...`, and `menu_test.rs` globs `crate::tray::menu::*`.
pub(crate) use core_versions::refresh_core_versions_submenu;
pub(crate) use profiles::refresh_profile_switch_submenu;
pub(crate) use proxies::refresh_proxy_groups_submenu;
pub(crate) use tun::refresh_tun_menu_item;
// Kept for path stability with the pre-split module; today only the inline
// test module consumes them (via the `crate::tray::menu::*` glob).
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use helpers::truncate_label;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use ids::{build_menu_id, insert_profile_menu_id, insert_proxy_menu_id};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use modes::is_script_enabled;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use proxies::{build_proxy_node_label, is_selectable_group};

use core_versions::build_core_versions_submenu;
use modes::build_mode_submenu;
use profiles::build_profile_switch_submenu;
use proxies::build_proxy_groups_submenu;
use sections::{build_about_submenu, build_advanced_submenu, build_sync_submenu};
use tun::build_tun_menu_item;

use std::collections::HashMap;

use log::warn;
use mihomo_version::manager::VersionManager;
use tauri::{
    AppHandle, Wry, include_image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use crate::{
    app_state::{AppState, TrayInfoItems},
    autostart::is_autostart_enabled,
    frontend::open_frontend,
    locales::{Lang, Localizer},
    platform::is_running_as_admin,
};

pub(crate) async fn refresh_tray_menu(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let (menu, items) = build_tray_menu(app, state).await?;
    if let Some(tray) = app.tray_by_id("metacube-tray") {
        tray.set_menu(Some(menu))?;
    }
    state.set_tray_info_items(items).await;
    state.refresh_system_proxy_state().await;
    state
        .update_admin_privilege_text(is_running_as_admin())
        .await;
    state.refresh_core_version_info().await;

    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());

    if let Some(url) = state.static_server_url().await {
        state
            .update_static_info_text(format!("{}: {}", lang.tr("static_server"), url))
            .await;
    } else {
        state
            .update_static_info_text(format!(
                "{}: {}",
                lang.tr("static_server"),
                lang.tr("not_started")
            ))
            .await;
    }
    if let Some(url) = state.admin_server_url().await {
        state
            .update_admin_info_text(format!("{}: {}", lang.tr("admin_server"), url))
            .await;
    } else {
        state
            .update_admin_info_text(format!(
                "{}: {}",
                lang.tr("admin_server"),
                lang.tr("not_started")
            ))
            .await;
    }
    if let Ok(runtime) = state.runtime().await {
        state
            .update_controller_info_text(format!(
                "{}: {}",
                lang.tr("controller_api"),
                runtime.controller_url
            ))
            .await;
    } else {
        state
            .update_controller_info_text(format!(
                "{}: {}",
                lang.tr("controller_api"),
                lang.tr("initializing")
            ))
            .await;
    }
    Ok(())
}

pub(crate) fn build_fallback_tray(app: &AppHandle, state: AppState) -> tauri::Result<()> {
    // Fallback tray usually happens on error, likely before we load settings or if loading settings fails.
    // We can try to get language, but synchronous here.
    // Since this is panic/error fallback, defaulting to Chinese or English hardcoded is fine.
    // But let's try to be consistent if possible. However, state.get_lang_code() is async.
    // Let's stick to Chinese default for fallback to minimize complexity, or hardcode simple English.
    // Or we can block on async if we really want, but `build_fallback_tray` is sync in signature above (tauri::Result).
    // Let's just use hardcoded Chinese as it was before, or minimal English.
    // The previous code had Chinese.
    let error_item = MenuItem::with_id(
        app,
        "tray-error",
        "托盘菜单初始化失败 / Tray Init Failed",
        false,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(
        app,
        "show",
        "打开代理页 / Open Proxy Page",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 / Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&error_item, &show_item, &quit_item])?;

    TrayIconBuilder::with_id("metacube-tray")
        .tooltip("MusicFrog Despicable Infiltrator")
        .icon(include_image!("icons/tray.ico"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |_app, event| match event.id.as_ref() {
            "show" => {
                open_frontend(state.clone());
            }
            "quit" => {
                std::process::exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub(crate) async fn build_tray_menu(
    app: &AppHandle,
    state: &AppState,
) -> tauri::Result<(Menu<Wry>, TrayInfoItems)> {
    let mut profile_map: HashMap<String, String> = HashMap::new();
    let mut proxy_map: HashMap<String, (String, String)> = HashMap::new();

    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());

    let open_webui_checked = state.open_webui_on_startup().await;
    let admin_ready = state.admin_server_url().await.is_some();
    let core_ready = state.runtime().await.is_ok();
    let versions = match VersionManager::new() {
        Ok(vm) => match vm.list_installed().await {
            Ok(list) => list,
            Err(err) => {
                warn!("failed to read installed versions: {err}");
                Vec::new()
            }
        },
        Err(err) => {
            warn!("failed to read installed versions: {err}");
            Vec::new()
        }
    };
    let core_default_checked = state.use_bundled_core().await || versions.is_empty();

    let about_submenu = build_about_submenu(app, state, &lang).await?;
    let (mode_submenu, mode_items) = build_mode_submenu(app, state, &lang).await?;
    let profile_switch_submenu = build_profile_switch_submenu(app, &mut profile_map, &lang).await?;
    let proxy_groups_submenu =
        build_proxy_groups_submenu(app, state, &mut proxy_map, &lang).await?;
    let tun_item = build_tun_menu_item(app, state, &lang).await?;

    // Group 1: Connection Info
    let static_info_item = MenuItem::with_id(
        app,
        "static-info",
        format!("{}: {}", lang.tr("static_server"), lang.tr("starting")),
        false,
        None::<&str>,
    )?;
    let controller_info_item = MenuItem::with_id(
        app,
        "controller-info",
        format!("{}: {}", lang.tr("controller_api"), lang.tr("initializing")),
        false,
        None::<&str>,
    )?;
    let admin_info_item = MenuItem::with_id(
        app,
        "admin-info",
        format!("{}: {}", lang.tr("admin_server"), lang.tr("starting")),
        false,
        None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    // Group 3: Privilege & Restart
    let admin_privilege_item = MenuItem::with_id(
        app,
        "admin-privilege",
        format!("{}: {}", lang.tr("admin_privilege"), lang.tr("checking")),
        false,
        None::<&str>,
    )?;
    let restart_admin_item = MenuItem::with_id(
        app,
        "restart-admin",
        lang.tr("restart_admin"),
        true,
        None::<&str>,
    )?;
    let factory_reset_item = MenuItem::with_id(
        app,
        "factory-reset",
        lang.tr("factory_reset"),
        true,
        None::<&str>,
    )?;

    // Group 2: Pages
    let show_item = MenuItem::with_id(app, "show", lang.tr("open_browser"), true, None::<&str>)?;
    let open_config_item = MenuItem::with_id(
        app,
        "config-open-manager",
        lang.tr("open_config_manager"),
        admin_ready,
        None::<&str>,
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    // Group 5: Settings & Sync
    let autostart_enabled = is_autostart_enabled(crate::AUTOSTART_REG_NAME);
    let autostart_supported = cfg!(target_os = "windows");
    let autostart_is_admin = is_running_as_admin();
    let autostart_label = if autostart_supported && !autostart_is_admin {
        lang.tr("autostart_admin_required")
    } else {
        lang.tr("autostart")
    };
    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        autostart_label,
        autostart_supported && autostart_is_admin,
        autostart_enabled,
        None::<&str>,
    )?;
    let open_webui_item = CheckMenuItem::with_id(
        app,
        "open-webui",
        lang.tr("open_webui_startup"),
        true,
        open_webui_checked,
        None::<&str>,
    )?;

    // Core submenu
    let core_version_item = MenuItem::with_id(
        app,
        "core-version",
        format!("{}: {}", lang.tr("current_core"), lang.tr("reading")),
        false,
        None::<&str>,
    )?;
    let core_installed_item = MenuItem::with_id(
        app,
        "core-installed",
        format!("{}: {}", lang.tr("downloaded_version"), lang.tr("reading")),
        false,
        None::<&str>,
    )?;
    let core_status_item = MenuItem::with_id(
        app,
        "core-status",
        format!("{}: {}", lang.tr("update_status"), lang.tr("reading")),
        false,
        None::<&str>,
    )?;
    let core_network_item = MenuItem::with_id(
        app,
        "core-network",
        format!("{}: {}", lang.tr("network_check"), lang.tr("reading")),
        false,
        None::<&str>,
    )?;
    let core_update_item = MenuItem::with_id(
        app,
        "core-update",
        lang.tr("update_to_stable"),
        true,
        None::<&str>,
    )?;
    let core_default_item = CheckMenuItem::with_id(
        app,
        "core-default",
        lang.tr("default_core"),
        true,
        core_default_checked,
        None::<&str>,
    )?;
    let core_versions_submenu = build_core_versions_submenu(app, &lang, &versions)?;

    let core_submenu = Submenu::with_items(
        app,
        lang.tr("core_manager"),
        true,
        &[
            &core_version_item,
            &core_installed_item,
            &core_status_item,
            &core_network_item,
            &core_default_item,
            &core_versions_submenu,
            &core_update_item,
        ],
    )?;

    let settings_submenu = Submenu::with_items(
        app,
        lang.tr("settings"),
        true,
        &[&autostart_item, &open_webui_item, &tun_item],
    )?;
    let sync_submenu = build_sync_submenu(app, state, &lang).await?;
    let advanced_submenu = build_advanced_submenu(app, &lang, admin_ready, core_ready)?;
    let proxy_item = MenuItem::with_id(
        app,
        "system-proxy",
        format!("{}: {}", lang.tr("system_proxy"), lang.tr("disabled")),
        true,
        None::<&str>,
    )?;

    // Group 4: Core Manager
    let sep3 = PredefinedMenuItem::separator(app)?;

    // Group 5: Settings & Sync
    let sep4 = PredefinedMenuItem::separator(app)?;

    // Group 6: Advanced Settings
    let sep5 = PredefinedMenuItem::separator(app)?;

    // Group 7: Runtime Control
    let sep6 = PredefinedMenuItem::separator(app)?;

    // Group 3: Privilege & Restart (after Group 7)
    let sep7 = PredefinedMenuItem::separator(app)?;

    // Group 8: About & Quit
    let quit_item = MenuItem::with_id(app, "quit", lang.tr("quit"), true, None::<&str>)?;

    let is_admin = is_running_as_admin();
    if let Err(err) = restart_admin_item.set_enabled(!is_admin) {
        warn!("failed to update restart admin menu item: {err}");
    }

    let menu = Menu::with_items(
        app,
        &[
            // Group 1
            &static_info_item,
            &controller_info_item,
            &admin_info_item,
            &sep1,
            // Group 2
            &show_item,
            &open_config_item,
            &sep2,
            // Group 4
            &core_submenu,
            &sep3,
            // Group 5
            &settings_submenu,
            &sync_submenu,
            &proxy_item,
            &sep4,
            // Group 6
            &advanced_submenu,
            &sep5,
            // Group 7
            &mode_submenu,
            &profile_switch_submenu,
            &proxy_groups_submenu,
            &sep6,
            // Group 3 (after Group 7)
            &admin_privilege_item,
            &restart_admin_item,
            &factory_reset_item,
            &sep7,
            // Group 8
            &about_submenu,
            &quit_item,
        ],
    )?;

    let items = TrayInfoItems {
        controller: controller_info_item.clone(),
        static_host: static_info_item.clone(),
        admin_host: admin_info_item.clone(),
        system_proxy: proxy_item.clone(),
        admin_privilege: admin_privilege_item.clone(),
        core_version: core_version_item.clone(),
        core_installed: core_installed_item.clone(),
        core_status: core_status_item.clone(),
        core_network: core_network_item.clone(),
        core_update: core_update_item.clone(),
        core_default: core_default_item.clone(),
        core_versions: core_versions_submenu.clone(),
        tun_mode: tun_item.clone(),
        mode_rule: mode_items.rule.clone(),
        mode_global: mode_items.global.clone(),
        mode_direct: mode_items.direct.clone(),
        mode_script: mode_items.script.clone(),
        profile_switch: profile_switch_submenu.clone(),
        proxy_groups: proxy_groups_submenu.clone(),
        autostart: autostart_item.clone(),
        open_webui: open_webui_item.clone(),
    };
    state.set_tray_profile_map(profile_map).await;
    state.set_tray_proxy_map(proxy_map).await;
    Ok((menu, items))
}

#[cfg(test)]
#[path = "menu_test.rs"]
mod menu_test;
