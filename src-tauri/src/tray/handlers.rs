use anyhow::anyhow;
use log::warn;
use mihomo_rs::config::ConfigManager;
use tauri::{menu::MenuEvent, AppHandle, Manager};
use tokio::time::Duration;
use chrono::{Duration as ChronoDuration, Utc};
use despicable_infiltrator_core::profiles as core_profiles;

use crate::{
    app_state::AppState,
    autostart::{is_autostart_enabled, set_autostart_enabled},
    core_update::{delete_core_version, switch_core_version, update_mihomo_core},
    factory_reset::factory_reset,
    frontend::{open_admin_frontend, open_frontend},
    locales::Localizer,
    platform::{confirm_dialog, is_running_as_admin, restart_as_admin, show_error_dialog},
    runtime::rebuild_runtime,
    system_proxy::apply_system_proxy,
    utils::wait_for_port_release,
};

use super::menu::refresh_tray_menu;

pub fn handle_menu_event(app: &AppHandle, event: MenuEvent, state: &AppState) {
    let id = event.id.as_ref().to_string();
    let app_handle = app.clone();
    let state_clone = state.clone();

    match id.as_str() {
        "show" => {
            open_frontend(state_clone);
        }
        "config-manager" => {
            open_admin_frontend(state_clone);
        }
        "system-proxy" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_system_proxy_toggle(state_clone).await {
                    show_error_dialog(format!("切换系统代理失败: {err:#}"));
                }
            });
        }
        "autostart" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_autostart_toggle(state_clone).await {
                    show_error_dialog(format!("切换开机自启失败: {err:#}"));
                }
            });
        }
        "open-webui" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_open_webui_toggle(state_clone).await {
                    show_error_dialog(format!("切换启动打开 Web UI 失败: {err:#}"));
                }
            });
        }
        "profile-update-all" => {
            let app_cloned = app.clone();
            tokio::spawn(async move {
                let state = app_cloned.state::<AppState>();
                let ctx = state.ctx_as_admin();
                
                // Notify start
                state.notify_subscription_update_start().await;

                let client = reqwest::Client::new();
                let raw_client = reqwest::Client::new();
                
                match despicable_infiltrator_core::scheduler::subscription::update_all_subscriptions(
                    &ctx,
                    &client,
                    &raw_client,
                )
                .await
                {
                    Ok(summary) => {
                        // Notify summary
                        state.notify_subscription_update_summary(
                            summary.updated,
                            summary.failed,
                            summary.skipped
                        ).await;
                    }
                    Err(err) => {
                        log::error!("failed to update subscriptions: {err}");
                        state.notify_subscription_update("All Subscriptions", false, Some(err.to_string())).await;
                    }
                }
            });
        }
        "tun-mode" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_tun_toggle(state_clone.clone()).await {
                    show_error_dialog(format!("切换 TUN 模式失败: {err:#}"));
                    return;
                }
                if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                    warn!("failed to refresh tray menu: {err:#}");
                }
            });
        }
        "mode-rule" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_mode_switch(state_clone.clone(), "rule").await {
                    show_error_dialog(format!("切换代理模式失败: {err:#}"));
                    return;
                }
                if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                    warn!("failed to refresh tray menu: {err:#}");
                }
            });
        }
        "mode-global" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_mode_switch(state_clone.clone(), "global").await {
                    show_error_dialog(format!("切换代理模式失败: {err:#}"));
                    return;
                }
                if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                    warn!("failed to refresh tray menu: {err:#}");
                }
            });
        }
        "mode-direct" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_mode_switch(state_clone.clone(), "direct").await {
                    show_error_dialog(format!("切换代理模式失败: {err:#}"));
                    return;
                }
                if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                    warn!("failed to refresh tray menu: {err:#}");
                }
            });
        }
        "mode-script" => {
            tauri::async_runtime::spawn(async move {
                if let Err(err) = handle_mode_switch(state_clone.clone(), "script").await {
                    show_error_dialog(format!("切换代理模式失败: {err:#}"));
                    return;
                }
                if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                    warn!("failed to refresh tray menu: {err:#}");
                }
            });
        }
        "core-default" => {
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
        "webdav-sync-now" => {
            tauri::async_runtime::spawn(async move {
                let settings = state_clone.get_app_settings().await;
                if !settings.webdav.enabled {
                    let lang_code = state_clone.get_lang_code().await;
                    let lang = crate::locales::Lang(lang_code.as_str());
                    show_error_dialog(lang.tr("webdav_sync_disabled").into_owned());
                    return;
                }
                match despicable_infiltrator_core::scheduler::sync::run_sync_tick(&state_clone.ctx_as_admin(), &settings.webdav).await {
                    Ok(summary) => {
                        state_clone.notify_webdav_sync_result(true, summary.success_count, None).await;
                        if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                            warn!("failed to refresh tray menu: {err:#}");
                        }
                    }
                    Err(err) => {
                        state_clone.notify_webdav_sync_result(false, 0, Some(err.to_string())).await;
                    }
                }
            });
        }
        "webdav-sync-settings" => {
            open_admin_frontend(state_clone);
        }
        "quit" => {
            app_handle.exit(0);
        }
        _ => {
            tauri::async_runtime::spawn(async move {
                if let Some(profile_name) = id.strip_prefix("profile-auto-update-") {
                    if let Err(err) = handle_auto_update_toggle(profile_name.to_string()).await {
                        show_error_dialog(format!("切换自动更新失败: {err:#}"));
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                    return;
                }

                if let Some(profile_name) = state_clone.tray_profile_map().await.get(&id).cloned() {
                    if let Err(err) =
                        handle_profile_switch(&app_handle, state_clone.clone(), &profile_name).await
                    {
                        show_error_dialog(format!("切换配置失败: {err:#}"));
                        return;
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                    return;
                }

                if let Some((group_name, node_name)) =
                    state_clone.tray_proxy_map().await.get(&id).cloned()
                {
                    let runtime = match state_clone.runtime().await {
                        Ok(runtime) => runtime,
                        Err(_) => {
                            // Retry refresh if runtime not ready/proxy map outdated
                            if let Err(_err) = refresh_tray_menu(&app_handle, &state_clone).await {
                                warn!("failed to retry refresh tray menu: {_err}");
                            }
                            return;
                        }
                    };
                    if let Err(err) = runtime.switch_proxy(&group_name, &node_name).await {
                        show_error_dialog(format!("切换代理失败: {err:#}"));
                        return;
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                    return;
                }

                if let Some(version) = id.strip_prefix("core-use-") {
                    if let Err(err) = switch_core_version(&app_handle, &state_clone, version).await
                    {
                        show_error_dialog(format!("切换内核版本失败: {err:#}"));
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                    return;
                }
                if let Some(version) = id.strip_prefix("core-delete-") {
                    let confirmed = confirm_dialog(
                        &format!("确定删除内核版本 {version} 吗？该操作无法撤销。"),
                        "删除内核版本",
                    );
                    if !confirmed {
                        return;
                    }
                    if let Err(err) = delete_core_version(version).await {
                        show_error_dialog(format!("删除内核版本失败: {err:#}"));
                    }
                    if let Err(err) = refresh_tray_menu(&app_handle, &state_clone).await {
                        warn!("failed to refresh tray menu: {err:#}");
                    }
                }
            });
        }
    }
}

// Logic handlers

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

async fn handle_tun_toggle(state: AppState) -> anyhow::Result<()> {
    if !is_running_as_admin() {
        return Err(anyhow!("启用 TUN 需要管理员权限"));
    }
    let (available, enabled) = state.refresh_tun_state().await?;
    if !available {
        // Should not happen with new logic, but safe guard
        return Err(anyhow!("当前配置未启用 TUN"));
    }
    let runtime = state.runtime().await?;
    runtime.set_tun_enabled(!enabled).await?;
    state.set_tun_enabled(!enabled).await;
    Ok(())
}

async fn handle_mode_switch(state: AppState, mode: &str) -> anyhow::Result<()> {
    let runtime = state.runtime().await?;
    runtime.set_mode(mode).await?;
    let current_mode = runtime.current_mode().await.ok();
    state.set_current_mode(current_mode).await;
    Ok(())
}

async fn handle_profile_switch(
    app: &AppHandle,
    state: AppState,
    profile_name: &str,
) -> anyhow::Result<()> {
    let profile_name = core_profiles::sanitize_profile_name(profile_name)?;
    let profiles = core_profiles::list_profile_infos().await?;
    if profiles
        .iter()
        .any(|profile| profile.name == profile_name && profile.active)
    {
        return Ok(());
    }
    let manager = ConfigManager::new()?;
    manager.set_current(&profile_name).await?;
    rebuild_runtime(app, &state).await?;
    Ok(())
}

async fn handle_auto_update_toggle(profile_name: String) -> anyhow::Result<()> {
    let manager = ConfigManager::new()?;
    let mut metadata = manager.get_profile_metadata(&profile_name).await?;

    let new_state = !metadata.auto_update_enabled;
    metadata.auto_update_enabled = new_state;

    if new_state {
        let interval = metadata.update_interval_hours.unwrap_or(24);
        metadata.update_interval_hours = Some(interval);
        metadata.next_update = Some(Utc::now() + ChronoDuration::hours(interval as i64));
    } else {
        metadata.next_update = None;
    }

    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await?;
    Ok(())
}
