use anyhow::anyhow;
use despicable_infiltrator_core::MihomoRuntime;
use log::warn;
use mihomo_rs::{core::TrafficData, version::VersionManager};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration, Instant};

use crate::{
    app_state::AppState,
    paths::{app_data_dir, bundled_core_candidates},
    system_proxy::apply_system_proxy,
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
    tauri::async_runtime::spawn(async move {
        match bootstrap_runtime(&app, &state).await {
            Ok(runtime) => {
                register_runtime(&app, &state, runtime).await;
                spawn_traffic_stream(app.clone(), state.clone());
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
    if let Some(runtime) = previous_runtime {
        if let Err(err) = runtime.shutdown().await {
            warn!("failed to stop running mihomo instance: {err}");
        }
    }
    if let Some(port) = previous_controller_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            return Err(anyhow!(
                "控制接口端口 {} 仍被占用，请确认旧进程已退出或关闭占用程序",
                port
            ));
        }
    }
    if let Some(port) = previous_proxy_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
        if !is_port_available(port).await {
            return Err(anyhow!(
                "代理端口 {} 仍被占用，请确认旧进程已退出或关闭占用程序",
                port
            ));
        }
    }
    let runtime = bootstrap_runtime(app, state).await?;
    register_runtime(app, state, runtime).await;
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
    Ok(())
}

async fn bootstrap_runtime(app: &AppHandle, state: &AppState) -> anyhow::Result<MihomoRuntime> {
    let vm = VersionManager::new()?;
    let data_dir = app_data_dir(app)?;
    let bundled_candidates = bundled_core_candidates(app);
    let installed = vm.list_installed().await.unwrap_or_default();
    let use_bundled = state.use_bundled_core().await || installed.is_empty();
    let runtime =
        MihomoRuntime::bootstrap(&vm, use_bundled, &bundled_candidates, &data_dir).await?;
    wait_for_controller_ready(&runtime).await?;
    if installed.is_empty() {
        state.set_use_bundled_core(true).await;
    }
    Ok(runtime)
}

async fn wait_for_controller_ready(runtime: &MihomoRuntime) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last_err = None;
    while Instant::now() < deadline {
        match runtime.client().get_version().await {
            Ok(_) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Err(anyhow!(
        "控制接口未就绪: {}",
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
