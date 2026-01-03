#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::anyhow;
use axum::{
    extract::{Path as AxumPath, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use log::{error, info, warn};
use mihomo_rs::{
    config::{ConfigManager, Profile as MihomoProfile},
    core::{find_available_port, MihomoClient, ProxyGroup, TrafficData},
    proxy::ProxyManager,
    service::{ServiceManager, ServiceStatus},
    version::{channel::fetch_latest, Channel, DownloadProgress, VersionManager},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{
    include_image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, Submenu},
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, RunEvent, Wry,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, RwLock},
    time::{sleep, timeout},
};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone, Default)]
struct AppState {
    runtime: Arc<RwLock<Option<Arc<MihomoRuntime>>>>,
    static_server: Arc<RwLock<Option<StaticServerHandle>>>,
    admin_server: Arc<RwLock<Option<AdminServerHandle>>>,
    tray_info: Arc<RwLock<Option<TrayInfoItems>>>,
    system_proxy: Arc<RwLock<SystemProxyState>>,
    settings: Arc<RwLock<AppSettings>>,
    app_handle: Arc<RwLock<Option<AppHandle>>>,
}

#[derive(Clone, Default)]
struct SystemProxyState {
    enabled: bool,
    endpoint: Option<String>,
}

impl AppState {
    async fn set_runtime(&self, runtime: MihomoRuntime) {
        let mut guard = self.runtime.write().await;
        *guard = Some(Arc::new(runtime));
    }

    async fn runtime(&self) -> anyhow::Result<Arc<MihomoRuntime>> {
        let guard = self.runtime.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("mihomo runtime is not ready yet"))
    }

    async fn set_static_server(&self, handle: StaticServerHandle) {
        let mut guard = self.static_server.write().await;
        *guard = Some(handle);
    }

    async fn stop_frontends(&self) {
        if let Some(handle) = self.static_server.write().await.take() {
            handle.stop();
            self.update_static_info_text("静态站点: 已停止").await;
        }
        if let Some(handle) = self.admin_server.write().await.take() {
            handle.stop();
            self.update_admin_info_text("配置管理: 已停止").await;
        }
    }

    async fn stop_runtime(&self) {
        let runtime = self.runtime.write().await.take();
        if let Some(runtime) = runtime {
            if let Err(err) = runtime.shutdown().await {
                warn!("failed to stop mihomo runtime: {err}");
            } else {
                self.update_controller_info_text("控制接口: 已停止").await;
            }
        }
    }

    async fn shutdown_all(&self) {
        self.stop_frontends().await;
        self.stop_runtime().await;
        self.disable_system_proxy().await;
    }

    async fn static_server_url(&self) -> Option<String> {
        self.static_server
            .read()
            .await
            .as_ref()
            .map(|handle| handle.url.clone())
    }

    async fn set_admin_server(&self, handle: AdminServerHandle) {
        let mut guard = self.admin_server.write().await;
        *guard = Some(handle);
    }

    async fn admin_server_url(&self) -> Option<String> {
        self.admin_server
            .read()
            .await
            .as_ref()
            .map(|handle| handle.url.clone())
    }

    async fn set_tray_info_items(&self, items: TrayInfoItems) {
        let mut guard = self.tray_info.write().await;
        *guard = Some(items);
    }

    async fn set_app_handle(&self, handle: AppHandle) {
        let mut guard = self.app_handle.write().await;
        *guard = Some(handle);
    }

    async fn update_static_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.static_host.set_text(text.into()) {
                warn!("failed to update static host info menu item: {err}");
            }
        }
    }

    async fn update_controller_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.controller.set_text(text.into()) {
                warn!("failed to update controller info menu item: {err}");
            }
        }
    }

    async fn update_admin_info_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.admin_host.set_text(text.into()) {
                warn!("failed to update admin info menu item: {err}");
            }
        }
    }

    async fn update_system_proxy_text(&self, enabled: bool, endpoint: Option<&str>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            let text = if enabled {
                match endpoint {
                    Some(addr) => format!("系统代理: 已开启 ({addr})"),
                    None => "系统代理: 已开启".to_string(),
                }
            } else {
                "系统代理: 已关闭".to_string()
            };
            if let Err(err) = items.system_proxy.set_text(text) {
                warn!("failed to update system proxy menu item: {err}");
            }
        }
    }

    async fn update_admin_privilege_text(&self, is_admin: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            let text = if is_admin {
                "管理员权限: 已获取"
            } else {
                "管理员权限: 未获取（开机自启需管理员）"
            };
            if let Err(err) = items.admin_privilege.set_text(text) {
                warn!("failed to update admin privilege menu item: {err}");
            }
        }
    }

    async fn update_core_version_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_version.set_text(text.into()) {
                warn!("failed to update core version menu item: {err}");
            }
        }
    }

    async fn update_core_installed_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_installed.set_text(text.into()) {
                warn!("failed to update core installed menu item: {err}");
            }
        }
    }

    async fn update_core_status_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_status.set_text(text.into()) {
                warn!("failed to update core status menu item: {err}");
            }
        }
    }

    async fn update_core_network_text(&self, text: impl Into<String>) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_network.set_text(text.into()) {
                warn!("failed to update core network menu item: {err}");
            }
        }
    }

    async fn set_core_update_enabled(&self, enabled: bool) {
        if let Some(items) = self.tray_info.read().await.as_ref() {
            if let Err(err) = items.core_update.set_enabled(enabled) {
                warn!("failed to update core update menu item: {err}");
            }
        }
    }

    async fn refresh_core_version_info(&self) {
        match read_core_version_info(self).await {
            Ok((current, installed, use_bundled)) => {
                let current_text = if use_bundled || (current.is_none() && installed == 0) {
                    "当前内核: 默认内核".to_string()
                } else {
                    current
                        .map(|v| format!("当前内核: {v}"))
                        .unwrap_or_else(|| "当前内核: 未设置".to_string())
                };
                let installed_text = format!("已下载版本: {installed}");
                self.update_core_version_text(current_text).await;
                self.update_core_installed_text(installed_text).await;
                self.update_core_status_text("更新状态: 空闲")
                    .await;
                self.update_core_network_text("网络: 未检测")
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
                self.update_core_version_text("当前内核: 读取失败")
                    .await;
                self.update_core_installed_text("已下载版本: 读取失败")
                    .await;
                self.update_core_status_text("更新状态: 读取失败")
                    .await;
                self.update_core_network_text("网络: 未检测")
                    .await;
                self.set_core_update_enabled(true).await;
            }
        }
    }

    async fn set_system_proxy_state(&self, enabled: bool, endpoint: Option<String>) {
        let mut guard = self.system_proxy.write().await;
        guard.enabled = enabled;
        guard.endpoint = endpoint.clone();
        drop(guard);
        self.update_system_proxy_text(enabled, endpoint.as_deref())
            .await;
    }

    async fn refresh_system_proxy_state(&self) {
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

    async fn is_system_proxy_enabled(&self) -> bool {
        self.system_proxy.read().await.enabled
    }

    async fn disable_system_proxy(&self) {
        if self.is_system_proxy_enabled().await {
            if let Err(err) = apply_system_proxy(None) {
                warn!("failed to disable system proxy: {err}");
            }
            self.refresh_system_proxy_state().await;
        }
    }

    async fn current_ports(&self) -> (Option<u16>, Option<u16>) {
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

    async fn set_open_webui_on_startup(&self, enabled: bool) {
        {
            let mut guard = self.settings.write().await;
            guard.open_webui_on_startup = enabled;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    async fn open_webui_on_startup(&self) -> bool {
        self.settings.read().await.open_webui_on_startup
    }

    async fn set_editor_path(&self, path: Option<String>) {
        {
            let mut guard = self.settings.write().await;
            guard.editor_path = path;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    async fn editor_path(&self) -> Option<String> {
        self.settings.read().await.editor_path.clone()
    }

    async fn set_use_bundled_core(&self, enabled: bool) {
        {
            let mut guard = self.settings.write().await;
            guard.use_bundled_core = enabled;
        }
        if let Err(err) = save_settings(self).await {
            warn!("failed to save settings: {err}");
        }
    }

    async fn use_bundled_core(&self) -> bool {
        self.settings.read().await.use_bundled_core
    }

}

struct StaticServerHandle {
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl StaticServerHandle {
    fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

struct AdminServerHandle {
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl AdminServerHandle {
    fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

struct TrayInfoItems {
    controller: MenuItem<Wry>,
    static_host: MenuItem<Wry>,
    admin_host: MenuItem<Wry>,
    system_proxy: MenuItem<Wry>,
    admin_privilege: MenuItem<Wry>,
    core_version: MenuItem<Wry>,
    core_installed: MenuItem<Wry>,
    core_status: MenuItem<Wry>,
    core_network: MenuItem<Wry>,
    core_update: MenuItem<Wry>,
    core_default: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    open_webui: CheckMenuItem<Wry>,
}

#[derive(Clone)]
struct AdminServerState {
    app: AppHandle,
    app_state: AppState,
    http_client: Client,
}

struct MihomoRuntime {
    config_manager: ConfigManager,
    config_path: PathBuf,
    controller_url: String,
    client: MihomoClient,
    service_manager: ServiceManager,
}

impl MihomoRuntime {
    async fn bootstrap(app: &AppHandle, state: &AppState) -> anyhow::Result<Self> {
        let vm = VersionManager::new()?;
        let cm = ConfigManager::new()?;

        cm.ensure_default_config().await?;
        let controller_url = cm.ensure_external_controller().await?;
        let config_path = cm.get_current_path().await?;

        let binary = resolve_binary(app, state, &vm).await?;
        let service_manager = ServiceManager::new(binary, config_path.clone());

        if !service_manager.is_running().await {
            info!("Starting mihomo service");
            service_manager.start().await?;
        }

        let client = MihomoClient::new(&controller_url, None)?;

        Ok(Self {
            config_manager: cm,
            config_path,
            controller_url,
            client,
            service_manager,
        })
    }

    fn client(&self) -> MihomoClient {
        self.client.clone()
    }

    async fn summary(&self) -> anyhow::Result<MihomoSummary> {
        let profile = self.config_manager.get_current().await?;
        let mode = self.read_mode(&profile).await?;
        let running = matches!(
            self.service_manager.status().await?,
            ServiceStatus::Running(_)
        );

        let proxy_manager = ProxyManager::new(self.client());
        let groups = proxy_manager.list_groups().await.unwrap_or_default();

        Ok(MihomoSummary {
            profile,
            mode,
            running,
            controller: self.controller_url.clone(),
            groups,
        })
    }

    async fn read_mode(&self, profile: &str) -> anyhow::Result<String> {
        let content = self.config_manager.load(profile).await?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;
        Ok(doc
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("rule")
            .to_string())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        self.service_manager
            .stop()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    async fn http_proxy_endpoint(&self) -> anyhow::Result<Option<String>> {
        let content = tokio::fs::read_to_string(&self.config_path).await?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;
        let port = doc
            .get("mixed-port")
            .or_else(|| doc.get("port"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u16);
        Ok(port.map(|p| format!("127.0.0.1:{p}")))
    }
}

#[derive(Debug, Clone, Serialize)]
struct MihomoSummary {
    profile: String,
    mode: String,
    running: bool,
    controller: String,
    groups: Vec<ProxyGroup>,
}

#[derive(Debug, Clone, Serialize)]
struct ReadyPayload {
    controller: String,
    config_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProfileInfo {
    name: String,
    active: bool,
    path: String,
}

#[derive(Debug, Serialize)]
struct ProfileDetail {
    name: String,
    active: bool,
    path: String,
    content: String,
}

fn profile_to_info(profile: MihomoProfile) -> ProfileInfo {
    ProfileInfo {
        name: profile.name,
        active: profile.active,
        path: profile.path.to_string_lossy().to_string(),
    }
}

async fn load_profile_info(name: &str) -> anyhow::Result<ProfileInfo> {
    let cm = ConfigManager::new()?;
    let profiles = cm.list_profiles().await?;
    profiles
        .into_iter()
        .find(|profile| profile.name == name)
        .map(profile_to_info)
        .ok_or_else(|| anyhow!("未找到名称为 {name} 的配置文件"))
}

fn sanitize_profile_name(name: &str) -> anyhow::Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("配置名称不能为空"));
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(anyhow!("配置名称不能包含特殊字符 / \\ : * ? \" < > |"));
    }
    Ok(trimmed.to_string())
}

fn ensure_valid_profile_name(name: &str) -> Result<String, ApiError> {
    sanitize_profile_name(name).map_err(|e| ApiError::bad_request(e.to_string()))
}

async fn switch_profile_internal(
    app: &AppHandle,
    state: &AppState,
    name: &str,
) -> anyhow::Result<()> {
    let profile_name = sanitize_profile_name(name)?;
    let manager = ConfigManager::new()?;
    manager.set_current(&profile_name).await?;
    rebuild_runtime(app, state).await?;
    Ok(())
}

async fn import_profile_from_url_internal(
    app: &AppHandle,
    state: &AppState,
    client: &Client,
    name: &str,
    url: &str,
    activate: bool,
) -> anyhow::Result<ProfileInfo> {
    let profile_name = sanitize_profile_name(name)?;
    let source_url = url.trim();
    if source_url.is_empty() {
        return Err(anyhow!("订阅链接不能为空"));
    }

    let response = client.get(source_url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("拉取失败，HTTP {}", response.status()));
    }
    let content = response.text().await?;
    if content.trim().is_empty() {
        return Err(anyhow!("订阅返回内容为空"));
    }

    let manager = ConfigManager::new()?;
    manager.save(&profile_name, &content).await?;

    if activate {
        manager.set_current(&profile_name).await?;
        rebuild_runtime(app, state).await?;
    }

    load_profile_info(&profile_name).await
}

fn main() {
    let launch_ports = parse_launch_ports();
    let builder = tauri::Builder::default()
        .manage(AppState::default())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
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

fn spawn_runtime(app: AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        match MihomoRuntime::bootstrap(&app, &state).await {
            Ok(runtime) => {
                register_runtime(&app, &state, runtime).await;
                spawn_traffic_stream(app.clone(), state.clone());
            }
            Err(err) => {
                error!("failed to bootstrap mihomo runtime: {err:#}");
                show_error_dialog(format!("无法启动 mihomo 服务: {err:#}"));
                state
                    .update_controller_info_text(format!("控制接口: 启动失败 ({err})"))
                    .await;
                if let Err(e) = app.emit("mihomo://error", err.to_string()) {
                    warn!("failed to emit runtime error event: {e}");
                }
            }
        }
    });
}

fn spawn_frontends(app: AppHandle, state: AppState, static_port: Option<u16>, admin_port: Option<u16>) {
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
                show_error_dialog(format!("静态站点启动失败: {err:#}"));
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

async fn register_runtime(app: &AppHandle, state: &AppState, runtime: MihomoRuntime) {
    let controller = runtime.controller_url.clone();
    let config_path = runtime.config_path.to_string_lossy().to_string();
    state.set_runtime(runtime).await;
    state
        .update_controller_info_text(format!("控制接口: {controller}"))
        .await;
    if let Err(err) = app.emit(
        "mihomo://ready",
        ReadyPayload {
            controller,
            config_path,
        },
    ) {
        warn!("failed to emit ready event: {err}");
    }
}

async fn rebuild_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    if let Ok(runtime) = state.runtime().await {
        if let Err(err) = runtime.shutdown().await {
            warn!("failed to stop running mihomo instance: {err}");
        }
    }
    let runtime = MihomoRuntime::bootstrap(app, state).await?;
    register_runtime(app, state, runtime).await;
    Ok(())
}

fn spawn_traffic_stream(app: AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let client = match state.runtime().await {
                Ok(runtime) => runtime.client(),
                Err(err) => {
                    warn!("runtime not ready for traffic stream: {err}");
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            match client.stream_traffic().await {
                Ok(mut rx) => {
                    while let Some(message) = rx.recv().await {
                        if let Err(err) = app.emit("mihomo://traffic", &TrafficEvent { message }) {
                            warn!("failed to emit traffic event: {err}");
                        }
                    }
                }
                Err(err) => {
                    warn!("traffic stream error: {err}");
                    sleep(Duration::from_secs(3)).await;
                }
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

    let port = match preferred_port {
        Some(port) => port,
        None => find_available_port(4173).ok_or_else(|| anyhow!("没有可用端口用于静态站点"))?,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    let main_service = ServeDir::new(main_dir.clone())
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(main_dir.join("index.html")));

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, Router::new().fallback_service(main_service))
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

        if let Err(err) = server.await {
            warn!("静态站点服务异常退出: {err}");
        }
    });

    Ok(StaticServerHandle {
        url: format!("http://127.0.0.1:{port}"),
        shutdown: Some(shutdown_tx),
    })
}

async fn start_admin_frontend(
    app: &AppHandle,
    state: AppState,
    preferred_port: Option<u16>,
) -> anyhow::Result<AdminServerHandle> {
    let admin_dir = resolve_admin_dir(app)?;
    info!("配置管理静态目录: {}", admin_dir.display());

    let port = match preferred_port {
        Some(port) => port,
        None => find_available_port(5210).ok_or_else(|| anyhow!("没有可用端口用于配置管理界面"))?,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    let admin_state = AdminServerState {
        app: app.clone(),
        app_state: state,
        http_client: Client::new(),
    };

    let admin_static_service = ServeDir::new(admin_dir.clone())
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(admin_dir.join("index.html")));

    let router = Router::new()
        .route("/admin/api/profiles", get(list_profiles_http))
        .route(
            "/admin/api/profiles/:name",
            get(get_profile_http).delete(delete_profile_http),
        )
        .route("/admin/api/profiles/switch", post(switch_profile_http))
        .route("/admin/api/profiles/save", post(save_profile_http))
        .route("/admin/api/profiles/import", post(import_profile_http))
        .route("/admin/api/profiles/open", post(open_profile_in_editor_http))
        .route(
            "/admin/api/editor",
            get(get_editor_config_http).post(set_editor_config_http),
        )
        .route("/admin/api/core/versions", get(list_core_versions_http))
        .route("/admin/api/core/activate", post(activate_core_version_http))
        .nest_service("/admin", admin_static_service)
        .route("/", get(|| async { Redirect::temporary("/admin/") }))
        .fallback(|| async { (StatusCode::NOT_FOUND, "请访问 /admin/") })
        .with_state(admin_state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        if let Err(err) = server.await {
            warn!("配置管理服务异常退出: {err}");
        }
    });

    Ok(AdminServerHandle {
        url: format!("http://127.0.0.1:{port}/admin/"),
        shutdown: Some(shutdown_tx),
    })
}

fn resolve_main_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dev_main = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zashboard");

    let main_dir = if let Ok(custom) = env::var("METACUBEXD_STATIC_DIR") {
        let path = PathBuf::from(custom);
        if path.exists() {
            path
        } else {
            dev_main.clone()
        }
    } else if dev_main.exists() {
        dev_main
    } else {
        app.path().resolve("bin/zashboard", BaseDirectory::Resource)?
    };

    if !main_dir.exists() {
        return Err(anyhow!(
            "未找到 Zashboard 静态资源，请将 dist 内容放到 zashboard/ 目录"
        ));
    }

    Ok(main_dir)
}

fn resolve_admin_dir(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dev_admin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config-manager-ui");

    let admin_dir = if let Ok(custom) = env::var("METACUBEXD_ADMIN_DIR") {
        let path = PathBuf::from(custom);
        if path.exists() {
            path
        } else {
            dev_admin.clone()
        }
    } else if dev_admin.exists() {
        dev_admin
    } else {
        app.path()
            .resolve("bin/config-manager", BaseDirectory::Resource)?
    };

    if !admin_dir.exists() {
        return Err(anyhow!(
            "未找到配置管理静态资源，请保留 config-manager-ui/ 目录"
        ));
    }

    Ok(admin_dir)
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct AppSettings {
    open_webui_on_startup: bool,
    editor_path: Option<String>,
    use_bundled_core: bool,
}

async fn load_settings(state: &AppState) -> anyhow::Result<()> {
    let path = settings_path(state).await?;
    if path.exists() {
        let content = tokio::fs::read_to_string(&path).await?;
        let settings: AppSettings = serde_json::from_str(&content)?;
        *state.settings.write().await = settings;
    }
    Ok(())
}

async fn save_settings(state: &AppState) -> anyhow::Result<()> {
    let path = settings_path(state).await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let settings = state.settings.read().await;
    let content = serde_json::to_string_pretty(&*settings)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

async fn settings_path(state: &AppState) -> anyhow::Result<PathBuf> {
    let app_handle = state
        .app_handle
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("app handle is not ready"))?;
    let resolver = app_handle.path();
    let base = resolver
        .app_local_data_dir()
        .or_else(|_| resolver.app_data_dir())
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(base.join("settings.json"))
}

#[derive(Debug, Serialize)]
struct TrafficEvent {
    message: TrafficData,
}

fn open_in_browser(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{
            UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL,
        };

        let op: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
        let wide_url: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                wide_url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        } as isize;

        if result <= 32 {
            return Err(anyhow!("无法打开浏览器，ShellExecute 错误码: {:?}", result));
        }
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }

    Ok(())
}

fn open_frontend(state: AppState) {
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

fn open_admin_frontend(state: AppState) {
    tauri::async_runtime::spawn(async move {
        match state.admin_server_url().await {
            Some(url) => {
                if let Err(err) = open_in_browser(&url) {
                    error!("打开配置管理界面失败: {err}");
                }
            }
            None => warn!("配置管理界面尚未就绪"),
        }
    });
}

async fn read_core_version_info(state: &AppState) -> anyhow::Result<(Option<String>, usize, bool)> {
    let vm = VersionManager::new()?;
    let installed = vm.list_installed().await?;
    let current = vm.get_default().await.ok();
    let installed_len = installed.len();
    let use_bundled = state.use_bundled_core().await || installed_len == 0;
    Ok((current, installed_len, use_bundled))
}

async fn update_mihomo_core(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let vm = VersionManager::new()?;
    let current = vm.get_default().await.ok();
    state
        .update_core_status_text("更新状态: 检查最新版本...")
        .await;
    state.update_core_network_text("网络: 检测中...").await;
    let info = match timeout(Duration::from_secs(30), fetch_latest(Channel::Stable)).await {
        Ok(Ok(info)) => info,
        Ok(Err(err)) => {
            state.update_core_network_text("网络: 不可用").await;
            return Err(anyhow!(err.to_string()));
        }
        Err(_) => {
            state.update_core_network_text("网络: 超时").await;
            return Err(anyhow!("获取最新稳定版超时"));
        }
    };
    let latest = info.version;
    state.update_core_network_text("网络: 正常").await;

    if current.as_deref() == Some(&latest) {
        state
            .update_core_status_text(format!("更新状态: 已是最新 ({latest})"))
            .await;
        return Ok(());
    }

    let installed = vm.list_installed().await?;
    let already_installed = installed.iter().any(|item| item.version == latest);
    if !already_installed {
        state
            .update_core_status_text(format!("更新状态: 下载 {latest}..."))
            .await;
        let (tx, mut rx) = mpsc::unbounded_channel::<DownloadProgress>();
        let state_for_progress = state.clone();
        let download_start = std::time::Instant::now();
        tauri::async_runtime::spawn(async move {
            let mut last_bytes = 0u64;
            let mut last_tick = std::time::Instant::now();
            while let Some(progress) = rx.recv().await {
                let total = progress.total;
                let downloaded = progress.downloaded;
                let now = std::time::Instant::now();
                let elapsed_secs = download_start.elapsed().as_secs_f32();
                let delta_bytes = downloaded.saturating_sub(last_bytes);
                let delta_secs = now.duration_since(last_tick).as_secs_f32();
                let speed = if delta_secs > 0.0 {
                    delta_bytes as f64 / delta_secs as f64
                } else {
                    0.0
                };
                last_bytes = downloaded;
                last_tick = now;
                let text = if let Some(total) = total {
                    let pct = (downloaded as f64 / total as f64) * 100.0;
                    format!(
                        "更新状态: 下载中 {:.1}% ({}/{}, {}/s, {:.0}s)",
                        pct,
                        format_bytes(downloaded),
                        format_bytes(total),
                        format_speed(speed),
                        elapsed_secs
                    )
                } else {
                    format!(
                        "更新状态: 下载中 {} ({}/s, {:.0}s)",
                        format_bytes(downloaded),
                        format_speed(speed),
                        elapsed_secs
                    )
                };
                state_for_progress.update_core_status_text(text).await;
            }
        });

        match timeout(
            Duration::from_secs(600),
            vm.install_with_progress(&latest, |progress| {
                let _ = tx.send(progress);
            }),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                state.update_core_status_text("更新状态: 下载失败").await;
                return Err(anyhow!(err.to_string()));
            }
            Err(_) => {
                state.update_core_status_text("更新状态: 下载超时").await;
                return Err(anyhow!("下载最新稳定版超时"));
            }
        }
    }

    state
        .update_core_status_text(format!("更新状态: 切换到 {latest}"))
        .await;
    state.set_use_bundled_core(false).await;
    vm.set_default(&latest).await?;
    rebuild_runtime(app, state).await?;

    state
        .update_core_status_text("更新状态: 清理旧版本...")
        .await;
    let installed_after = vm.list_installed().await?;
    for version in installed_after {
        if version.version != latest {
            if let Err(err) = vm.uninstall(&version.version).await {
                warn!("failed to remove old version {}: {err}", version.version);
            }
        }
    }

    state
        .update_core_status_text(format!("更新状态: 完成 ({latest})"))
        .await;
    Ok(())
}

async fn switch_core_version(
    app: &AppHandle,
    state: &AppState,
    version: &str,
) -> anyhow::Result<()> {
    let vm = VersionManager::new()?;
    state.set_use_bundled_core(false).await;
    vm.set_default(version).await?;
    rebuild_runtime(app, state).await?;
    state.refresh_core_version_info().await;
    Ok(())
}

async fn delete_core_version(version: &str) -> anyhow::Result<()> {
    let vm = VersionManager::new()?;
    vm.uninstall(version).await?;
    Ok(())
}

async fn refresh_tray_menu(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
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

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.2} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec <= 0.0 {
        return "0 B".to_string();
    }
    format_bytes(bytes_per_sec.round() as u64)
}

#[cfg(target_os = "windows")]
fn windows_shell_execute(editor: &str, args: &[String]) -> anyhow::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL,
    };

    let params = if args.is_empty() {
        None
    } else {
        Some(args_to_command_line(args))
    };
    let op: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let exe_w: Vec<u16> = OsStr::new(editor).encode_wide().chain(Some(0)).collect();
    let params_w: Option<Vec<u16>> = params
        .as_deref()
        .map(|p| OsStr::new(p).encode_wide().chain(Some(0)).collect());

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            exe_w.as_ptr(),
            params_w
                .as_ref()
                .map(|p| p.as_ptr())
                .unwrap_or(std::ptr::null()),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;

    if result <= 32 {
        return Err(anyhow!("ShellExecute 失败，错误码: {result}"));
    }
    Ok(())
}

fn confirm_dialog(message: &str, title: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDOK, MB_ICONWARNING, MB_OKCANCEL,
        };

        let wide_body: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();
        let wide_title: Vec<u16> = OsStr::new(title).encode_wide().chain(Some(0)).collect();

        unsafe { MessageBoxW(std::ptr::null_mut(), wide_body.as_ptr(), wide_title.as_ptr(), MB_OKCANCEL | MB_ICONWARNING) == IDOK }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (message, title);
        true
    }
}

async fn open_profile_in_editor(state: &AppState, name: &str) -> anyhow::Result<()> {
    let profile = load_profile_info(name).await?;
    let (editor, args) = resolve_editor_command(state).await?;
    let mut final_args = args;
    final_args.push(profile.path.clone());
    #[cfg(target_os = "windows")]
    {
        windows_shell_execute(&editor, &final_args)?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut command = Command::new(editor);
        command.args(final_args);
        command.spawn()?;
        Ok(())
    }
}

async fn resolve_editor_command(state: &AppState) -> anyhow::Result<(String, Vec<String>)> {
    if let Some(path) = state.editor_path().await {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return default_editor_command();
        }
        let is_path_like = trimmed.contains(['\\', '/']);
        if is_path_like {
            let candidate = PathBuf::from(trimmed);
            if !candidate.exists() {
                return Err(anyhow!("未找到编辑器路径: {trimmed}"));
            }
        } else if !is_command_available(trimmed) {
            return Err(anyhow!("未找到编辑器命令: {trimmed}"));
        }
        return Ok((trimmed.to_string(), Vec::new()));
    }
    default_editor_command()
}

fn default_editor_command() -> anyhow::Result<(String, Vec<String>)> {
    if is_vscode_available() {
        return Ok(("code".to_string(), vec!["-w".to_string()]));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(("notepad.exe".to_string(), Vec::new()));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(("open".to_string(), Vec::new()));
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Ok(("xdg-open".to_string(), Vec::new()))
    }
}

fn is_command_available(command: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return Command::new("where")
            .arg(command)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(command)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn is_vscode_available() -> bool {
    is_command_available("code")
}

fn sort_versions_desc(list: &mut [String]) {
    list.sort_by(|a, b| compare_versions_desc(a, b));
}

fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);
    match (va, vb) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim().trim_start_matches('v');
    let core = trimmed.split('-').next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

#[cfg(target_os = "windows")]
fn is_running_as_admin() -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(target_os = "windows"))]
fn is_running_as_admin() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn restart_as_admin(static_port: Option<u16>, admin_port: Option<u16>) -> anyhow::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL,
    };

    let exe = std::env::current_exe()?;
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(port) = static_port {
        args.push(format!("--static-port={port}"));
    }
    if let Some(port) = admin_port {
        args.push(format!("--admin-port={port}"));
    }
    let params = if args.is_empty() {
        None
    } else {
        Some(args_to_command_line(&args))
    };

    let op: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let exe_w: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let params_w: Option<Vec<u16>> = params
        .as_deref()
        .map(|p| OsStr::new(p).encode_wide().chain(Some(0)).collect());

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            exe_w.as_ptr(),
            params_w
                .as_ref()
                .map(|p| p.as_ptr())
                .unwrap_or(std::ptr::null()),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;

    if result <= 32 {
        return Err(anyhow!("ShellExecute 失败，错误码: {result}"));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn restart_as_admin(_static_port: Option<u16>, _admin_port: Option<u16>) -> anyhow::Result<()> {
    Err(anyhow!("以管理员身份重启仅支持 Windows"))
}

fn args_to_command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

#[derive(Debug, Clone, Copy)]
struct LaunchPorts {
    static_port: Option<u16>,
    admin_port: Option<u16>,
}

fn parse_launch_ports() -> LaunchPorts {
    let mut static_port = None;
    let mut admin_port = None;
    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("--static-port=") {
            static_port = value.parse::<u16>().ok();
        } else if let Some(value) = arg.strip_prefix("--admin-port=") {
            admin_port = value.parse::<u16>().ok();
        }
    }
    LaunchPorts {
        static_port,
        admin_port,
    }
}

fn extract_port_from_url(url: &str) -> Option<u16> {
    let host = url.split("://").nth(1)?;
    let host = host.split('/').next()?;
    let port = host.split(':').last()?;
    port.parse::<u16>().ok()
}

async fn wait_for_port_release(port: u16, timeout: Duration) {
    let start = std::time::Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    loop {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                drop(listener);
                break;
            }
            Err(_) => {
                if start.elapsed() >= timeout {
                    break;
                }
                sleep(Duration::from_millis(150)).await;
            }
        }
    }
}

const AUTOSTART_TASK_NAME: &str = "MihomoDespicableInfiltrator";

fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", AUTOSTART_TASK_NAME])
            .output();
        return output.map(|o| o.status.success()).unwrap_or(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

fn set_autostart_enabled(enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe()?;
            let task_cmd = format!("\"{}\"", exe.to_string_lossy());
            let status = Command::new("schtasks")
                .args([
                    "/Create",
                    "/F",
                    "/SC",
                    "ONLOGON",
                    "/RL",
                    "HIGHEST",
                    "/TN",
                    AUTOSTART_TASK_NAME,
                    "/TR",
                    &task_cmd,
                ])
                .status()?;
            if !status.success() {
                return Err(anyhow!("创建计划任务失败"));
            }
        } else if is_autostart_enabled() {
            let status = Command::new("schtasks")
                .args(["/Delete", "/TN", AUTOSTART_TASK_NAME, "/F"])
                .status()?;
            if !status.success() {
                return Err(anyhow!("删除计划任务失败"));
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err(anyhow!("开机自启仅支持 Windows"))
    }
}

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
    if let Some(items) = state.tray_info.read().await.as_ref() {
        if let Err(err) = items.autostart.set_checked(new_state) {
            warn!("failed to update autostart menu item: {err}");
        }
    }
    Ok(())
}

async fn handle_open_webui_toggle(state: AppState) -> anyhow::Result<()> {
    let current = state.open_webui_on_startup().await;
    let new_state = !current;
    state.set_open_webui_on_startup(new_state).await;
    if let Some(items) = state.tray_info.read().await.as_ref() {
        if let Err(err) = items.open_webui.set_checked(new_state) {
            warn!("failed to update open webui menu item: {err}");
        }
    }
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

fn create_tray(app: &AppHandle, state: AppState) -> tauri::Result<()> {
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

fn show_error_dialog(message: impl Into<String>) {
    let body = message.into();
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

        let wide_body: Vec<u16> = OsStr::new(&body).encode_wide().chain(Some(0)).collect();
        let wide_title: Vec<u16> = OsStr::new("Mihomo Despicable Infiltrator")
            .encode_wide()
            .chain(Some(0))
            .collect();

        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                wide_body.as_ptr(),
                wide_title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        warn!("startup issue: {body}");
    }
}

#[derive(Deserialize)]
struct SwitchProfilePayload {
    name: String,
}

#[derive(Deserialize)]
struct ImportProfilePayload {
    name: String,
    url: String,
    activate: Option<bool>,
}

#[derive(Deserialize)]
struct SaveProfilePayload {
    name: String,
    content: String,
    activate: Option<bool>,
}

#[derive(Deserialize)]
struct OpenProfilePayload {
    name: String,
}

#[derive(Deserialize)]
struct EditorConfigPayload {
    editor: Option<String>,
}

#[derive(Serialize)]
struct EditorConfigResponse {
    editor: Option<String>,
}

#[derive(Serialize)]
struct CoreVersionsResponse {
    current: Option<String>,
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct CoreActivatePayload {
    version: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

async fn list_profiles_http(
    AxumState(_state): AxumState<AdminServerState>,
) -> Result<Json<Vec<ProfileInfo>>, ApiError> {
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let profiles = manager
        .list_profiles()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .into_iter()
        .map(profile_to_info)
        .collect();
    Ok(Json(profiles))
}

async fn get_profile_http(
    AxumState(_state): AxumState<AdminServerState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<ProfileDetail>, ApiError> {
    let profile = load_profile_info(&name).await?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let content = manager
        .load(&profile.name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(ProfileDetail {
        name: profile.name,
        active: profile.active,
        path: profile.path,
        content,
    }))
}

async fn switch_profile_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<SwitchProfilePayload>,
) -> Result<StatusCode, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    switch_profile_internal(&state.app, &state.app_state, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn import_profile_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<ImportProfilePayload>,
) -> Result<Json<ProfileInfo>, ApiError> {
    let profile_name = ensure_valid_profile_name(&payload.name)?;
    if payload.url.trim().is_empty() {
        return Err(ApiError::bad_request("订阅链接不能为空"));
    }
    let profile = import_profile_from_url_internal(
        &state.app,
        &state.app_state,
        &state.http_client,
        &profile_name,
        &payload.url,
        payload.activate.unwrap_or(false),
    )
    .await?;
    Ok(Json(profile))
}

async fn save_profile_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<SaveProfilePayload>,
) -> Result<Json<ProfileInfo>, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    if payload.content.trim().is_empty() {
        return Err(ApiError::bad_request("配置内容不能为空"));
    }

    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    manager
        .save(&name, &payload.content)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    if payload.activate.unwrap_or(false) {
        manager
            .set_current(&name)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        rebuild_runtime(&state.app, &state.app_state).await?;
    }

    let info = load_profile_info(&name).await?;
    Ok(Json(info))
}

async fn delete_profile_http(
    AxumState(_state): AxumState<AdminServerState>,
    AxumPath(name): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    manager
        .delete_profile(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_editor_config_http(
    AxumState(state): AxumState<AdminServerState>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.app_state.editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

async fn set_editor_config_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<EditorConfigPayload>,
) -> Result<StatusCode, ApiError> {
    let editor = payload.editor.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });
    state.app_state.set_editor_path(editor).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn open_profile_in_editor_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<OpenProfilePayload>,
) -> Result<StatusCode, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    open_profile_in_editor(&state.app_state, &name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_core_versions_http(
    AxumState(_state): AxumState<AdminServerState>,
) -> Result<Json<CoreVersionsResponse>, ApiError> {
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let versions = vm
        .list_installed()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut list: Vec<String> = versions.into_iter().map(|v| v.version).collect();
    sort_versions_desc(&mut list);
    let current = vm.get_default().await.ok();
    Ok(Json(CoreVersionsResponse {
        current,
        versions: list,
    }))
}

async fn activate_core_version_http(
    AxumState(state): AxumState<AdminServerState>,
    Json(payload): Json<CoreActivatePayload>,
) -> Result<StatusCode, ApiError> {
    let version = payload.version.trim();
    if version.is_empty() {
        return Err(ApiError::bad_request("版本不能为空"));
    }
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    state.app_state.set_use_bundled_core(false).await;
    vm.set_default(version)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    rebuild_runtime(&state.app, &state.app_state).await?;
    state.app_state.refresh_core_version_info().await;
    Ok(StatusCode::NO_CONTENT)
}

fn apply_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        set_windows_system_proxy(endpoint)
    }
    #[cfg(not(target_os = "windows"))]
    {
        if endpoint.is_some() {
            Err(anyhow!("系统代理切换仅支持 Windows"))
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
fn set_windows_system_proxy(endpoint: Option<&str>) -> anyhow::Result<()> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        },
        System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
            REG_DWORD, REG_SZ,
        },
    };

    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let path = encode_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key);
        if status != ERROR_SUCCESS {
            return Err(anyhow!("RegOpenKeyExW failed: {status}"));
        }

        let enable: u32 = if endpoint.is_some() { 1 } else { 0 };
        let enable_ptr = &enable as *const u32 as *const u8;
        let proxy_enable_name = encode_wide("ProxyEnable");
        let enable_status = RegSetValueExW(
            key,
            proxy_enable_name.as_ptr(),
            0,
            REG_DWORD,
            enable_ptr,
            std::mem::size_of::<u32>() as u32,
        );
        if enable_status != ERROR_SUCCESS {
            RegCloseKey(key);
            return Err(anyhow!(
                "RegSetValueExW ProxyEnable failed: {enable_status}"
            ));
        }

        let server = endpoint.unwrap_or("");
        let server_w = encode_wide(server);
        let proxy_server_name = encode_wide("ProxyServer");
        let server_status = RegSetValueExW(
            key,
            proxy_server_name.as_ptr(),
            0,
            REG_SZ,
            server_w.as_ptr() as *const u8,
            (server_w.len() * 2) as u32,
        );
        RegCloseKey(key);
        if server_status != ERROR_SUCCESS {
            return Err(anyhow!(
                "RegSetValueExW ProxyServer failed: {server_status}"
            ));
        }

        InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }

    Ok(())
}

fn read_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    #[cfg(target_os = "windows")]
    {
        read_windows_system_proxy_state()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(SystemProxyState {
            enabled: false,
            endpoint: None,
        })
    }
}

#[cfg(target_os = "windows")]
fn read_windows_system_proxy_state() -> anyhow::Result<SystemProxyState> {
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
        },
    };

    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let path = encode_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut key,
        );
        if status != ERROR_SUCCESS {
            return Err(anyhow!("RegOpenKeyExW failed: {status}"));
        }

        let proxy_enable_name = encode_wide("ProxyEnable");
        let mut enable_value: u32 = 0;
        let mut enable_len = std::mem::size_of::<u32>() as u32;
        let mut enable_type: u32 = 0;
        let enable_status = RegQueryValueExW(
            key,
            proxy_enable_name.as_ptr(),
            std::ptr::null_mut(),
            &mut enable_type,
            &mut enable_value as *mut u32 as *mut u8,
            &mut enable_len,
        );
        let enabled = enable_status == ERROR_SUCCESS && enable_value != 0;

        let proxy_server_name = encode_wide("ProxyServer");
        let mut server_len: u32 = 0;
        let mut server_type: u32 = 0;
        let mut endpoint = None;
        let server_status = RegQueryValueExW(
            key,
            proxy_server_name.as_ptr(),
            std::ptr::null_mut(),
            &mut server_type,
            std::ptr::null_mut(),
            &mut server_len,
        );

        if server_status == ERROR_SUCCESS && server_len >= 2 {
            let mut buffer = vec![0u16; server_len as usize / 2];
            let mut len_copy = server_len;
            let read_status = RegQueryValueExW(
                key,
                proxy_server_name.as_ptr(),
                std::ptr::null_mut(),
                &mut server_type,
                buffer.as_mut_ptr() as *mut u8,
                &mut len_copy,
            );
            if read_status == ERROR_SUCCESS {
                if let Some(pos) = buffer.iter().position(|&c| c == 0) {
                    buffer.truncate(pos);
                }
                let text = String::from_utf16_lossy(&buffer);
                if !text.trim().is_empty() {
                    endpoint = Some(text);
                }
            }
        } else if server_status != ERROR_FILE_NOT_FOUND {
            warn!("读取 ProxyServer 失败，状态码: {server_status}");
        }

        RegCloseKey(key);

        Ok(SystemProxyState { enabled, endpoint })
    }
}

async fn resolve_binary(
    app: &AppHandle,
    state: &AppState,
    vm: &VersionManager,
) -> anyhow::Result<PathBuf> {
    if state.use_bundled_core().await {
        if let Some(path) = copy_bundled_binary(app).await? {
            return Ok(path);
        }
        warn!("bundled core not found, fallback to version manager");
    }
    let installed = vm.list_installed().await.unwrap_or_default();
    if !installed.is_empty() {
        if let Ok(default_version) = vm.get_default().await {
            if let Ok(path) = vm.get_binary_path(Some(&default_version)).await {
                return Ok(path);
            }
            warn!("default mihomo binary not found for {default_version}");
        }

        let mut versions: Vec<String> = installed.into_iter().map(|v| v.version).collect();
        sort_versions_desc(&mut versions);
        if let Some(latest) = versions.first() {
            if vm.set_default(latest).await.is_ok() {
                if let Ok(path) = vm.get_binary_path(Some(latest)).await {
                    return Ok(path);
                }
            }
            warn!("failed to use latest installed version {latest}, fallback to bundled");
        }
    }

    if let Some(path) = copy_bundled_binary(app).await? {
        return Ok(path);
    }

    let version = vm
        .install_channel(Channel::Stable)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    vm.set_default(&version)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    vm.get_binary_path(Some(&version))
        .await
        .map_err(|e| anyhow!(e.to_string()))
}

async fn copy_bundled_binary(app: &AppHandle) -> anyhow::Result<Option<PathBuf>> {
    #[cfg(not(windows))]
    {
        let _ = app;
        Ok(None)
    }

    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        let project_resource =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mihomo-windows-amd64-v3.exe");
        if project_resource.exists() {
            candidates.push(project_resource);
        }

        if let Ok(resource_path) = app
            .path()
            .resolve("bin/mihomo/mihomo.exe", BaseDirectory::Resource)
        {
            if resource_path.exists() {
                candidates.push(resource_path);
            }
        }

        let Some(source_path) = candidates.into_iter().find(|p| p.exists()) else {
            return Ok(None);
        };

        let resolver = app.path().clone();
        let data_dir = resolver
            .app_local_data_dir()
            .or_else(|_| resolver.app_data_dir())
            .map_err(|e| anyhow!(e.to_string()))?;

        let runtime_dir = data_dir.join("mihomo");
        tokio::fs::create_dir_all(&runtime_dir).await?;
        let target = runtime_dir.join("mihomo.exe");

        if !target.exists() {
            tokio::fs::copy(&source_path, &target).await?;
        }

        Ok(Some(target))
    }
}

#[cfg(target_os = "windows")]
fn encode_wide<S: AsRef<std::ffi::OsStr>>(input: S) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    input.as_ref().encode_wide().chain(Some(0)).collect()
}
