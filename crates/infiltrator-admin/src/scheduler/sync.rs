use anyhow::Result;
use log::info;

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
    let application = ctx.sync_application().await?;
    let report = application
        .sync(config.clone(), configs_dir.map(str::to_owned))
        .await
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    info!(
        "WebDAV sync completed: {} success, {} failed.",
        report.success_count, report.failed_count
    );
    Ok(SyncSummary {
        success_count: report.success_count as usize,
        failed_count: report.failed_count as usize,
        total_actions: report.total_actions as usize,
    })
}

#[cfg(test)]
mod sync_test;
