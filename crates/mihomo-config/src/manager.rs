//! Profile/config management for the mihomo infiltrator.
//!
//! [`ConfigManager`] owns an engine-scoped config directory and profile-state
//! file. The implementation is split along business seams:
//!
//! * `profiles` — profile CRUD and per-profile metadata
//! * `active` — active-profile selection and settings-file plumbing
//! * `defaults` — default-config bootstrap and proxy-port conflict repair
//! * `controller` — external-controller endpoint management
//! * `paths` — configs-dir resolution, profile name validation and
//!   canonicalized path construction
//! * `subscription_store` — credential-store persistence of subscription URLs
//! * `metadata` — settings-TOML mapping helpers

use std::path;
mod active;
mod controller;
mod defaults;
#[cfg(test)]
mod manager_test;
mod metadata;
mod namespace_migration;
pub mod paths;
mod profile_protection;
pub(crate) mod profiles;
mod subscription_store;

use crate::profile_write_boundary::shared_boundary;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::{Mutex, OnceCell};

pub struct ConfigManager<S: SecureStore> {
    config_root: PathBuf,
    config_dir: PathBuf,
    settings_file: PathBuf,
    legacy_settings_file: PathBuf,
    credential_store: S,
    write_boundary: Arc<Mutex<()>>,
    profile_namespace_initialized: OnceCell<()>,
    pub(crate) apply_observations: RwLock<BTreeMap<String, ApplyTransactionSnapshot>>,
}

impl<S: SecureStore> ConfigManager<S> {
    pub fn engine_id(&self) -> EngineId {
        crate::engine_id()
    }

    pub fn with_home_and_store(home: PathBuf, credential_store: S) -> Result<Self> {
        Self::with_home_configs_dir_and_store(home, None, credential_store)
    }

    /// 同时指定 home 与共享 configs 根目录来源。Profile 内容和 profile
    /// 状态都落入 `<configs_dir>/engines/<engine-id>/`；应用设置仍由
    /// `<home>/settings.toml` 的 owner 管理。
    pub fn with_home_configs_dir_and_store(
        home: PathBuf,
        configs_dir: Option<&str>,
        credential_store: S,
    ) -> Result<Self> {
        let config_root = paths::resolve_configs_dir_in(configs_dir, &home)?;
        let engine = crate::engine_id();
        let config_dir = paths::engine_config_dir(&config_root, &engine);
        let write_boundary = shared_boundary(&config_dir)?;
        let settings_file = config_dir.join("profile-state.toml");
        let legacy_settings_file = home.join("config.toml");

        Ok(Self {
            config_root,
            config_dir,
            settings_file,
            legacy_settings_file,
            credential_store,
            write_boundary,
            profile_namespace_initialized: OnceCell::new(),
            apply_observations: RwLock::new(BTreeMap::new()),
        })
    }

    /// 解析后的 Mihomo namespace 目录。它位于共享 configs 根下的
    /// `engines/mihomo/`，并可能尚不存在（由迁移、doctor fix 或 save 创建）。
    pub fn config_dir(&self) -> &path::Path {
        &self.config_dir
    }

    /// Shared root before adding this adapter's engine namespace.
    pub fn config_root(&self) -> &path::Path {
        &self.config_root
    }

    /// Adapter transaction boundary shared by every manager of this store.
    /// Hold only across persistence comparison/publication; never across
    /// lifecycle callbacks that can themselves repair or save a profile.
    pub async fn lock_profile_writes(&self) -> impl Send + '_ {
        self.write_boundary.lock().await
    }
}
