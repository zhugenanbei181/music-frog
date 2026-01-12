use anyhow::{anyhow, Context, Result};
use log::{info, warn};

use dav_client::client::WebDavClient;
use state_store::StateStore;
use sync_engine::{SyncPlanner, executor::SyncExecutor};
use mihomo_platform::get_home_dir;

use crate::admin_api::AdminApiContext;
use infiltrator_core::settings::WebDavConfig;

/// Sync result summary for notification purposes
#[derive(Debug, Default)]
pub struct SyncSummary {
    pub success_count: usize,
    pub failed_count: usize,
    pub total_actions: usize,
}

pub async fn run_sync_tick<C: AdminApiContext>(
    _ctx: &C,
    config: &WebDavConfig,
) -> Result<SyncSummary> {
    if !config.enabled {
        return Ok(SyncSummary::default());
    }
    
    if config.url.is_empty() {
        return Err(anyhow!("WebDAV URL is empty"));
    }

    info!("Starting WebDAV sync tick...");

    // 1. 初始化组件 - 带有错误上下文
    let dav = WebDavClient::new(&config.url, &config.username, &config.password)
        .context("Failed to create WebDAV client")?;
    
    // 定位数据目录
    let home = get_home_dir().map_err(|e| anyhow!("Failed to get home directory: {}", e))?;
    let local_root = home.join("configs");
    
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
        &store
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

    info!("WebDAV sync completed: {} success, {} failed.", success_count, failed_count);
    
    Ok(SyncSummary {
        success_count,
        failed_count,
        total_actions,
    })
}
