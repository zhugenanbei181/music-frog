use anyhow::anyhow;
use infiltrator_desktop::MihomoRuntime;
use infiltrator_admin::{
    AdminEvent, EVENT_REBUILD_FAILED, EVENT_REBUILD_FINISHED, EVENT_REBUILD_STARTED,
};
use log::{info, warn};
use mihomo_api::TrafficData;
use mihomo_config::ConfigManager;
use mihomo_version::VersionManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration, Instant};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::{
    app_state::AppState,
    paths::{app_data_dir, bundled_core_candidates},
    system_proxy::apply_system_proxy,
    tray::{refresh_profile_switch_submenu, refresh_proxy_groups_submenu, refresh_tray_menu},
    utils::{extract_port_from_url, is_port_available, wait_for_port_release},
};

#[derive(Debug, Serialize)]
struct TrafficEvent {
    message: TrafficData,
}

#[derive(Debug, Clone, Serialize)]
struct ReadyPayload {
    controller: String,
    config_path: String,
}

use std::fmt::Debug;

#[async_trait]
pub trait RuntimeHandle: Send + Sync + Debug {
    async fn wait_for_ready(&self) -> anyhow::Result<()>;
    async fn shutdown(&self) -> anyhow::Result<()>;
    fn into_runtime(self: Box<Self>) -> MihomoRuntime;
}

pub struct RealRuntimeHandle(MihomoRuntime);

impl Debug for RealRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealRuntimeHandle")
            .field("controller_url", &self.0.controller_url)
            .finish()
    }
}

#[async_trait]
impl RuntimeHandle for RealRuntimeHandle {
    async fn wait_for_ready(&self) -> anyhow::Result<()> {
        wait_for_controller_ready(&self.0).await
    }
    async fn shutdown(&self) -> anyhow::Result<()> {
        self.0.shutdown().await
    }
    fn into_runtime(self: Box<Self>) -> MihomoRuntime {
        self.0
    }
}

#[async_trait]
pub trait BootstrapContext: Send + Sync {
    async fn bootstrap_attempt(&self) -> anyhow::Result<Box<dyn RuntimeHandle>>;
    async fn rotate_port(&self) -> anyhow::Result<()>;
    async fn on_success(&self);
}

pub(crate) async fn run_bootstrap_loop<C: BootstrapContext>(ctx: &C) -> anyhow::Result<Box<dyn RuntimeHandle>> {
    let max_retries = 3;
    let mut last_err = anyhow!("unknown error");

    for attempt in 0..max_retries {
        info!("bootstrap attempt {}/{}", attempt + 1, max_retries);
        match ctx.bootstrap_attempt().await {
            Ok(handle) => match handle.wait_for_ready().await {
                Ok(_) => {
                    ctx.on_success().await;
                    return Ok(handle);
                }
                Err(err) => {
                    warn!("startup attempt {}/{} failed during readiness check: {err:#}", attempt + 1, max_retries);
                    last_err = err;
                    let _ = handle.shutdown().await;
                }
            },
            Err(err) => {
                warn!("startup attempt {}/{} failed during bootstrap: {err:#}", attempt + 1, max_retries);
                last_err = err;
            }
        }

        if attempt < max_retries - 1 {
            if let Err(e) = ctx.rotate_port().await {
                warn!("failed to rotate port: {e}");
            } else {
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
    Err(last_err)
}

struct RealBootstrapContext<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
}

#[async_trait]
impl<'a> BootstrapContext for RealBootstrapContext<'a> {
    async fn bootstrap_attempt(&self) -> anyhow::Result<Box<dyn RuntimeHandle>> {
        let vm = VersionManager::new()?;
        let data_dir = app_data_dir(self.app)?;
        let bundled_candidates = bundled_core_candidates(self.app);
        let installed = vm.list_installed().await.unwrap_or_default();
        let use_bundled = self.state.use_bundled_core().await || installed.is_empty();
        
        let r = MihomoRuntime::bootstrap(&vm, use_bundled, &bundled_candidates, &data_dir).await?;
        Ok(Box::new(RealRuntimeHandle(r)))
    }
    async fn rotate_port(&self) -> anyhow::Result<()> {
        let cm = ConfigManager::new().map_err(|e| anyhow!(e.to_string()))?;
        cm.rotate_external_controller().await?;
        Ok(())
    }
    async fn on_success(&self) {
        let vm = VersionManager::new().ok();
        let installed = if let Some(vm) = vm {
            vm.list_installed().await.unwrap_or_default()
        } else {
            vec![]
        };
        if installed.is_empty() {
            self.state.set_use_bundled_core(true).await;
        }
    }
}

pub(crate) async fn bootstrap_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<Box<dyn RuntimeHandle>> {
    let ctx = RealBootstrapContext { app, state };
    run_bootstrap_loop(&ctx).await
}

pub(crate) fn spawn_runtime(app: AppHandle, state: AppState) {
    let app_handle_for_refresh = app.clone();
    let state_for_refresh = state.clone();
    let state_for_lock = state.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = state_for_lock.rebuild_lock.lock().await;
        
        match bootstrap_runtime(&app, &state).await {
            Ok(handle) => {
                let runtime = handle.into_runtime();
                register_runtime(&app, &state, runtime).await;
                spawn_traffic_stream(app.clone(), state.clone());
                spawn_config_monitor(state.clone());
                if let Err(err) = refresh_tray_menu(&app_handle_for_refresh, &state_for_refresh).await {
                    warn!("initial tray menu refresh failed: {err}");
                }
            }
            Err(err) => {
                log::error!("failed to bootstrap mihomo runtime: {err:#}");
                crate::platform::show_error_dialog(format!("无法启动 mihomo 服务: {err:#}"));
                state.update_controller_info_text(format!("控制接口: 启动失败 ({err})")).await;
                if let Err(e) = app.emit("mihomo://error", err.to_string()) {
                    warn!("failed to emit runtime error event: {e}");
                }
            }
        }
    });
}

fn spawn_config_monitor(state: AppState) {
    tauri::async_runtime::spawn(async move {
        let mut last_tun_enabled = None;
        loop {
            sleep(Duration::from_secs(5)).await;
            let runtime = match state.runtime().await {
                Ok(r) => r,
                Err(_) => continue,
            };

            match runtime.client().get_config().await {
                Ok(config) => {
                    let current_tun = config.tun.as_ref()
                        .and_then(|t| t.get("enable"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    
                    if Some(current_tun) != last_tun_enabled {
                        if last_tun_enabled.is_some() {
                            info!("detected external TUN state change: {}", current_tun);
                            state.set_tun_enabled(current_tun).await;
                            state.update_tun_checked(current_tun).await;
                            state.emit_admin_event(infiltrator_admin::AdminEvent::new(infiltrator_admin::EVENT_TUN_CHANGED));
                        }
                        last_tun_enabled = Some(current_tun);
                    }
                }
                Err(err) => {
                    warn!("config monitor failed to get config: {err}");
                }
            }
        }
    });
}

pub(crate) async fn register_runtime(
    app: &AppHandle,
    state: &AppState,
    runtime: MihomoRuntime,
) {
    let controller = runtime.controller_url.clone();
    let config_path = runtime.config_path.to_string_lossy().to_string();
    state.set_runtime(runtime).await;
    state.update_controller_info_text(format!("控制接口: {controller}")).await;
    if let Err(err) = app.emit("mihomo://ready", ReadyPayload { controller, config_path }) {
        warn!("failed to emit ready event: {err}");
    }
}

pub(crate) async fn rebuild_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let _guard = state.rebuild_lock.lock().await;
    rebuild_runtime_without_lock(app, state).await
}

pub(crate) async fn rebuild_runtime_without_lock(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    state.emit_admin_event(AdminEvent::new(EVENT_REBUILD_STARTED));
    info!("runtime rebuild start");
    let result: anyhow::Result<()> = async {
    let previous_runtime = state.runtime().await.ok();
    let previous_controller_port = previous_runtime
        .as_ref()
        .and_then(|runtime| extract_port_from_url(&runtime.controller_url));
    let mut previous_proxy_port = None;
    if let Some(runtime) = previous_runtime.as_ref()
        && let Ok(Some(endpoint)) = runtime.http_proxy_endpoint().await {
            let endpoint = format!("http://{}", endpoint);
            previous_proxy_port = extract_port_from_url(&endpoint);
        }
    if let Some(runtime) = previous_runtime {
        if let Err(err) = runtime.shutdown().await {
            warn!("failed to stop running mihomo instance: {err}");
        }
    }
    if let Some(port) = previous_controller_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            let manager = ConfigManager::new().map_err(|e| anyhow!(e.to_string()))?;
            let _ = manager.rotate_external_controller().await;
        }
    }
    if let Some(port) = previous_proxy_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            let manager = ConfigManager::new().map_err(|e| anyhow!(e.to_string()))?;
            let _ = manager.ensure_proxy_ports().await;
        }
    }
    let handle = bootstrap_runtime(app, state).await?;
    let runtime = handle.into_runtime();
    register_runtime(app, state, runtime).await;
    
    if state.is_system_proxy_enabled().await
        && let Ok(runtime) = state.runtime().await {
            if let Ok(Some(endpoint)) = runtime.http_proxy_endpoint().await {
                if let Err(_) = apply_system_proxy(Some(&endpoint)) {
                } else {
                    state.set_system_proxy_state(true, Some(endpoint)).await;
                }
            }
        }
    let _ = refresh_profile_switch_submenu(app, state).await;
    let _ = refresh_proxy_groups_submenu(app, state).await;
    Ok(())
    }
    .await;

    match &result {
        Ok(()) => {
            info!("runtime rebuild finished");
            state.emit_admin_event(AdminEvent::new(EVENT_REBUILD_FINISHED));
        }
        Err(err) => state.emit_admin_event(
            AdminEvent::new(EVENT_REBUILD_FAILED).with_detail(err.to_string()),
        ),
    }

    result
}

async fn wait_for_controller_ready(runtime: &MihomoRuntime) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last_err = None;

    while Instant::now() < deadline {
        match runtime.client().get_version().await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if !runtime.is_running().await {
                    return Err(anyhow!("内核进程已退出"));
                }
                last_err = Some(err);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(anyhow!("控制接口未就绪: {}", last_err.map(|e| e.to_string()).unwrap_or_default()))
}

fn spawn_traffic_stream(app: AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let client = match state.runtime().await {
                Ok(runtime) => runtime.client(),
                Err(_) => {
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            if let Ok(mut rx) = client.stream_traffic().await {
                while let Some(message) = rx.recv().await {
                    let _ = app.emit("mihomo://traffic", &TrafficEvent { message });
                }
            }
            sleep(Duration::from_secs(3)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug)]
    struct MockHandle {
        ready_res: bool, // Store as bool to satisfy Debug easily
        shutdown_called: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl RuntimeHandle for MockHandle {
        async fn wait_for_ready(&self) -> anyhow::Result<()> {
            if self.ready_res { Ok(()) } else { Err(anyhow!("not ready")) }
        }
        async fn shutdown(&self) -> anyhow::Result<()> {
            *self.shutdown_called.lock().await = true;
            Ok(())
        }
        fn into_runtime(self: Box<Self>) -> MihomoRuntime {
            panic!("MockHandle into_runtime should not be called")
        }
    }

    struct MockCtx {
        bootstrap_results: Arc<Mutex<Vec<anyhow::Result<bool>>>>, // true=ready, false=not ready, Err=bootstrap err
        rotate_called: Arc<Mutex<u32>>,
        success_called: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl BootstrapContext for MockCtx {
        async fn bootstrap_attempt(&self) -> anyhow::Result<Box<dyn RuntimeHandle>> {
            let res = self.bootstrap_results.lock().await.remove(0);
            match res {
                Ok(ready) => Ok(Box::new(MockHandle { ready_res: ready, shutdown_called: Arc::new(Mutex::new(false)) })),
                Err(e) => Err(e),
            }
        }
        async fn rotate_port(&self) -> anyhow::Result<()> {
            *self.rotate_called.lock().await += 1;
            Ok(())
        }
        async fn on_success(&self) {
            *self.success_called.lock().await = true;
        }
    }

    #[tokio::test]
    async fn test_bootstrap_loop_success_first_try() {
        let ctx = MockCtx {
            bootstrap_results: Arc::new(Mutex::new(vec![Ok(true)])),
            rotate_called: Arc::new(Mutex::new(0)),
            success_called: Arc::new(Mutex::new(false)),
        };
        let res = run_bootstrap_loop(&ctx).await;
        assert!(res.is_ok());
        assert_eq!(*ctx.rotate_called.lock().await, 0);
        assert!(*ctx.success_called.lock().await);
    }

    #[tokio::test]
    async fn test_bootstrap_loop_retry_and_success() {
        let ctx = MockCtx {
            bootstrap_results: Arc::new(Mutex::new(vec![Err(anyhow!("fail")), Ok(false), Ok(true)])),
            rotate_called: Arc::new(Mutex::new(0)),
            success_called: Arc::new(Mutex::new(false)),
        };
        let res = run_bootstrap_loop(&ctx).await;
        assert!(res.is_ok());
        assert_eq!(*ctx.rotate_called.lock().await, 2);
        assert!(*ctx.success_called.lock().await);
    }

    #[tokio::test]
    async fn test_bootstrap_loop_all_fail() {
        let ctx = MockCtx {
            bootstrap_results: Arc::new(Mutex::new(vec![Err(anyhow!("1")), Err(anyhow!("2")), Err(anyhow!("3"))])),
            rotate_called: Arc::new(Mutex::new(0)),
            success_called: Arc::new(Mutex::new(false)),
        };
        let res = run_bootstrap_loop(&ctx).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "3");
        assert_eq!(*ctx.rotate_called.lock().await, 2);
        assert!(!*ctx.success_called.lock().await);
    }
}