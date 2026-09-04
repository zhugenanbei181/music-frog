//! Profile persistence and configs-directory port access for the Iced host.
//!
//! The concrete ConfigManager, keyring and settings resolution stay behind
//! the core adapter; this module exposes only the port object to UI handlers.

use std::path::PathBuf;
use std::sync::Arc;

use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::profile_store::ProfileStore;

/// Construct the host's profile persistence port.
pub async fn config_manager() -> Result<Arc<dyn ProfileStore>, InfiltratorError> {
    infiltrator_desktop::storage::profile_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))
}

/// 解析后的 configs 目录（env > settings 字段 > `<home>/configs`），供
/// profile_options / 快照 / MRS 扫描等需要目录本身（而非 manager）的路径用，
/// 避免与 [`config_manager`] 的解析结果分叉。
pub async fn configs_dir() -> Result<PathBuf, InfiltratorError> {
    let store = config_manager().await?;
    Ok(store.config_dir())
}
