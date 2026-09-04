//! configs 目录重定向的统一构造入口：把 settings 的 `configs_dir` 字段喂给
//! `ConfigManager`，供所有 update/app 路径复用。
//!
//! 约束：解析优先级为 `INFILTRATOR_CONFIGS_DIR` env、`AppSettings.configs_dir`、
//! `<home>/configs` 依次回退，前两级比较在 `mihomo-config` 内部完成；settings
//! 读不到（无 home、路径推导失败、文件损坏）时按未设置处理。

use std::path::PathBuf;

use infiltrator_core::error::InfiltratorError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_platform::paths::get_home_dir;

/// settings 的 `configs_dir` 覆盖；任何读取失败一律回退 `None`。
async fn settings_configs_dir() -> Option<String> {
    let home = mihomo_platform::paths::get_home_dir().ok()?;
    let path = infiltrator_core::settings::settings_path(&home).ok()?;
    let settings = infiltrator_core::settings::load_settings(&path)
        .await
        .ok()?;
    settings.configs_dir
}

/// 构造感知 settings `configs_dir` 的 [`ConfigManager`]（env 优先级不变）。
pub async fn config_manager() -> Result<ConfigManager<DefaultCredentialStore>, InfiltratorError> {
    let home = get_home_dir().map_err(InfiltratorError::from)?;
    let configs_dir = settings_configs_dir().await;
    ConfigManager::with_home_configs_dir_and_store(
        home,
        configs_dir.as_deref(),
        DefaultCredentialStore::default(),
    )
    .map_err(InfiltratorError::from)
}

/// 解析后的 configs 目录（env > settings 字段 > `<home>/configs`），供
/// profile_options / 快照 / MRS 扫描等需要目录本身（而非 manager）的路径用，
/// 避免与 [`config_manager`] 的解析结果分叉。
pub async fn configs_dir() -> Result<PathBuf, InfiltratorError> {
    let home = get_home_dir().map_err(InfiltratorError::from)?;
    let configs_dir = settings_configs_dir().await;
    mihomo_config::manager::paths::resolve_configs_dir_in(configs_dir.as_deref(), &home)
        .map_err(InfiltratorError::from)
}
