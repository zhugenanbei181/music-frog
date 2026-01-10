use anyhow::anyhow;
use mihomo_version::{channel::fetch_latest, download::DownloadProgress, Channel, VersionManager};
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};

use crate::{app_state::AppState, runtime::rebuild_runtime};

pub(crate) async fn update_mihomo_core(app: &tauri::AppHandle, state: &AppState) -> anyhow::Result<()> {
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
                log::warn!("failed to remove old version {}: {err}", version.version);
            }
        }
    }

    state
        .update_core_status_text(format!("更新状态: 完成 ({latest})"))
        .await;
    Ok(())
}

pub(crate) async fn switch_core_version(
    app: &tauri::AppHandle,
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

pub(crate) async fn delete_core_version(version: &str) -> anyhow::Result<()> {
    let vm = VersionManager::new()?;
    vm.uninstall(version).await?;
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
