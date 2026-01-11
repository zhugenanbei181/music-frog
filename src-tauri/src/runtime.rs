use anyhow::anyhow;
use infiltrator_desktop::MihomoRuntime;
use infiltrator_core::admin_api::{
    AdminEvent, EVENT_REBUILD_FAILED, EVENT_REBUILD_FINISHED, EVENT_REBUILD_STARTED,
};
use log::{info, warn};
use mihomo_api::TrafficData;
use mihomo_config::ConfigManager;
use mihomo_version::VersionManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration, Instant};

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

pub(crate) fn spawn_runtime(app: AppHandle, state: AppState) {
    let app_handle_for_refresh = app.clone();
    let state_for_refresh = state.clone();
    let state_for_lock = state.clone();
    tauri::async_runtime::spawn(async move {
        // Acquire lock for startup to prevent race with early tray actions
        let _guard = state_for_lock.rebuild_lock.lock().await;
        
        match bootstrap_runtime(&app, &state).await {
            Ok(runtime) => {
                register_runtime(&app, &state, runtime).await;
                spawn_traffic_stream(app.clone(), state.clone());
                // Force a tray refresh now that runtime is ready
                if let Err(err) = refresh_tray_menu(&app_handle_for_refresh, &state_for_refresh).await {
                    warn!("initial tray menu refresh failed: {err}");
                }
            }
            Err(err) => {
                log::error!("failed to bootstrap mihomo runtime: {err:#}");
                crate::platform::show_error_dialog(format!("无法启动 mihomo 服务: {err:#}"));
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

pub(crate) async fn register_runtime(
    app: &AppHandle,
    state: &AppState,
    runtime: MihomoRuntime,
) {
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

pub(crate) async fn rebuild_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let _guard = state.rebuild_lock.lock().await;
    rebuild_runtime_without_lock(app, state).await
}

pub(crate) async fn rebuild_runtime_without_lock(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    state.emit_admin_event(AdminEvent::new(EVENT_REBUILD_STARTED));
    info!("runtime rebuild start");
    let result = async {
    let previous_runtime = state.runtime().await.ok();
    let previous_controller_port = previous_runtime
        .as_ref()
        .and_then(|runtime| extract_port_from_url(&runtime.controller_url));
    let mut previous_proxy_port = None;
    if let Some(runtime) = previous_runtime.as_ref() {
        if let Ok(Some(endpoint)) = runtime.http_proxy_endpoint().await {
            let endpoint = format!("http://{}", endpoint);
            previous_proxy_port = extract_port_from_url(&endpoint);
        }
    }
    info!(
        "runtime rebuild previous ports: controller={:?} proxy={:?}",
        previous_controller_port, previous_proxy_port
    );
    if let Some(runtime) = previous_runtime {
        info!("stopping previous mihomo runtime");
        if let Err(err) = runtime.shutdown().await {
            warn!("failed to stop running mihomo instance: {err}");
        }
    }
    if let Some(port) = previous_controller_port {
        info!("waiting for controller port release: {port}");
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            warn!("controller port still occupied after wait: {port}");
            let manager = ConfigManager::new().map_err(|e| anyhow!(e.to_string()))?;
            match manager.rotate_external_controller().await {
                Ok(new_url) => {
                    warn!(
                        "controller port {} still occupied, rotated external controller to {}",
                        port, new_url
                    );
                }
                Err(err) => {
                    return Err(anyhow!(
                        "控制接口端口 {} 仍被占用，请确认旧进程已退出或关闭占用程序 ({})",
                        port,
                        err
                    ));
                }
            }
        } else {
            info!("controller port released: {port}");
        }
    }
    if let Some(port) = previous_proxy_port {
        info!("waiting for proxy port release: {port}");
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            warn!("proxy port still occupied after wait: {port}");
            let manager = ConfigManager::new().map_err(|e| anyhow!(e.to_string()))?;
            manager
                .ensure_proxy_ports()
                .await
                .map_err(|e| anyhow!(e.to_string()))?;
            warn!(
                "proxy port {} still occupied, switched to available port in config",
                port
            );
        } else {
            info!("proxy port released: {port}");
        }
    }
    info!("bootstrapping new mihomo runtime");
    let runtime = bootstrap_runtime(app, state).await?;
    info!(
        "mihomo runtime bootstrapped: controller={}",
        runtime.controller_url
    );
    register_runtime(app, state, runtime).await;
    info!("mihomo runtime registered");
    if state.is_system_proxy_enabled().await {
        if let Ok(runtime) = state.runtime().await {
            match runtime.http_proxy_endpoint().await {
                Ok(Some(endpoint)) => {
                    if let Err(err) = apply_system_proxy(Some(&endpoint)) {
                        warn!("failed to refresh system proxy: {err}");
                    } else {
                        state
                            .set_system_proxy_state(true, Some(endpoint))
                            .await;
                    }
                }
                Ok(None) => {
                    if let Err(err) = apply_system_proxy(None) {
                        warn!("failed to disable system proxy: {err}");
                    }
                    state.set_system_proxy_state(false, None).await;
                }
                Err(err) => {
                    warn!("failed to read proxy endpoint: {err}");
                }
            }
        }
    }
    if let Err(err) = refresh_profile_switch_submenu(app, state).await {
        warn!("failed to refresh profile switch submenu after rebuild: {err}");
    }
    if let Err(err) = refresh_proxy_groups_submenu(app, state).await {
        warn!("failed to refresh proxy groups submenu after rebuild: {err}");
    }
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

async fn bootstrap_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<MihomoRuntime> {
    let vm = VersionManager::new()?;
    let data_dir = app_data_dir(app)?;
    let bundled_candidates = bundled_core_candidates(app);
    let installed = vm.list_installed().await.unwrap_or_default();
    let use_bundled = state.use_bundled_core().await || installed.is_empty();

    let max_retries = 3;
    let mut last_err = anyhow!("unknown error");

    for attempt in 0..max_retries {
        info!("bootstrap attempt {}/{}", attempt + 1, max_retries);
        match MihomoRuntime::bootstrap(&vm, use_bundled, &bundled_candidates, &data_dir).await {
            Ok(runtime) => match wait_for_controller_ready(&runtime).await {
                Ok(_) => {
                    if installed.is_empty() {
                        state.set_use_bundled_core(true).await;
                    }
                    info!(
                        "mihomo controller ready: {}",
                        runtime.controller_url
                    );
                    return Ok(runtime);
                }
                Err(err) => {
                    warn!(
                        "startup attempt {}/{} failed during readiness check: {err:#}",
                        attempt + 1,
                        max_retries
                    );
                    last_err = err;
                    // shutdown if it was partially started
                    let _ = runtime.shutdown().await;
                }
            },
            Err(err) => {
                warn!(
                    "startup attempt {}/{} failed during bootstrap: {err:#}",
                    attempt + 1,
                    max_retries
                );
                last_err = err;
            }
        }

        // Prepare for retry: rotate port if we have retries left
        if attempt < max_retries - 1 {
            match ConfigManager::new() {
                Ok(cm) => {
                    if let Err(e) = cm.rotate_external_controller().await {
                        warn!("failed to rotate external controller port: {e}");
                    } else {
                        // wait a bit for old port to be technically released if it was bound
                        sleep(Duration::from_millis(500)).await;
                    }
                }
                Err(e) => warn!("failed to create config manager for rotation: {e}"),
            }
        }
    }

    Err(last_err)
}

async fn wait_for_controller_ready(runtime: &MihomoRuntime) -> anyhow::Result<()> {
    info!(
        "waiting for controller ready: {}",
        runtime.controller_url
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last_err = None;
    let mut attempt = 0u32;

    while Instant::now() < deadline {
        attempt = attempt.saturating_add(1);
        match runtime.client().get_version().await {
            Ok(_) => {
                info!(
                    "controller ready after {} attempts: {}",
                    attempt,
                    runtime.controller_url
                );
                return Ok(());
            }
            Err(err) => {
                // Check if the process is still alive while we wait
                if !runtime.is_running().await {
                    warn!(
                        "mihomo process exited before controller ready: {}",
                        runtime.controller_url
                    );
                    return Err(anyhow!("内核进程已退出，启动失败"));
                }
                warn!(
                    "controller not ready (attempt {}): {} ({})",
                    attempt,
                    runtime.controller_url,
                    err
                );
                last_err = Some(err);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    warn!(
        "controller readiness timeout after {} attempts: {}",
        attempt,
        runtime.controller_url
    );
    Err(anyhow!(
        "控制接口未就绪 ({}): {}",
        runtime.controller_url,
        last_err
            .map(|err| err.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    ))
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
