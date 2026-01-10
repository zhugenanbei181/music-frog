use anyhow::anyhow;
use infiltrator_core::profiles;
use log::warn;
use mihomo_platform::get_home_dir;
use tauri::AppHandle;
use tokio::{fs, time::Duration};

use crate::{
    app_state::AppState,
    autostart::{is_autostart_enabled, set_autostart_enabled},
    frontend::spawn_frontends,
    paths::app_data_dir,
    runtime::rebuild_runtime_without_lock,
    settings::reset_settings,
    utils::wait_for_port_release,
};

pub(crate) async fn factory_reset(app: &AppHandle, state: &AppState) -> anyhow::Result<()> {
    let _guard = state.rebuild_lock.lock().await;

    let (static_port, admin_port) = state.current_ports().await;
    state.shutdown_all().await;
    if let Some(port) = static_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
    }
    if let Some(port) = admin_port {
        wait_for_port_release(port, Duration::from_secs(5)).await;
    }

    if is_autostart_enabled() {
        if let Err(err) = set_autostart_enabled(false) {
            warn!("failed to disable autostart during factory reset: {err}");
        }
        state.set_autostart_checked(false).await;
    }

    reset_settings(state).await?;
    clear_app_logs(app).await?;
    clear_mihomo_home().await?;
    profiles::reset_profiles_to_default().await?;
    state.set_use_bundled_core(true).await;
    state.set_open_webui_checked(false).await;

    rebuild_runtime_without_lock(app, state).await?;
    spawn_frontends(app.clone(), state.clone(), static_port, admin_port);
    state.refresh_core_version_info().await;
    Ok(())
}

async fn clear_mihomo_home() -> anyhow::Result<()> {
    let home = get_home_dir().map_err(|err| anyhow!(err.to_string()))?;
    if home.exists() {
        fs::remove_dir_all(&home).await?;
    }
    Ok(())
}

async fn clear_app_logs(app: &AppHandle) -> anyhow::Result<()> {
    let base = app_data_dir(app)?;
    let logs_dir = base.join("logs");
    if logs_dir.exists() {
        fs::remove_dir_all(&logs_dir).await?;
    }
    Ok(())
}
