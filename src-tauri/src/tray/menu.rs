use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use infiltrator_core::profiles as core_profiles;
use log::warn;
use mihomo_api::ProxyInfo;
use mihomo_version::{manager::VersionInfo, VersionManager};
use tauri::{
    include_image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder},
    AppHandle, Wry,
};
use tokio::time::Duration;

use crate::{
    app_state::{AppState, TrayInfoItems},
    autostart::is_autostart_enabled,
    platform::{is_running_as_admin},
    frontend::open_frontend,
    locales::{Lang, Localizer},
};

pub(crate) async fn refresh_tray_menu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let (menu, items) = build_tray_menu(app, state).await?;
    if let Some(tray) = app.tray_by_id("metacube-tray") {
        tray.set_menu(Some(menu))?;
    }
    state.set_tray_info_items(items).await;
    state.refresh_system_proxy_state().await;
    state.update_admin_privilege_text(is_running_as_admin()).await;
    state.refresh_core_version_info().await;
    
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());

    if let Some(url) = state.static_server_url().await {
        state
            .update_static_info_text(format!("{}: {}", lang.tr("static_server"), url))
            .await;
    } else {
        state.update_static_info_text(format!("{}: {}", lang.tr("static_server"), lang.tr("not_started"))).await;
    }
    if let Some(url) = state.admin_server_url().await {
        state.update_admin_info_text(format!("{}: {}", lang.tr("admin_server"), url)).await;
    } else {
        state.update_admin_info_text(format!("{}: {}", lang.tr("admin_server"), lang.tr("not_started"))).await;
    }
    if let Ok(runtime) = state.runtime().await {
        state
            .update_controller_info_text(format!("{}: {}", lang.tr("controller_api"), runtime.controller_url))
            .await;
    } else {
        state.update_controller_info_text(format!("{}: {}", lang.tr("controller_api"), lang.tr("initializing"))).await;
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
    let show_item = MenuItem::with_id(app, "show", "打开代理页 / Open Proxy Page", true, None::<&str>)?;
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
    let proxy_groups_submenu = build_proxy_groups_submenu(app, state, &mut proxy_map, &lang).await?;
    let tun_item = build_tun_menu_item(app, state, &lang).await?;

    // Group 1: Connection Info
    let static_info_item = MenuItem::with_id(
        app, "static-info", format!("{}: {}", lang.tr("static_server"), lang.tr("starting")),
    false, None::<&str>)?;
    let controller_info_item = MenuItem::with_id(
        app, "controller-info", format!("{}: {}", lang.tr("controller_api"), lang.tr("initializing")),
    false, None::<&str>)?;
    let admin_info_item = MenuItem::with_id(
        app, "admin-info", format!("{}: {}", lang.tr("admin_server"), lang.tr("starting")),
    false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    // Group 3: Privilege & Restart
    let admin_privilege_item = MenuItem::with_id(
        app, "admin-privilege", format!("{}: {}", lang.tr("admin_privilege"), lang.tr("checking")),
    false, None::<&str>)?;
    let restart_admin_item =
        MenuItem::with_id(app, "restart-admin", lang.tr("restart_admin"), true, None::<&str>)?;
    let factory_reset_item =
        MenuItem::with_id(app, "factory-reset", lang.tr("factory_reset"), true, None::<&str>)?;

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
    let autostart_enabled = is_autostart_enabled();
    let autostart_supported = cfg!(target_os = "windows");
    let autostart_is_admin = is_running_as_admin();
    let autostart_label = if autostart_supported && !autostart_is_admin {
        lang.tr("autostart_admin_required")
    } else {
        lang.tr("autostart")
    };
    let autostart_item = CheckMenuItem::with_id(
        app, "autostart", autostart_label, autostart_supported && autostart_is_admin, autostart_enabled, None::<&str>,
    )?;
    let open_webui_item = CheckMenuItem::with_id(
        app, "open-webui", lang.tr("open_webui_startup"), true, open_webui_checked, None::<&str>,
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
    let core_update_item =
        MenuItem::with_id(app, "core-update", lang.tr("update_to_stable"), true, None::<&str>)?;
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
        app, lang.tr("settings"), true,
        &[&autostart_item, &open_webui_item, &tun_item],
    )?;
    let sync_submenu = build_sync_submenu(app, state, &lang).await?;
    let advanced_submenu = build_advanced_submenu(app, &lang, admin_ready, core_ready)?;
    let proxy_item = MenuItem::with_id(app, "system-proxy", format!("{}: {}", lang.tr("system_proxy"), lang.tr("disabled")),
    true, None::<&str>)?;

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
            &static_info_item, &controller_info_item, &admin_info_item, &sep1,
            // Group 2
            &show_item, &open_config_item, &sep2,
            // Group 4
            &core_submenu, &sep3,
            // Group 5
            &settings_submenu, &sync_submenu, &proxy_item, &sep4,
            // Group 6
            &advanced_submenu, &sep5,
            // Group 7
            &mode_submenu, &profile_switch_submenu, &proxy_groups_submenu, &sep6,
            // Group 3 (after Group 7)
            &admin_privilege_item, &restart_admin_item, &factory_reset_item, &sep7,
            // Group 8
            &about_submenu, &quit_item,
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

async fn build_about_submenu(app: &AppHandle, state: &AppState, lang: &Lang<'_>) -> tauri::Result<Submenu<Wry>> {
    let app_version = format!("MusicFrog-Despicable-Infiltrator v{}", env!("CARGO_PKG_VERSION"));
    let sdk_version = "mihomo-sdk (workspace)";
    
    let core_version = if let Ok(runtime) = state.runtime().await {
        match runtime.client().get_version().await {
            Ok(v) => format!("{} {}", lang.tr("core_service"), v.version),
            Err(_) => format!("{} ({})", lang.tr("core_service"), lang.tr("unknown")),
        }
    } else {
        format!("{} ({})", lang.tr("core_service"), lang.tr("not_started"))
    };

    let app_item = MenuItem::with_id(app, "about-app", &app_version, false, None::<&str>)?;
    let sdk_item = MenuItem::with_id(app, "about-sdk", sdk_version, false, None::<&str>)?;
    let core_item = MenuItem::with_id(app, "about-core", &core_version, false, None::<&str>)?;

    Submenu::with_items(app, lang.tr("about"), true, &[&app_item, &sdk_item, &core_item])
}

async fn build_profile_switch_submenu(
    app: &AppHandle,
    profile_map: &mut HashMap<String, String>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let items = build_profile_switch_items(app, profile_map, lang).await?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Submenu::with_items(app, lang.tr("profile_switch"), true, &item_refs)
}

async fn build_profile_switch_items(
    app: &AppHandle,
    profile_map: &mut HashMap<String, String>,
    lang: &Lang<'_>,
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    let mut profiles = match core_profiles::list_profile_infos().await {
        Ok(list) => list,
        Err(err) => {
            warn!("failed to list profiles: {err:#}");
            let failed_item =
                MenuItem::with_id(app, "profile-switch-error", lang.tr("profile_read_failed"), false, None::<&str>)?;
            return Ok(vec![Box::new(failed_item)]);
        }
    };

    let active_profile = profiles.iter().find(|p| p.active).cloned();

    if profiles.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "profile-switch-empty", lang.tr("profile_empty"), false, None::<&str>)?;
        return Ok(vec![Box::new(empty_item)]);
    }

    profiles.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
    let has_subscription = profiles.iter().any(|profile| profile.subscription_url.is_some());

    let max_visible = 10usize;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();

    for profile in profiles.iter().take(max_visible) {
        let label = if profile.subscription_url.is_some() {
            format!("{} ({})", profile.name, lang.tr("subscription"))
        } else {
            profile.name.clone()
        };
        let label = truncate_label(&label, 60);
        let menu_id = insert_profile_menu_id(profile_map, &profile.name);
        let item = CheckMenuItem::with_id(
            app,
            menu_id,
            label,
            true,
            profile.active,
            None::<&str>,
        )?;
        items.push(Box::new(item));
    }

    if profiles.len() > max_visible {
        let mut overflow_items: Vec<CheckMenuItem<Wry>> = Vec::new();
        for profile in profiles.iter().skip(max_visible) {
            let menu_id = insert_profile_menu_id(profile_map, &profile.name);
            let item = CheckMenuItem::with_id(
                app,
                menu_id,
                truncate_label(&profile.name, 60),
                true,
                profile.active,
                None::<&str>,
            )?;
            overflow_items.push(item);
        }
        let overflow_refs: Vec<&dyn IsMenuItem<Wry>> = overflow_items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect();
        let overflow_submenu =
            Submenu::with_items(app, lang.tr("more_profiles"), true, overflow_refs.as_slice())?;
        items.push(Box::new(overflow_submenu));
    }

    items.push(Box::new(PredefinedMenuItem::separator(app)?));

    let update_all_item = MenuItem::with_id(
        app,
        "profile-update-all",
        lang.tr("update_all_subs"),
        has_subscription,
        None::<&str>,
    )?;
    items.push(Box::new(update_all_item));

    if let Some(active) = active_profile
        && active.subscription_url.is_some() {
            let auto_update_item = CheckMenuItem::with_id(
                app,
                format!("profile-auto-update-{}", active.name),
                lang.tr("auto_update_sub"),
                true,
                active.auto_update_enabled,
                None::<&str>,
            )?;
            items.push(Box::new(auto_update_item));
        }

    Ok(items)
}

struct ModeMenuItems {
    rule: CheckMenuItem<Wry>,
    global: CheckMenuItem<Wry>,
    direct: CheckMenuItem<Wry>,
    script: CheckMenuItem<Wry>,
}

async fn build_mode_submenu(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<(Submenu<Wry>, ModeMenuItems)> {
    let mut current_mode: Option<String> = None;
    let mut script_enabled = false;
    let mut menu_enabled = false;
    if let Ok(runtime) = state.runtime().await {
        match runtime.client().get_config().await {
            Ok(config) => {
                menu_enabled = true;
                let mode = config.mode.trim().to_ascii_lowercase();
                if !mode.is_empty() {
                    current_mode = Some(mode);
                }
                script_enabled = is_script_enabled(config.script.as_ref());
            }
            Err(err) => {
                warn!("failed to read config for mode: {err:#}");
            }
        }
    }
    state.set_current_mode(current_mode.clone()).await;

    let is_rule = current_mode.as_deref() == Some("rule");
    let is_global = current_mode.as_deref() == Some("global");
    let is_direct = current_mode.as_deref() == Some("direct");
    let is_script = current_mode.as_deref() == Some("script");

    let rule_item =
        CheckMenuItem::with_id(app, "mode-rule", lang.tr("mode_rule"), menu_enabled, is_rule, None::<&str>)?;
    let global_item = CheckMenuItem::with_id(
        app,
        "mode-global",
        lang.tr("mode_global"),
        menu_enabled,
        is_global,
        None::<&str>,
    )?;
    let direct_item = CheckMenuItem::with_id(
        app,
        "mode-direct",
        lang.tr("mode_direct"),
        menu_enabled,
        is_direct,
        None::<&str>,
    )?;
    let script_label = if script_enabled {
        lang.tr("mode_script").into_owned()
    } else {
        format!("{} ({})", lang.tr("mode_script"), lang.tr("disabled"))
    };
    let script_item = CheckMenuItem::with_id(
        app,
        "mode-script",
        script_label,
        menu_enabled && script_enabled,
        is_script,
        None::<&str>,
    )?;

    let submenu = Submenu::with_items(
        app,
        lang.tr("proxy_mode"),
        true,
        &[&rule_item, &global_item, &direct_item, &script_item],
    )?;

    Ok((
        submenu,
        ModeMenuItems {
            rule: rule_item,
            global: global_item,
            direct: direct_item,
            script: script_item,
        },
    ))
}

fn build_core_versions_submenu(
    app: &AppHandle,
    lang: &Lang<'_>,
    versions: &[VersionInfo],
) -> tauri::Result<Submenu<Wry>> {
    let items = build_core_versions_items(app, lang, versions)?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Submenu::with_items(app, lang.tr("downloaded_version"), true, item_refs.as_slice())
}

fn build_core_versions_items(
    app: &AppHandle,
    lang: &Lang<'_>,
    versions: &[VersionInfo],
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    if versions.is_empty() {
        let empty_versions_item =
            MenuItem::with_id(app, "core-empty", lang.tr("empty"), false, None::<&str>)?;
        return Ok(vec![Box::new(empty_versions_item)]);
    }

    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for version in versions {
        let use_item =
            MenuItem::with_id(app, format!("core-use-{}", version.version), lang.tr("use"), true, None::<&str>)?;
        let delete_item =
            MenuItem::with_id(app, format!("core-delete-{}", version.version), lang.tr("delete"), true, None::<&str>)?;
        let submenu = Submenu::with_items(app, &version.version, true, &[&use_item, &delete_item])?;
        items.push(Box::new(submenu));
    }

    Ok(items)
}

async fn build_proxy_groups_submenu(
    app: &AppHandle,
    state: &AppState,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let items = build_proxy_groups_items(app, state, proxy_map, lang).await?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Submenu::with_items(app, lang.tr("proxy_groups"), true, item_refs.as_slice())
}

async fn build_proxy_groups_items(
    app: &AppHandle,
    state: &AppState,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    let proxies = match state.refresh_proxy_groups().await {
        Ok(proxies) => proxies,
        Err(err) => {
            warn!("failed to refresh proxies: {err:#}");
            let failed_item =
                MenuItem::with_id(app, "proxy-groups-error", lang.tr("proxy_groups_read_failed"), false, None::<&str>)?;
            return Ok(vec![Box::new(failed_item)]);
        }
    };

    let mut groups: Vec<(String, ProxyInfo)> = proxies
        .iter()
        .filter(|(_, info)| is_selectable_group(info))
        .map(|(name, info)| (name.clone(), info.clone()))
        .collect();
    if groups.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "proxy-groups-empty", lang.tr("proxy_groups_empty"), false, None::<&str>)?;
        return Ok(vec![Box::new(empty_item)]);
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    let max_groups = 5usize;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for (name, info) in groups.iter().take(max_groups) {
        let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map, lang)?;
        items.push(Box::new(submenu));
    }

    if groups.len() > max_groups {
        let mut overflow_submenus: Vec<Submenu<Wry>> = Vec::new();
        let mut overflow_items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
        for (name, info) in groups.iter().skip(max_groups) {
            let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map, lang)?;
            overflow_submenus.push(submenu);
        }
        for submenu in &overflow_submenus {
            overflow_items.push(submenu);
        }
        let overflow_submenu =
            Submenu::with_items(app, lang.tr("more_groups"), true, overflow_items.as_slice())?;
        items.push(Box::new(overflow_submenu));
    }

    Ok(items)
}

fn build_proxy_group_submenu(
    app: &AppHandle,
    proxies: &std::collections::HashMap<String, ProxyInfo>,
    group_name: &str,
    group_info: &ProxyInfo,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let nodes = group_info.all.clone().unwrap_or_default();
    if nodes.is_empty() {
        let empty_item = MenuItem::with_id(
            app,
            format!("proxy-empty-{group_name}"),
            lang.tr("no_nodes"),
            false,
            None::<&str>,
        )?;
        return Submenu::with_items(app, group_name, true, &[&empty_item]);
    }

    let current = group_info.now.as_deref().unwrap_or("");
    let max_nodes = 20usize;

    let mut node_items: Vec<CheckMenuItem<Wry>> = Vec::new();
    for node in nodes.iter().take(max_nodes) {
        let label = truncate_label(&build_proxy_node_label(proxies, node), 60);
        let menu_id = insert_proxy_menu_id(proxy_map, group_name, node);
        let item = CheckMenuItem::with_id(
            app,
            menu_id,
            label,
            true,
            current == node,
            None::<&str>,
        )?;
        node_items.push(item);
    }
    let mut item_refs: Vec<&dyn IsMenuItem<Wry>> =
        node_items.iter().map(|item| item as &dyn IsMenuItem<Wry>).collect();

    let mut overflow_submenus: Vec<Submenu<Wry>> = Vec::new();
    if nodes.len() > max_nodes {
        let mut overflow_items: Vec<CheckMenuItem<Wry>> = Vec::new();
        for node in nodes.iter().skip(max_nodes) {
            let label = truncate_label(&build_proxy_node_label(proxies, node), 60);
            let menu_id = insert_proxy_menu_id(proxy_map, group_name, node);
            let item = CheckMenuItem::with_id(
                app,
                menu_id,
                label,
                true,
                current == node,
                None::<&str>,
            )?;
            overflow_items.push(item);
        }
        let overflow_refs: Vec<&dyn IsMenuItem<Wry>> = overflow_items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect();
        let overflow_submenu =
            Submenu::with_items(app, lang.tr("more_nodes"), true, overflow_refs.as_slice())?;
        overflow_submenus.push(overflow_submenu);
        if let Some(submenu) = overflow_submenus.last() {
            item_refs.push(submenu);
        }
    }

    Submenu::with_items(app, group_name, true, item_refs.as_slice())
}

pub(crate) fn build_proxy_node_label(
    proxies: &std::collections::HashMap<String, ProxyInfo>,
    node: &str,
) -> String {
    if let Some(delay) = proxies
        .get(node)
        .and_then(|info| info.history.last().map(|entry| entry.delay))
    {
        format!("{node} ({delay}ms)")
    } else {
        node.to_string()
    }
}

pub(crate) fn is_selectable_group(info: &ProxyInfo) -> bool {
    matches!(
        info.proxy_type.as_str(),
        "Selector" | "URLTest" | "Fallback" | "LoadBalance"
    )
}

async fn build_tun_menu_item(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<CheckMenuItem<Wry>> {
    let is_admin = is_running_as_admin();
    let (available, enabled) = match state.refresh_tun_state().await {
        Ok(result) => result,
        Err(err) => {
            warn!("failed to refresh tun state: {err:#}");
            (false, false)
        }
    };
    let label = if !is_admin {
        lang.tr("tun_mode_admin_required")
    } else if !available {
        lang.tr("tun_mode_disabled")
    } else {
        lang.tr("tun_mode")
    };
    CheckMenuItem::with_id(
        app,
        "tun-mode",
        label,
        is_admin && available,
        enabled,
        None::<&str>,
    )
}

pub(crate) fn is_script_enabled(script: Option<&serde_json::Value>) -> bool {
    match script {
        Some(value) => value
            .get("enable")
            .and_then(|v: &serde_json::Value| v.as_bool())
            .unwrap_or(true),
        None => false,
    }
}

pub(crate) fn build_menu_id(prefix: &str, key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{prefix}-{hash:016x}")
}

pub(crate) fn insert_profile_menu_id(
    profile_map: &mut HashMap<String, String>,
    profile_name: &str,
) -> String {
    let base_id = build_menu_id("profile-switch", profile_name);
    let mut menu_id = base_id.clone();
    let mut counter = 1u32;
    while profile_map.contains_key(&menu_id) {
        menu_id = format!("{base_id}-{counter}");
        counter = counter.saturating_add(1);
    }
    profile_map.insert(menu_id.clone(), profile_name.to_string());
    menu_id
}

pub(crate) fn insert_proxy_menu_id(
    proxy_map: &mut HashMap<String, (String, String)>,
    group_name: &str,
    node_name: &str,
) -> String {
    let base_id = build_menu_id("proxy", &format!("{group_name}\n{node_name}"));
    let mut menu_id = base_id.clone();
    let mut counter = 1u32;
    while proxy_map.contains_key(&menu_id) {
        menu_id = format!("{base_id}-{counter}");
        counter = counter.saturating_add(1);
    }
    proxy_map.insert(
        menu_id.clone(),
        (group_name.to_string(), node_name.to_string()),
    );
    menu_id
}

pub(crate) fn truncate_label(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let take_len = max_chars.saturating_sub(3);
    let mut truncated: String = chars.into_iter().take(take_len).collect();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
#[path = "menu_test.rs"]
mod menu_test;

fn build_advanced_submenu(
    app: &AppHandle,
    lang: &Lang<'_>,
    admin_ready: bool,
    core_ready: bool,
) -> tauri::Result<Submenu<Wry>> {
    let enabled = admin_ready && core_ready;
    let dns_item = MenuItem::with_id(
        app,
        "dns-open-settings",
        lang.tr("dns_settings"),
        enabled,
        None::<&str>,
    )?;
    let fake_ip_item = MenuItem::with_id(
        app,
        "fake-ip-open-settings",
        lang.tr("fake_ip_settings"),
        enabled,
        None::<&str>,
    )?;
    let fake_ip_flush_item = MenuItem::with_id(
        app,
        "fake-ip-flush",
        lang.tr("fake_ip_flush"),
        enabled,
        None::<&str>,
    )?;
    let rules_item = MenuItem::with_id(
        app,
        "rules-open-settings",
        lang.tr("rules_settings"),
        enabled,
        None::<&str>,
    )?;
    let tun_item = MenuItem::with_id(
        app,
        "tun-open-settings",
        lang.tr("tun_settings"),
        enabled,
        None::<&str>,
    )?;

    Submenu::with_items(
        app,
        lang.tr("advanced_settings"),
        true,
        &[&dns_item, &fake_ip_item, &fake_ip_flush_item, &rules_item, &tun_item],
    )
}

async fn build_sync_submenu(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let settings = state.get_app_settings().await;
    let enabled = settings.webdav.enabled;
    
    let status_label = if enabled {
        format!("{}: {}", lang.tr("webdav_sync"), lang.tr("enabled"))
    } else {
        format!("{}: {}", lang.tr("webdav_sync"), lang.tr("disabled"))
    };
    
    let status_item = MenuItem::with_id(app, "sync-status", &status_label, false, None::<&str>)?;
    let sync_now_item = MenuItem::with_id(app, "webdav-sync-now", lang.tr("sync_now"), enabled, None::<&str>)?;
    let sync_settings_item = MenuItem::with_id(app, "webdav-sync-settings", lang.tr("sync_settings"), true, None::<&str>)?;
    
    Submenu::with_items(
        app,
        lang.tr("sync_and_backup"),
        true,
        &[&status_item, &sync_now_item, &sync_settings_item],
    )
}

pub(crate) async fn refresh_profile_switch_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        warn!("tray info items not available for profile switch submenu refresh");
        return Ok(());
    };

    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
    let mut profile_map = HashMap::new();

    // Add retry logic with exponential backoff
    let max_attempts = 3;
    let mut attempt = 0;
    let mut delay = Duration::from_millis(100);

    loop {
        attempt += 1;

        let result = async {
            let menu_items = build_profile_switch_items(app, &mut profile_map, &lang).await?;
            clear_submenu_items(&items.profile_switch)?;
            append_items_to_submenu(&items.profile_switch, &menu_items)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => {
                state.set_tray_profile_map(profile_map).await;
                log::info!("profile switch submenu refreshed successfully (attempt {})", attempt);
                return Ok(());
            }
            Err(err) => {
                if attempt >= max_attempts {
                    warn!(
                        "failed to refresh profile switch submenu after {} attempts: {:#}",
                        max_attempts, err
                    );
                    return Err(err);
                }
                warn!(
                    "profile switch submenu refresh failed (attempt {}/{}), retrying in {:?}: {:#}",
                    attempt, max_attempts, delay, err
                );
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2).min(Duration::from_secs(2));
            }
        }
    }
}

pub(crate) async fn refresh_proxy_groups_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
    let mut proxy_map = HashMap::new();
    {
        let menu_items = build_proxy_groups_items(app, state, &mut proxy_map, &lang).await?;
        clear_submenu_items(&items.proxy_groups)?;
        append_items_to_submenu(&items.proxy_groups, &menu_items)?;
    }
    state.set_tray_proxy_map(proxy_map).await;
    Ok(())
}

pub(crate) async fn refresh_core_versions_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
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
    let menu_items = build_core_versions_items(app, &lang, &versions)?;
    clear_submenu_items(&items.core_versions)?;
    append_items_to_submenu(&items.core_versions, &menu_items)?;
    Ok(())
}

pub(crate) async fn refresh_tun_menu_item(
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());

    let is_admin = is_running_as_admin();
    let (available, enabled) = match state.refresh_tun_state().await {
        Ok(result) => result,
        Err(err) => {
            warn!("failed to refresh tun state: {err:#}");
            (false, false)
        }
    };

    let label = if !is_admin {
        lang.tr("tun_mode_admin_required")
    } else if !available {
        lang.tr("tun_mode_disabled")
    } else {
        lang.tr("tun_mode")
    };

    items.tun_mode.set_text(label)?;
    items.tun_mode.set_checked(enabled)?;
    items.tun_mode.set_enabled(is_admin && available)?;
    Ok(())
}

fn clear_submenu_items(submenu: &Submenu<Wry>) -> tauri::Result<()> {
    loop {
        let items = submenu.items()?;
        if items.is_empty() {
            break;
        }
        let _ = submenu.remove_at(0)?;
    }
    Ok(())
}

fn append_items_to_submenu(
    submenu: &Submenu<Wry>,
    items: &[Box<dyn IsMenuItem<Wry>>],
) -> tauri::Result<()> {
    for item in items {
        submenu.append(item.as_ref())?;
    }
    Ok(())
}
