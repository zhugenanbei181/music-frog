use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use mihomo_config::manager::paths::resolve_configs_dir_in;

use dav_client::client::WebDavClient;
use mihomo_platform::paths::get_home_dir;
use state_store::StateStore;
use sync_engine::{SyncPlanner, executor::SyncExecutor};

use crate::admin_api::state::AdminApiContext;
use infiltrator_domain::settings::WebDavConfig;

/// Sync result summary for notification purposes
#[derive(Debug, Default)]
pub struct SyncSummary {
    pub success_count: usize,
    pub failed_count: usize,
    pub total_actions: usize,
}

/// `configs_dir` 来自 app settings（调用方从其持有的 settings 透传），
/// 与 profile 存储共用同一重定向解析：`INFILTRATOR_CONFIGS_DIR` 环境变量 >
/// settings `configs_dir` > `<home>/configs`。
///
/// 密码解析：`config.password` 非空时直接使用（显式构造的调用方），为空时
/// 回退 OS keyring——load_settings 迁移后磁盘与内存镜像都不再携带明文。
pub async fn run_sync_tick<C: AdminApiContext>(
    ctx: &C,
    config: &WebDavConfig,
    configs_dir: Option<&str>,
) -> Result<SyncSummary> {
    if !config.enabled {
        return Ok(SyncSummary::default());
    }

    if config.url.is_empty() {
        return Err(anyhow!("WebDAV URL is empty"));
    }

    info!("Starting WebDAV sync tick...");

    let password = if config.password.is_empty() {
        ctx.webdav_password().await.unwrap_or_default()
    } else {
        config.password.clone()
    };

    // 1. 初始化组件 - 带有错误上下文
    let dav = WebDavClient::new(&config.url, &config.username, &password)
        .context("Failed to create WebDAV client")?;

    // 定位数据目录：sync 扫描根必须与 profile 存储目录一致（云同步重定向）。
    let home = get_home_dir().map_err(|e| anyhow!("Failed to get home directory: {}", e))?;
    let local_root = resolve_configs_dir_in(configs_dir, &home)
        .map_err(|e| anyhow!("Failed to resolve configs directory: {}", e))?;

    // 确保本地目录存在
    if !local_root.exists() {
        tokio::fs::create_dir_all(&local_root)
            .await
            .context("Failed to create local configs directory")?;
    }

    let db_path = home.join("sync_state.db").to_string_lossy().to_string();

    let store = StateStore::new(&db_path)
        .await
        .context("Failed to open sync state database")?;

    // 2. 生成计划
    let planner = SyncPlanner::new(
        local_root.clone(),
        "/".to_string(), // 远端根路径
        &dav,
        &store,
    );

    let actions = match planner.build_plan().await {
        Ok(actions) => actions,
        Err(err) => {
            // 网络错误或远端不可达时，记录但不panic
            warn!("Failed to build sync plan: {err:#}");
            return Err(err.context("Failed to build sync plan"));
        }
    };

    if actions.is_empty() {
        info!("No sync actions needed.");
        return Ok(SyncSummary::default());
    }

    let total_actions = actions.len();
    info!("Found {} sync actions to perform.", total_actions);

    // 3. 执行动作 - 统计成功/失败
    let executor = SyncExecutor::new(&dav, &store);
    let mut success_count = 0usize;
    let mut failed_count = 0usize;

    for action in actions {
        match executor.execute(action).await {
            Ok(()) => success_count = success_count.saturating_add(1),
            Err(err) => {
                warn!("Failed to execute sync action: {err:#}");
                failed_count = failed_count.saturating_add(1);
            }
        }
    }

    info!(
        "WebDAV sync completed: {} success, {} failed.",
        success_count, failed_count
    );

    Ok(SyncSummary {
        success_count,
        failed_count,
        total_actions,
    })
}

#[cfg(test)]
mod sync_test;
