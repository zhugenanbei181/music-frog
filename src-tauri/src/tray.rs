use anyhow::anyhow;
use log::warn;
use mihomo_rs::version::VersionManager;
use tauri::{
    include_image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Wry,
};
use tokio::time::Duration;

use crate::{
    app_state::{AppState, TrayInfoItems},
    autostart::{is_autostart_enabled, set_autostart_enabled},
    core_update::{delete_core_version, switch_core_version, update_mihomo_core},
    factory_reset::factory_reset,
    frontend::{open_admin_frontend, open_frontend},
    platform::{confirm_dialog, is_running_as_admin, restart_as_admin, show_error_dialog},
    runtime::rebuild_runtime,
    system_proxy::apply_system_proxy,
    utils::wait_for_port_release,
};

async fn handle_system_proxy_toggle(state: AppState) -> anyhow::Result<()> {
    if state.is_system_proxy_enabled().await {
        apply_system_proxy(None)?;
        state.refresh_system_proxy_state().await;
        Ok(())
    } else {
        let runtime = state.runtime().await?;
        let endpoint = runtime
            .http_proxy_endpoint()
            .await?
            .ok_or_else(|| anyhow!("当前配置中未配置代理端口（port/mixed-port）"))?;
        apply_system_proxy(Some(&endpoint))?;
        state.refresh_system_proxy_state().await;
        Ok(())
    }
}

async fn handle_autostart_toggle(state: AppState) -> anyhow::Result<()> {
    let enabled = is_autostart_enabled();
    let new_state = !enabled;
    if new_state && !is_running_as_admin() {
        return Err(anyhow!("开启开机自启需要管理员权限"));
    }
    set_autostart_enabled(new_state)?;
    state.set_autostart_checked(new_state).await;
    Ok(())
}

async fn handle_open_webui_toggle(state: AppState) -> anyhow::Result<()> {
    let current = state.open_webui_on_startup().await;
    let new_state = !current;
    state.set_open_webui_on_startup(new_state).await;
    state.set_open_webui_checked(new_state).await;
    Ok(())
}

async fn build_tray_menu(
    app: &AppHandle,
    state: &AppState,
) -> tauri::Result<(Menu<Wry>, TrayInfoItems)> {
    let static_info_item = MenuItem::with_id(
        app,
        "static-info",
        "静态站点: 启动中...",
        false,
        None::<&str>,
    )?;
    let controller_info_item = MenuItem::with_id(
        app,
        "controller-info",
        "控制接口: 初始化中",
        false,
        None::<&str>,
    )?;
    let admin_info_item = MenuItem::with_id(
        app,
        "admin-info",
        "配置管理: 启动中...",
        false,
        None::<&str>,
    )?;
    let admin_privilege_item = MenuItem::with_id(
        app,
        "admin-privilege",
        "管理员权限: 检测中...",
        false,
        None::<&str>,
    )?;
    let autostart_enabled = is_autostart_enabled();
    let autostart_supported = cfg!(target_os = "windows");
    let open_webui_checked = state.open_webui_on_startup().await;
    let autostart_is_admin = is_running_as_admin();
    let autostart_label = if autostart_supported && !autostart_is_admin {
        "开机自启（需管理员）"
    } else {
        "开机自启"
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
        "启动时打开 Web UI",
        true,
        open_webui_checked,
        None::<&str>,
    )?;
    let core_version_item =
        MenuItem::with_id(app, "core-version", "当前内核: 读取中...", false, None::<&str>)?;
    let core_installed_item = MenuItem::with_id(
        app,
        "core-installed",
        "已下载版本: 读取中...",
        false,
        None::<&str>,
    )?;
    let core_status_item =
        MenuItem::with_id(app, "core-status", "更新状态: 读取中...", false, None::<&str>)?;
    let core_network_item =
        MenuItem::with_id(app, "core-network", "网络: 读取中...", false, None::<&str>)?;
    let core_update_item =
        MenuItem::with_id(app, "core-update", "更新到最新 Stable", true, None::<&str>)?;
    let versions = match VersionManager::new() {
        Ok(vm) => vm.list_installed().await.unwrap_or_default(),
        Err(err) => {
            warn!("failed to read installed versions: {err}");
            Vec::new()
        }
    };
    let core_default_checked = state.use_bundled_core().await || versions.is_empty();
    let core_default_item = CheckMenuItem::with_id(
        app,
        "core-default",
        "默认内核",
        true,
        core_default_checked,
        None::<&str>,
    )?;
    let mut version_submenus: Vec<Submenu<Wry>> = Vec::new();
    for version in versions {
        let use_item = MenuItem::with_id(
            app,
            format!("core-use-{}", version.version),
            "启用",
            true,
            None::<&str>,
        )?;
        let delete_item = MenuItem::with_id(
            app,
            format!("core-delete-{}", version.version),
            "删除",
            true,
            None::<&str>,
        )?;
        let submenu = Submenu::with_items(
            app,
            version.version,
            true,
            &[&use_item, &delete_item],
        )?;
        version_submenus.push(submenu);
    }
    let empty_versions_item =
        MenuItem::with_id(app, "core-empty", "暂无已下载版本", false, None::<&str>)?;
    let mut version_items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
    if version_submenus.is_empty() {
        version_items.push(&empty_versions_item);
    } else {
        for submenu in &version_submenus {
            version_items.push(submenu);
        }
    }
    let core_versions_submenu =
        Submenu::with_items(app, "已下载版本", true, version_items.as_slice())?;

    let core_submenu = Submenu::with_items(
        app,
        "内核管理",
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
        "设置",
        true,
        &[&autostart_item, &open_webui_item],
    )?;
    let proxy_item =
        MenuItem::with_id(app, "system-proxy", "系统代理: 已关闭", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "打开浏览器", true, None::<&str>)?;
    let config_item = MenuItem::with_id(app, "config-manager", "打开配置管理", true, None::<&str>)?;
    let restart_admin_item =
        MenuItem::with_id(app, "restart-admin", "以管理员身份重启", true, None::<&str>)?;
    let factory_reset_item =
        MenuItem::with_id(app, "factory-reset", "恢复出厂设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let is_admin = is_running_as_admin();
    if let Err(err) = restart_admin_item.set_enabled(!is_admin) {
        warn!("failed to update restart admin menu item: {err}");
    }
    let menu = Menu::with_items(
        app,
        &[
            &static_info_item,
            &controller_info_item,
            &admin_info_item,
            &admin_privilege_item,
            &core_submenu,
            &settings_submenu,
            &proxy_item,
            &show_item,
            &config_item,
            &restart_admin_item,
            &factory_reset_item,
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
        autostart: autostart_item.clone(),
        open_webui: open_webui_item.clone(),
    };
    Ok((menu, items))
}

pub(crate) fn create_tray(app: &AppHandle, state: AppState) -> tauri::Result<()> {
    let (menu, items) = tauri::async_runtime::block_on(async { build_tray_menu(app, &state).await })?;
    let is_admin = is_running_as_admin();
    tauri::async_runtime::block_on(async {
        state.set_tray_info_items(items).await;
        state.refresh_system_proxy_state().await;
        state.update_admin_privilege_text(is_admin).await;
        state.refresh_core_version_info().await;
    });
    let state_for_menu = state.clone();
    let state_for_tray_click = state.clone();

    TrayIconBuilder::with_id("metacube-tray")
        .tooltip("Mihomo Despicable Infiltrator")
        .icon(include_image!("icons/tray.ico"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                open_frontend(state_for_menu.clone());
            }
            "config-manager" => {
                open_admin_frontend(state_for_menu.clone());
            }
            "system-proxy" => {
                let state_clone = state_for_menu.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = handle_system_proxy_toggle(state_clone).await {
                        show_error_dialog(format!("切换系统代理失败: {err:#}"));
                    }
                });
            }
            "autostart" => {
                let state_clone = state_for_menu.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = handle_autostart_toggle(state_clone).await {
                        show_error_dialog(format!("切换开机自启失败: {err:#}"));
                    }
                });
            }
            "open-webui" => {
                let state_clone = state_for_menu.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = handle_open_webui_toggle(state_clone).await {
                        show_error_dialog(format!("切换启动打开 Web UI 失败: {err:#}"));
                    }
                });
            }
            "core-default" => {
                let state_clone = state_for_menu.clone();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    state_clone.set_use_bundled_core(true).await;
                    if let Err(err) = rebuild_runtime(&app_handle, &state_clone).await {
                        show_error_dialog(format!("切换到默认内核失败: {err:#}"));
                        return;
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                });
            }
            "core-update" => {
                let app_handle = app.clone();
                let state_clone = state_for_menu.clone();
                tauri::async_runtime::spawn(async move {
                    state_clone
                        .update_core_version_text("内核版本: 更新中...")
                        .await;
                    state_clone
                        .update_core_installed_text("已安装版本: 更新中...")
                        .await;
                    state_clone.set_core_update_enabled(false).await;
                    let result = update_mihomo_core(&app_handle, &state_clone).await;
                    state_clone.set_core_update_enabled(true).await;
                    state_clone.refresh_core_version_info().await;
                    if let Err(err) = result {
                        show_error_dialog(format!("更新 Mihomo 内核失败: {err:#}"));
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                });
            }
            "restart-admin" => {
                if is_running_as_admin() {
                    show_error_dialog("当前已是管理员权限，无需重启".to_string());
                    return;
                }
                let state_clone = state_for_menu.clone();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let (static_port, admin_port) = state_clone.current_ports().await;
                    state_clone.shutdown_all().await;
                    if let Some(port) = static_port {
                        wait_for_port_release(port, Duration::from_secs(5)).await;
                    }
                    if let Some(port) = admin_port {
                        wait_for_port_release(port, Duration::from_secs(5)).await;
                    }
                    match restart_as_admin(static_port, admin_port) {
                        Ok(()) => app_handle.exit(0),
                        Err(err) => show_error_dialog(format!("以管理员身份重启失败: {err:#}")),
                    }
                });
            }
            "factory-reset" => {
                let state_clone = state_for_menu.clone();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let confirmed = confirm_dialog(
                        "恢复出厂设置会清空所有配置、已下载内核、日志与应用设置，并重启服务。是否继续？",
                        "恢复出厂设置",
                    );
                    if !confirmed {
                        return;
                    }
                    if let Err(err) = factory_reset(&app_handle, &state_clone).await {
                        show_error_dialog(format!("恢复出厂设置失败: {err:#}"));
                        return;
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {
                if let Some(version) = event.id.as_ref().strip_prefix("core-use-") {
                    let version = version.to_string();
                    let app_handle = app.clone();
                    let state_clone = state_for_menu.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(err) = switch_core_version(&app_handle, &state_clone, &version).await {
                            show_error_dialog(format!("切换内核版本失败: {err:#}"));
                        }
                        if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                            warn!("failed to refresh tray menu: {err:#}");
                        }
                    });
                    return;
                }
                if let Some(version) = event.id.as_ref().strip_prefix("core-delete-") {
                    let version = version.to_string();
                    let app_handle = app.clone();
                    let state_clone = state_for_menu.clone();
                    tauri::async_runtime::spawn(async move {
                        let confirmed = confirm_dialog(
                            &format!("确定删除内核版本 {version} 吗？该操作无法撤销。"),
                            "删除内核版本",
                        );
                        if !confirmed {
                            return;
                        }
                        if let Err(err) = delete_core_version(&version).await {
                            show_error_dialog(format!("删除内核版本失败: {err:#}"));
                        }
                        if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                            warn!("failed to refresh tray menu: {err:#}");
                        }
                    });
                }
            }
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_frontend(state_for_tray_click.clone());
            }
        })
        .build(app)?;

    // fire off a summary refresh when tray is ready
    let app_handle = app.clone();
    let summary_state = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(runtime) = summary_state.runtime().await {
            if let Ok(summary) = runtime.summary().await {
                if let Err(err) = app_handle.emit("mihomo://summary", &summary) {
                    warn!("failed to emit summary event: {err}");
                }
            }
        }
    });

    Ok(())
}

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
