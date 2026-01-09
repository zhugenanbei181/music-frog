use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use despicable_infiltrator_core::profiles as core_profiles;
use log::warn;
use mihomo_rs::core::ProxyInfo;
use mihomo_rs::version::VersionManager;
use tauri::{
    include_image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder},
    AppHandle, Wry,
};

use crate::{
    app_state::{AppState, TrayInfoItems},
    autostart::is_autostart_enabled,
    platform::{is_running_as_admin},
    frontend::open_frontend,
};

// Handlers are imported in handlers.rs, menu.rs only builds UI and updates it.
// Wait, handlers logic was moved to handlers.rs. 
// But menu.rs logic below includes handle_* calls? No, I see imports for handler logic?
// Ah, `handle_system_proxy_toggle` etc were copied here in previous full overwrite? 
// NO. The full overwrite of `tray.rs` (which I deleted) had them.
// The new `tray/menu.rs` I created in Step 3 ONLY had build logic?
// Let's check what I wrote to `tray/menu.rs`.
// I wrote "build_tray_menu" and helpers.
// But wait, the `handlers.rs` I wrote in Step 2 imports `refresh_tray_menu` from `menu.rs`.
// And `menu.rs` imports handlers? No, it shouldn't.
// Let's re-read the file I wrote to `src-tauri/src/tray/menu.rs`.

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
    if let Some(url) = state.static_server_url().await {
        state
            .update_static_info_text(format!("静态站点: {url}"))
            .await;
    } else {
        state.update_static_info_text("静态站点: 未启动").await;
    }
    if let Some(url) = state.admin_server_url().await {
        state.update_admin_info_text(format!("配置管理: {url}")).await;
    } else {
        state.update_admin_info_text("配置管理: 未启动").await;
    }
    if let Ok(runtime) = state.runtime().await {
        state
            .update_controller_info_text(format!("控制接口: {}", runtime.controller_url))
            .await;
    } else {
        state.update_controller_info_text("控制接口: 未初始化").await;
    }
    Ok(())
}

pub(crate) fn build_fallback_tray(app: &AppHandle, state: AppState) -> tauri::Result<()> {
    let error_item = MenuItem::with_id(
        app,
        "tray-error",
        "托盘菜单初始化失败",
        false,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(app, "show", "打开浏览器", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&error_item, &show_item, &quit_item])?;

        TrayIconBuilder::with_id("metacube-tray")

            .tooltip("Mihomo Despicable Infiltrator")

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

    let open_webui_checked = state.open_webui_on_startup().await;
    let versions = match VersionManager::new() {
        Ok(vm) => vm.list_installed().await.unwrap_or_default(),
        Err(err) => {
            warn!("failed to read installed versions: {err}");
            Vec::new()
        }
    };
    let core_default_checked = state.use_bundled_core().await || versions.is_empty();

    let about_submenu = build_about_submenu(app, state).await?;
    let mode_submenu = build_mode_submenu(app, state).await?;
    let profile_switch_submenu = build_profile_switch_submenu(app, &mut profile_map).await?;
    let proxy_groups_submenu = build_proxy_groups_submenu(app, state, &mut proxy_map).await?;
    let tun_item = build_tun_menu_item(app, state).await?;

    // Group 1: Connection Info
    let static_info_item = MenuItem::with_id(
        app, "static-info", "静态站点: 启动中...", false, None::<&str>,
    )?;
    let controller_info_item = MenuItem::with_id(
        app, "controller-info", "控制接口: 初始化中", false, None::<&str>,
    )?;
    let admin_info_item = MenuItem::with_id(
        app, "admin-info", "配置管理: 启动中...", false, None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    // Group 2: Privilege & Restart
    let admin_privilege_item = MenuItem::with_id(
        app, "admin-privilege", "管理员权限: 检测中...", false, None::<&str>,
    )?;
    let restart_admin_item =
        MenuItem::with_id(app, "restart-admin", "以管理员身份重启", true, None::<&str>)?;
    let factory_reset_item =
        MenuItem::with_id(app, "factory-reset", "恢复出厂设置", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    // Group 3: Settings & UI
    let autostart_enabled = is_autostart_enabled();
    let autostart_supported = cfg!(target_os = "windows");
    let autostart_is_admin = is_running_as_admin();
    let autostart_label = if autostart_supported && !autostart_is_admin {
        "开机自启（需管理员）"
    } else {
        "开机自启"
    };
    let autostart_item = CheckMenuItem::with_id(
        app, "autostart", autostart_label, autostart_supported && autostart_is_admin, autostart_enabled, None::<&str>,
    )?;
    let open_webui_item = CheckMenuItem::with_id(
        app, "open-webui", "启动时打开 Web UI", true, open_webui_checked, None::<&str>,
    )?;
    
    // Core submenu
    let core_version_item = MenuItem::with_id(app, "core-version", "当前内核: 读取中...", false, None::<&str>)?;
    let core_installed_item = MenuItem::with_id(app, "core-installed", "已下载版本: 读取中...", false, None::<&str>)?;
    let core_status_item = MenuItem::with_id(app, "core-status", "更新状态: 读取中...", false, None::<&str>)?;
    let core_network_item = MenuItem::with_id(app, "core-network", "网络: 读取中...", false, None::<&str>)?;
    let core_update_item = MenuItem::with_id(app, "core-update", "更新到最新 Stable", true, None::<&str>)?;
    let core_default_item = CheckMenuItem::with_id(app, "core-default", "默认内核", true, core_default_checked, None::<&str>)?;
    
            let mut version_submenus: Vec<Submenu<Wry>> = Vec::new();
            for version in versions {
                let use_item = MenuItem::with_id(app, format!("core-use-{}", version.version), "启用", true, None::<&str>)?;
                let delete_item = MenuItem::with_id(app, format!("core-delete-{}", version.version), "删除", true, None::<&str>)?;
                let submenu = Submenu::with_items(app, &version.version, true, &[&use_item, &delete_item])?;
                version_submenus.push(submenu);
            }
            let empty_versions_item = MenuItem::with_id(app, "core-empty", "暂无已下载版本", false, None::<&str>)?;
            
            let core_versions_submenu = {
                let mut version_items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
                if version_submenus.is_empty() {
                    version_items.push(&empty_versions_item);
                } else {
                    for submenu in &version_submenus {
                        version_items.push(submenu);
                    }
                }
                Submenu::with_items(app, "已下载版本", true, version_items.as_slice())?
            };
    
            let core_submenu = Submenu::with_items(        app, "内核管理", true,
        &[&core_version_item, &core_installed_item, &core_status_item, &core_network_item, &core_default_item, &core_versions_submenu, &core_update_item],
    )?;

    let settings_submenu = Submenu::with_items(
        app, "设置", true,
        &[&autostart_item, &open_webui_item, &tun_item],
    )?;
    let proxy_item = MenuItem::with_id(app, "system-proxy", "系统代理: 已关闭", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "打开浏览器", true, None::<&str>)?;
    let config_item = MenuItem::with_id(app, "config-manager", "打开配置管理", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    // Group 4: Runtime Control
    let sep4 = PredefinedMenuItem::separator(app)?;

    // Group 5: About & Quit
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

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
            &admin_privilege_item, &restart_admin_item, &factory_reset_item, &sep2,
            // Group 3
            &core_submenu, &settings_submenu, &proxy_item, &show_item, &config_item, &sep3,
            // Group 4
            &mode_submenu, &profile_switch_submenu, &proxy_groups_submenu, &sep4,
            // Group 5
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
        autostart: autostart_item.clone(),
        open_webui: open_webui_item.clone(),
    };
    state.set_tray_profile_map(profile_map).await;
    state.set_tray_proxy_map(proxy_map).await;
    Ok((menu, items))
}

async fn build_about_submenu(app: &AppHandle, state: &AppState) -> tauri::Result<Submenu<Wry>> {
    let app_version = format!("Mihomo-Despicable-Infiltrator v{}", env!("CARGO_PKG_VERSION"));
    let sdk_version = "mihomo-rs v1.2.2";
    
    let core_version = if let Ok(runtime) = state.runtime().await {
        match runtime.client().get_version().await {
            Ok(v) => format!("core-service {}", v.version),
            Err(_) => "core-service (未知)".to_string(),
        }
    } else {
        "core-service (未启动)".to_string()
    };

    let app_item = MenuItem::with_id(app, "about-app", &app_version, false, None::<&str>)?;
    let sdk_item = MenuItem::with_id(app, "about-sdk", sdk_version, false, None::<&str>)?;
    let core_item = MenuItem::with_id(app, "about-core", &core_version, false, None::<&str>)?;

    Submenu::with_items(app, "关于", true, &[&app_item, &sdk_item, &core_item])
}

async fn build_profile_switch_submenu(
    app: &AppHandle,
    profile_map: &mut HashMap<String, String>,
) -> tauri::Result<Submenu<Wry>> {
    let mut profiles = match core_profiles::list_profile_infos().await {
        Ok(list) => list,
        Err(err) => {
            warn!("failed to list profiles: {err:#}");
            let failed_item =
                MenuItem::with_id(app, "profile-switch-error", "配置读取失败", false, None::<&str>)?;
            return Submenu::with_items(app, "配置切换", true, &[&failed_item]);
        }
    };
    
    let active_profile = profiles.iter().find(|p| p.active).cloned();

    if profiles.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "profile-switch-empty", "暂无配置", false, None::<&str>)?;
        return Submenu::with_items(app, "配置切换", true, &[&empty_item]);
    }

    profiles.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
    let has_subscription = profiles.iter().any(|profile| profile.subscription_url.is_some());

    let max_visible = 10usize;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    
    for profile in profiles.iter().take(max_visible) {
        let label = if profile.subscription_url.is_some() {
            format!("{} (订阅)", profile.name)
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
            Submenu::with_items(app, "更多配置", true, overflow_refs.as_slice())?;
        items.push(Box::new(overflow_submenu));
    }

    items.push(Box::new(PredefinedMenuItem::separator(app)?));

    let update_all_item = MenuItem::with_id(
        app,
        "profile-update-all",
        "立即更新所有订阅",
        has_subscription,
        None::<&str>,
    )?;
    items.push(Box::new(update_all_item));

    if let Some(active) = active_profile {
        if active.subscription_url.is_some() {
            let auto_update_item = CheckMenuItem::with_id(
                app,
                format!("profile-auto-update-{}", active.name),
                "自动更新当前订阅",
                true,
                active.auto_update_enabled,
                None::<&str>,
            )?;
            items.push(Box::new(auto_update_item));
        }
    }

    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Submenu::with_items(app, "配置切换", true, &item_refs)
}

async fn build_mode_submenu(app: &AppHandle, state: &AppState) -> tauri::Result<Submenu<Wry>> {
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
        CheckMenuItem::with_id(app, "mode-rule", "规则模式", menu_enabled, is_rule, None::<&str>)?;
    let global_item = CheckMenuItem::with_id(
        app,
        "mode-global",
        "全局代理",
        menu_enabled,
        is_global,
        None::<&str>,
    )?;
    let direct_item = CheckMenuItem::with_id(
        app,
        "mode-direct",
        "直连模式",
        menu_enabled,
        is_direct,
        None::<&str>,
    )?;
    let script_label = if script_enabled {
        "脚本模式"
    } else {
        "脚本模式（未启用）"
    };
    let script_item = CheckMenuItem::with_id(
        app,
        "mode-script",
        script_label,
        menu_enabled && script_enabled,
        is_script,
        None::<&str>,
    )?;

    Submenu::with_items(
        app,
        "代理模式",
        true,
        &[&rule_item, &global_item, &direct_item, &script_item],
    )
}

async fn build_proxy_groups_submenu(
    app: &AppHandle,
    state: &AppState,
    proxy_map: &mut HashMap<String, (String, String)>,
) -> tauri::Result<Submenu<Wry>> {
    let proxies = match state.refresh_proxy_groups().await {
        Ok(proxies) => proxies,
        Err(err) => {
            warn!("failed to refresh proxies: {err:#}");
            let failed_item =
                MenuItem::with_id(app, "proxy-groups-error", "代理组读取失败", false, None::<&str>)?;
            return Submenu::with_items(app, "代理组", true, &[&failed_item]);
        }
    };

    let mut groups: Vec<(String, ProxyInfo)> = proxies
        .iter()
        .filter(|(_, info)| is_selectable_group(info))
        .map(|(name, info)| (name.clone(), info.clone()))
        .collect();
    if groups.is_empty() {
        let empty_item =
            MenuItem::with_id(app, "proxy-groups-empty", "暂无可选代理组", false, None::<&str>)?;
        return Submenu::with_items(app, "代理组", true, &[&empty_item]);
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    let max_groups = 5usize;
    let mut group_submenus: Vec<Submenu<Wry>> = Vec::new();
    for (name, info) in groups.iter().take(max_groups) {
        let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map)?;
        group_submenus.push(submenu);
    }

    if groups.len() > max_groups {
        let mut overflow_submenus: Vec<Submenu<Wry>> = Vec::new();
        let mut overflow_items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
        for (name, info) in groups.iter().skip(max_groups) {
            let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map)?;
            overflow_submenus.push(submenu);
        }
        for submenu in &overflow_submenus {
            overflow_items.push(submenu);
        }
        let overflow_submenu =
            Submenu::with_items(app, "更多代理组", true, overflow_items.as_slice())?;
        group_submenus.push(overflow_submenu);
    }

    let items: Vec<&dyn IsMenuItem<Wry>> = group_submenus
        .iter()
        .map(|submenu| submenu as &dyn IsMenuItem<Wry>)
        .collect();

    Submenu::with_items(app, "代理组", true, items.as_slice())
}

fn build_proxy_group_submenu(
    app: &AppHandle,
    proxies: &std::collections::HashMap<String, ProxyInfo>,
    group_name: &str,
    group_info: &ProxyInfo,
    proxy_map: &mut HashMap<String, (String, String)>,
) -> tauri::Result<Submenu<Wry>> {
    let nodes = group_info.all.clone().unwrap_or_default();
    if nodes.is_empty() {
        let empty_item = MenuItem::with_id(
            app,
            format!("proxy-empty-{group_name}"),
            "暂无节点",
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
            Submenu::with_items(app, "更多节点", true, overflow_refs.as_slice())?;
        overflow_submenus.push(overflow_submenu);
        if let Some(submenu) = overflow_submenus.last() {
            item_refs.push(submenu);
        }
    }

    Submenu::with_items(app, group_name, true, item_refs.as_slice())
}

fn build_proxy_node_label(
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

fn is_selectable_group(info: &ProxyInfo) -> bool {
    matches!(
        info.proxy_type.as_str(),
        "Selector" | "URLTest" | "Fallback" | "LoadBalance"
    )
}

async fn build_tun_menu_item(
    app: &AppHandle,
    state: &AppState,
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
        "TUN 模式（需管理员）"
    } else if !available {
        "TUN 模式（配置未启用）"
    } else {
        "TUN 模式"
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

fn is_script_enabled(script: Option<&serde_json::Value>) -> bool {
    match script {
        Some(value) => value
            .get("enable")
            .and_then(|v: &serde_json::Value| v.as_bool())
            .unwrap_or(true),
        None => false,
    }
}

fn build_menu_id(prefix: &str, key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{prefix}-{hash:016x}")
}

fn insert_profile_menu_id(
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

fn insert_proxy_menu_id(
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

fn truncate_label(value: &str, max_chars: usize) -> String {
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