use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use infiltrator_admin::{
    AdminEvent,
    AdminEventBus,
    SubscriptionScheduler,
    servers::{AdminServerHandle, StaticServerHandle},
};
use infiltrator_core::AppSettings;
use infiltrator_desktop::{MihomoRuntime, SystemProxyState};
use log::warn;
use mihomo_api::ProxyInfo;
use mihomo_version::VersionManager;
use tauri::{
    menu::{CheckMenuItem, MenuItem, Submenu},
    AppHandle, Wry,
};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::RwLock;

use crate::{
    locales::{Lang, Localizer},
    settings::save_settings,
    system_proxy::{apply_system_proxy, read_system_proxy_state},
    utils::extract_port_from_url,
};

#[derive(Clone, Default)]
pub(crate) struct AppState {
    runtime: Arc<RwLock<Option<Arc<MihomoRuntime>>>>,
    static_server: Arc<RwLock<Option<StaticServerHandle>>>,
    admin_server: Arc<RwLock<Option<AdminServerHandle>>>,
    tray_info: Arc<RwLock<Option<TrayInfoItems>>>,
    system_proxy: Arc<RwLock<SystemProxyState>>,
    current_mode: Arc<RwLock<Option<String>>>,
    proxy_groups: Arc<RwLock<HashMap<String, ProxyInfo>>>,
    tun_enabled: Arc<RwLock<bool>>,
    subscription_scheduler: Arc<RwLock<Option<SubscriptionScheduler>>>,
    tray_profile_map: Arc<RwLock<HashMap<String, String>>>,
    tray_proxy_map: Arc<RwLock<HashMap<String, (String, String)>>>,
    pub(crate) settings: Arc<RwLock<AppSettings>>,
    pub(crate) app_handle: Arc<RwLock<Option<AppHandle>>>,
    pub(crate) rebuild_lock: Arc<tokio::sync::Mutex<()>>,
    admin_events: AdminEventBus,
}

#[derive(Clone)]
pub(crate) struct TrayInfoItems {
    pub controller: MenuItem<Wry>,
    pub static_host: MenuItem<Wry>,
    pub admin_host: MenuItem<Wry>,
    pub system_proxy: MenuItem<Wry>,
    pub admin_privilege: MenuItem<Wry>,
    pub core_version: MenuItem<Wry>,
    pub core_installed: MenuItem<Wry>,
    pub core_status: MenuItem<Wry>,
    pub core_network: MenuItem<Wry>,
    pub core_update: MenuItem<Wry>,
    pub core_default: CheckMenuItem<Wry>,
    pub core_versions: Submenu<Wry>,
    pub tun_mode: CheckMenuItem<Wry>,
    pub mode_rule: CheckMenuItem<Wry>,
    pub mode_global: CheckMenuItem<Wry>,
    pub mode_direct: CheckMenuItem<Wry>,
    pub mode_script: CheckMenuItem<Wry>,
    pub profile_switch: Submenu<Wry>,
    pub proxy_groups: Submenu<Wry>,
    pub autostart: CheckMenuItem<Wry>,
    pub open_webui: CheckMenuItem<Wry>,
}

impl AppState {
    pub(crate) fn ctx_as_admin(&self) -> anyhow::Result<crate::admin_context::TauriAdminContext> {
        let handle = self
            .app_handle
            .try_read()
            .ok()
            .and_then(|g| g.as_ref().cloned())
            .ok_or_else(|| anyhow!("app handle is not ready"))?;
        Ok(crate::admin_context::TauriAdminContext {
            app: handle,
            app_state: self.clone(),
        })
    }

    pub(crate) fn admin_event_bus(&self) -> AdminEventBus {
        self.admin_events.clone()
    }

    pub(crate) fn emit_admin_event(&self, event: AdminEvent) {
        self.admin_events.publish(event);
    }

    pub(crate) async fn get_app_settings(&self) -> AppSettings {
        self.settings.read().await.clone()
    }

    pub(crate) async fn get_lang_code(&self) -> String {
        let lang = self.settings.read().await.language.clone();
        crate::locales::resolve_language_code(&lang)
    }

    pub(crate) async fn set_runtime(&self, runtime: MihomoRuntime) {
        let mut guard = self.runtime.write().await;
        *guard = Some(Arc::new(runtime));
    }

    pub(crate) async fn runtime(&self) -> anyhow::Result<Arc<MihomoRuntime>> {
        let guard = self.runtime.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("mihomo runtime is not ready yet"))
    }

    pub(crate) async fn set_static_server(&self, handle: StaticServerHandle) {
        let mut guard = self.static_server.write().await;
        *guard = Some(handle);
    }

    pub(crate) async fn stop_frontends(&self) {
        if let Some(handle) = self.static_server.write().await.take() {
            handle.stop();
            self.update_static_info_text("静态站点: 已停止").await;
        }
        if let Some(handle) = self.admin_server.write().await.take() {
            handle.stop();
            self.update_admin_info_text("配置管理: 已停止").await;
        }
    }

    pub(crate) async fn stop_runtime(&self) {
        let runtime = self.runtime.write().await.take();
        if let Some(runtime) = runtime {
            if let Err(err) = runtime.shutdown().await {
                warn!("failed to stop mihomo runtime: {err}");
            } else {
                self.update_controller_info_text("控制接口: 已停止").await;
            }
        }
    }

    pub(crate) async fn shutdown_all(&self) {
        self.shutdown_subscription_scheduler().await;
        self.stop_frontends().await;
        self.stop_runtime().await;
        self.disable_system_proxy().await;
    }

    pub(crate) async fn static_server_url(&self) -> Option<String> {
        self.static_server
            .read()
            .await
            .as_ref()
            .map(|handle| handle.url.clone())
    }

    pub(crate) async fn set_admin_server(&self, handle: AdminServerHandle) {
        let mut guard = self.admin_server.write().await;
        *guard = Some(handle);
    }

    pub(crate) async fn admin_server_url(&self) -> Option<String> {
        self.admin_server
            .read()
            .await
            .as_ref()
            .map(|handle| handle.url.clone())
    }

    pub(crate) async fn set_tray_info_items(&self, items: TrayInfoItems) {
        let mut guard = self.tray_info.write().await;
        *guard = Some(items);
    }

    pub(crate) async fn tray_info_items(&self) -> Option<TrayInfoItems> {
        self.tray_info.read().await.clone()
    }

    pub(crate) async fn set_app_handle(&self, handle: AppHandle) {
        let mut guard = self.app_handle.write().await;
        *guard = Some(handle);
    }

    pub(crate) async fn set_current_mode(&self, mode: Option<String>) {
        let mut guard = self.current_mode.write().await;
        *guard = mode;
    }

    pub(crate) async fn update_mode_checked(&self, mode: Option<&str>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            let is_rule = mode == Some("rule");
            let is_global = mode == Some("global");
            let is_direct = mode == Some("direct");
            let is_script = mode == Some("script");
            if let Err(err) = items.mode_rule.set_checked(is_rule) {
                warn!("failed to update mode rule menu item: {err}");
            }
            if let Err(err) = items.mode_global.set_checked(is_global) {
                warn!("failed to update mode global menu item: {err}");
            }
            if let Err(err) = items.mode_direct.set_checked(is_direct) {
                warn!("failed to update mode direct menu item: {err}");
            }
            if let Err(err) = items.mode_script.set_checked(is_script) {
                warn!("failed to update mode script menu item: {err}");
            }
        }
    }

    pub(crate) async fn set_proxy_groups(&self, groups: HashMap<String, ProxyInfo>) {
        let mut guard = self.proxy_groups.write().await;
        *guard = groups;
    }

    pub(crate) async fn refresh_proxy_groups(
        &self,
    ) -> anyhow::Result<HashMap<String, ProxyInfo>> {
        let runtime = self.runtime().await?;
        let proxies = runtime
            .client()
            .get_proxies()
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        self.set_proxy_groups(proxies.clone()).await;
        Ok(proxies)
    }

    pub(crate) async fn set_tun_enabled(&self, enabled: bool) {
        let mut guard = self.tun_enabled.write().await;
        *guard = enabled;
    }

    pub(crate) async fn update_tun_checked(&self, enabled: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.tun_mode.set_checked(enabled) {
                warn!("failed to update tun menu item: {err}");
            }
        }
    }

    pub(crate) async fn set_tray_profile_map(&self, map: HashMap<String, String>) {
        let mut guard = self.tray_profile_map.write().await;
        *guard = map;
    }

    pub(crate) async fn tray_profile_map(&self) -> HashMap<String, String> {
        self.tray_profile_map.read().await.clone()
    }

    pub(crate) async fn tray_proxy_map(&self) -> HashMap<String, (String, String)> {
        self.tray_proxy_map.read().await.clone()
    }

    pub(crate) async fn set_tray_proxy_map(
        &self,
        map: HashMap<String, (String, String)>,
    ) {
        let mut guard = self.tray_proxy_map.write().await;
        *guard = map;
    }

    pub(crate) async fn refresh_tun_state(&self) -> anyhow::Result<(bool, bool)> {
        let runtime = self.runtime().await?;
        let config = runtime
            .client()
            .get_config()
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        let enabled = match config.tun.as_ref() {
            Some(tun) => tun.get("enable").and_then(|value| value.as_bool()).unwrap_or(false),
            None => false,
        };
        // Always available to toggle if we can talk to the core
        let available = true;
        self.set_tun_enabled(enabled).await;
        Ok((available, enabled))
    }

    pub(crate) async fn set_subscription_scheduler(&self, scheduler: SubscriptionScheduler) {
        let mut guard = self.subscription_scheduler.write().await;
        *guard = Some(scheduler);
    }

    pub(crate) async fn shutdown_subscription_scheduler(&self) {
        if let Some(scheduler) = self.subscription_scheduler.write().await.take() {
            scheduler.shutdown();
        }
    }

    pub(crate) async fn update_static_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.static_host.set_text(text.into()) {
                warn!("failed to update static host info menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_controller_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.controller.set_text(text.into()) {
                warn!("failed to update controller info menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_admin_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.admin_host.set_text(text.into()) {
                warn!("failed to update admin info menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_system_proxy_text(&self, enabled: bool, endpoint: Option<&str>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            let lang_code = self.get_lang_code().await;
            let lang = Lang(lang_code.as_str());
            let prefix = lang.tr("system_proxy");
            let state_text = if enabled {
                lang.tr("enabled")
            } else {
                lang.tr("disabled")
            };

            let text = if enabled {
                match endpoint {
                    Some(addr) => format!("{}: {} ({})", prefix, state_text, addr),
                    None => format!("{}: {}", prefix, state_text),
                }
            } else {
                format!("{}: {}", prefix, state_text)
            };
            if let Err(err) = items.system_proxy.set_text(text) {
                warn!("failed to update system proxy menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_admin_privilege_text(&self, is_admin: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            let lang_code = self.get_lang_code().await;
            let lang = Lang(lang_code.as_str());
            let text = if is_admin {
                lang.tr("acquired")
            } else {
                lang.tr("not_acquired")
            };
            if let Err(err) = items.admin_privilege.set_text(text) {
                warn!("failed to update admin privilege menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_core_version_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_version.set_text(text.into()) {
                warn!("failed to update core version menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_core_installed_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_installed.set_text(text.into()) {
                warn!("failed to update core installed menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_core_status_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_status.set_text(text.into()) {
                warn!("failed to update core status menu item: {err}");
            }
        }
    }

    pub(crate) async fn update_core_network_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_network.set_text(text.into()) {
                warn!("failed to update core network menu item: {err}");
            }
        }
    }

    pub(crate) async fn set_core_update_enabled(&self, enabled: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_update.set_enabled(enabled) {
                warn!("failed to update core update menu item: {err}");
            }
        }
    }

    pub(crate) async fn refresh_core_version_info(&self) {
        let lang_code = self.get_lang_code().await;
        let lang = Lang(lang_code.as_str());

        match read_core_version_info(self).await {
            Ok((current, installed, use_bundled)) => {
                let current_prefix = lang.tr("current_core");
                let not_set = lang.tr("not_set");
                let default_core = lang.tr("default_core");

                let current_text = if use_bundled || (current.is_none() && installed == 0) {
                    format!("{}: {}", current_prefix, default_core)
                } else {
                    current
                        .map(|v| format!("{}: {}", current_prefix, v))
                        .unwrap_or_else(|| format!("{}: {}", current_prefix, not_set))
                };
                let installed_text = format!("{}: {}", lang.tr("downloaded_version"), installed);
                self.update_core_version_text(current_text).await;
                self.update_core_installed_text(installed_text).await;
                self.update_core_status_text(format!("{}: {}", lang.tr("update_status"), lang.tr("idle")))
                    .await;
                self.update_core_network_text(format!("{}: {}", lang.tr("network_check"), lang.tr("not_set")))
                    .await;
                self.set_core_update_enabled(true).await;
                if let Some(items) = self.tray_info.read().await.as_ref() {
                    if let Err(err) = items.core_default.set_checked(use_bundled) {
                        warn!("failed to update core default menu item: {err}");
                    }
                }
            }
            Err(err) => {
                warn!("failed to read core version info: {err:#}");
                let reading_failed = lang.tr("profile_read_failed"); 

                self.update_core_version_text(format!("{}: {}", lang.tr("current_core"), reading_failed))
                    .await;
                self.update_core_installed_text(format!("{}: {}", lang.tr("downloaded_version"), reading_failed))
                    .await;
                self.update_core_status_text(format!("{}: {}", lang.tr("update_status"), reading_failed))
                    .await;
                self.update_core_network_text(format!("{}: {}", lang.tr("network_check"), lang.tr("not_set")))
                    .await;
                self.set_core_update_enabled(true).await;
            }
        }
    }

    pub(crate) async fn set_system_proxy_state(&self, enabled: bool, endpoint: Option<String>) {
        let mut guard = self.system_proxy.write().await;
        guard.enabled = enabled;
        guard.endpoint = endpoint.clone();
        drop(guard);
        self.update_system_proxy_text(enabled, endpoint.as_deref())
            .await;
    }

    pub(crate) async fn refresh_system_proxy_state(&self) {
        match read_system_proxy_state() {
            Ok(state) => {
                self.set_system_proxy_state(state.enabled, state.endpoint)
                    .await;
            }
            Err(err) => {
                warn!("无法读取系统代理状态: {err:#}");
            }
        }
    }

    pub(crate) async fn is_system_proxy_enabled(&self) -> bool {
        self.system_proxy.read().await.enabled
    }

    pub(crate) async fn disable_system_proxy(&self) {
        if self.is_system_proxy_enabled().await {
            if let Err(err) = apply_system_proxy(None) {
                warn!("failed to disable system proxy: {err}");
            }
            self.refresh_system_proxy_state().await;
        }
    }

    pub(crate) async fn current_ports(&self) -> (Option<u16>, Option<u16>) {
        let static_port = self
            .static_server_url()
            .await
            .and_then(|url| extract_port_from_url(&url));
        let admin_port = self
            .admin_server_url()
            .await
            .and_then(|url| extract_port_from_url(&url));
        (static_port, admin_port)
    }

    pub(crate) async fn set_app_settings(&self, settings: AppSettings) -> anyhow::Result<()> {
        {
            let mut guard = self.settings.write().await;
            *guard = settings;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
            return Err(err);
        }
        // If language changed, refresh tray immediately
        if let Some(app_handle) = self.app_handle.read().await.as_ref() {
            if let Err(err) = crate::tray::refresh_tray_menu(
                app_handle,
                self,
            ).await {
                warn!("failed to refresh tray menu after settings change: {err}");
            }
        }
        Ok(())
    }

    pub(crate) async fn set_open_webui_on_startup(&self, enabled: bool) {
        {
            let mut guard = self.settings.write().await;
            guard.open_webui_on_startup = enabled;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    pub(crate) async fn set_open_webui_checked(&self, enabled: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.open_webui.set_checked(enabled) {
                warn!("failed to update open webui menu item: {err}");
            }
        }
    }

    pub(crate) async fn open_webui_on_startup(&self) -> bool {
        self.settings.read().await.open_webui_on_startup
    }

    pub(crate) async fn set_editor_path(&self, path: Option<String>) {
        {
            let mut guard = self.settings.write().await;
            guard.editor_path = path;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    pub(crate) async fn editor_path(&self) -> Option<String> {
        self.settings.read().await.editor_path.clone()
    }

    pub(crate) async fn set_use_bundled_core(&self, enabled: bool) {
        {
            let mut guard = self.settings.write().await;
            guard.use_bundled_core = enabled;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    pub(crate) async fn set_autostart_checked(&self, enabled: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.autostart.set_checked(enabled) {
                warn!("failed to update autostart menu item: {err}");
            }
        }
    }

    pub(crate) async fn use_bundled_core(&self) -> bool {
        self.settings.read().await.use_bundled_core
    }

    pub(crate) async fn notify_subscription_update(
        &self,
        profile: &str,
        success: bool,
        message: Option<String>,
    ) {
        let lang_code = self.get_lang_code().await;
        let lang = Lang(lang_code.as_str());

        let title = if success {
            lang.tr("sub_update_success")
        } else {
            lang.tr("sub_update_failed")
        };
        let body = if success {
            // Need to support formatting in locales, but simple replacement works for now
            lang.tr("sub_updated").replace("{0}", profile)
        } else {
            let reason = message.unwrap_or_else(|| lang.tr("unknown").into_owned());
            lang.tr("sub_failed_reason").replace("{0}", profile).replace("{1}", &reason)
        };
        self.show_notification(&title, &body).await;
    }

    pub(crate) async fn notify_subscription_update_start(&self) {
        let lang_code = self.get_lang_code().await;
        let lang = Lang(lang_code.as_str());
        self.show_notification(&lang.tr("sub_updating_title"), &lang.tr("sub_updating_msg")).await;
    }

    pub(crate) async fn notify_subscription_update_summary(
        &self,
        updated: usize,
        failed: usize,
        skipped: usize,
    ) {
        let lang_code = self.get_lang_code().await;
        let lang = Lang(lang_code.as_str());
        let body = lang.tr("sub_update_summary")
            .replace("{0}", &updated.to_string())
            .replace("{1}", &failed.to_string())
            .replace("{2}", &skipped.to_string());
        self.show_notification(&lang.tr("sub_update_done"), &body).await;
    }

    pub(crate) async fn notify_webdav_sync_result(
        &self,
        success: bool,
        action_count: usize,
        error_msg: Option<String>,
    ) {
        let lang_code = self.get_lang_code().await;
        let lang = Lang(lang_code.as_str());

        let title = if success {
            lang.tr("webdav_sync_success")
        } else {
            lang.tr("webdav_sync_failed")
        };
        let body = if success {
            lang.tr("webdav_sync_done").replace("{0}", &action_count.to_string())
        } else {
            let reason = error_msg.unwrap_or_else(|| lang.tr("unknown").into_owned());
            lang.tr("webdav_sync_error").replace("{0}", &reason)
        };
        self.show_notification(&title, &body).await;
    }

    async fn show_notification(&self, title: &str, body: &str) {
        let app_handle = self.app_handle.read().await.clone();
        if let Some(handle) = app_handle {
            if let Err(err) = handle
                .notification()
                .builder()
                .title(title)
                .body(body)
                .show()
            {
                warn!("failed to show notification: {err}");
            }
        }
    }
}

async fn read_core_version_info(
    state: &AppState,
) -> anyhow::Result<(Option<String>, usize, bool)> {
    let vm = VersionManager::new()?;
    let installed = vm.list_installed().await?;
    let current = vm.get_default().await.ok();
    let installed_len = installed.len();
    let use_bundled = state.use_bundled_core().await || installed_len == 0;
    Ok((current, installed_len, use_bundled))
}
