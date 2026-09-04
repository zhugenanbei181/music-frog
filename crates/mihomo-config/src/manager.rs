//! Profile/config management for the mihomo infiltrator.
//!
//! [`ConfigManager`] owns the config directory and the settings file
//! (`config.toml`). The implementation is split along business seams:
//!
//! * `profiles` — profile CRUD and per-profile metadata
//! * `active` — active-profile selection and settings-file plumbing
//! * `defaults` — default-config bootstrap and proxy-port conflict repair
//! * `controller` — external-controller endpoint management
//! * `paths` — configs-dir resolution, profile name validation and
//!   canonicalized path construction
//! * `subscription_store` — credential-store persistence of subscription URLs
//! * `metadata` — settings-TOML mapping helpers

mod active;
mod controller;
mod defaults;
#[cfg(test)]
mod manager_test;
mod metadata;
pub mod paths;
mod profiles;
mod subscription_store;

use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::Result;
use std::path::PathBuf;

pub struct ConfigManager<S: SecureStore> {
    config_dir: PathBuf,
    settings_file: PathBuf,
    credential_store: S,
}

impl<S: SecureStore> ConfigManager<S> {
    pub fn with_home_and_store(home: PathBuf, credential_store: S) -> Result<Self> {
        Self::with_home_configs_dir_and_store(home, None, credential_store)
    }

    /// 同时指定 home 与 configs 目录来源（`configs_dir`，即 settings 的
    /// `configs_dir` 字段）构造；环境变量 [`paths::CONFIGS_DIR_ENV`] 仍然
    /// 优先（解析规则见 [`paths::resolve_configs_dir_in`]）。settings 文件
    /// （当前 profile 指针）始终是 `<home>/config.toml`，不跟随 configs
    /// 目录重定向。
    pub fn with_home_configs_dir_and_store(
        home: PathBuf,
        configs_dir: Option<&str>,
        credential_store: S,
    ) -> Result<Self> {
        let config_dir = paths::resolve_configs_dir_in(configs_dir, &home)?;
        let settings_file = home.join("config.toml");

        Ok(Self {
            config_dir,
            settings_file,
            credential_store,
        })
    }

    /// 解析后的 configs（profiles yaml）存储目录。目录可能尚不存在
    /// （创建归 doctor fix 与各 save 流程）。
    pub fn config_dir(&self) -> &std::path::Path {
        &self.config_dir
    }
}
