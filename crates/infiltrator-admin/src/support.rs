//! crate 内共用的 ConfigManager / configs 目录构造入口（不导出 crate 外）。

#[cfg(test)]
use std::path::PathBuf;

use infiltrator_core::settings_io as settings;
use infiltrator_domain::settings::AppSettings;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_platform::paths::get_home_dir;

/// home → settings.toml → load_settings。settings 尚未落盘时返回默认值。
async fn load_app_settings() -> anyhow::Result<AppSettings> {
    let home = get_home_dir()?;
    let settings_file = settings::settings_path(&home)?;
    settings::load_settings(&settings_file).await
}

/// settings 感知的 ConfigManager 构造：home → settings.toml →
/// load_settings → `with_configs_dir(settings.configs_dir)`。
/// settings.configs_dir 为 None 时使用 `<home>/configs`；
/// `INFILTRATOR_CONFIGS_DIR` 环境变量仍优先于 settings 字段
/// （优先级解析见 `mihomo_config::manager::paths::resolve_configs_dir_in`）。
pub(crate) async fn app_config_manager() -> anyhow::Result<ConfigManager<DefaultCredentialStore>> {
    let home = get_home_dir()?;
    let settings = load_app_settings().await?;
    Ok(ConfigManager::with_home_configs_dir_and_store(
        home,
        settings.configs_dir.as_deref(),
        DefaultCredentialStore::default(),
    )?)
}

/// configs 目录路径解析，优先级与 [`app_config_manager`] 一致
/// （env > settings.configs_dir > `<home>/configs`）。
#[cfg(test)]
pub(crate) async fn app_configs_dir() -> anyhow::Result<PathBuf> {
    let home = get_home_dir()?;
    let settings = load_app_settings().await?;
    Ok(mihomo_config::manager::paths::resolve_configs_dir_in(
        settings.configs_dir.as_deref(),
        &home,
    )?)
}

#[cfg(test)]
pub(crate) mod test_env {
    use mihomo_config::manager::paths::CONFIGS_DIR_ENV;

    /// env 是进程级全局状态：调用方必须持有 `mihomo_platform::TEST_LOCK`。
    /// 返回被清除前的旧值，供 [`restore_configs_dir_env`] 恢复。
    pub(crate) fn clear_configs_dir_env() -> Option<String> {
        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        // SAFETY: 测试在 TEST_LOCK 互斥下串行修改进程级 env，并在结束时恢复。
        unsafe { std::env::remove_var(CONFIGS_DIR_ENV) };
        saved
    }

    /// 写入或清除 env（`None` 表示清除），用于恢复
    /// [`clear_configs_dir_env`] 返回的旧值。
    pub(crate) fn restore_configs_dir_env(saved: Option<String>) {
        // SAFETY: 同 clear_configs_dir_env。
        match saved {
            Some(value) => unsafe { std::env::set_var(CONFIGS_DIR_ENV, value) },
            None => unsafe { std::env::remove_var(CONFIGS_DIR_ENV) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{app_config_manager, app_configs_dir, test_env};
    use infiltrator_core::settings_io as settings;
    use infiltrator_domain::settings::AppSettings;
    use mihomo_platform::TEST_LOCK;

    async fn write_settings_with_configs_dir(
        home: &std::path::Path,
        configs_dir: &std::path::Path,
    ) {
        let settings = AppSettings {
            configs_dir: Some(configs_dir.to_string_lossy().into_owned()),
            ..AppSettings::default()
        };
        settings::save_settings(&settings::settings_path(home).unwrap(), &settings)
            .await
            .unwrap();
    }

    /// settings 字段 configs_dir 生效：manager 的 profile 读写落在重定向目录。
    #[tokio::test]
    async fn app_config_manager_follows_settings_configs_dir() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let saved_env = test_env::clear_configs_dir_env();

        let cloud = temp_dir.path().join("cloud");
        write_settings_with_configs_dir(temp_dir.path(), &cloud).await;

        let manager = app_config_manager().await.unwrap();
        manager.save("probe", "port: 7890").await.unwrap();
        assert!(cloud.join("probe.yaml").exists());
        assert!(!temp_dir.path().join("configs").exists());

        test_env::restore_configs_dir_env(saved_env);
        mihomo_platform::paths::clear_home_dir_override();
    }

    /// settings.configs_dir 未设置时 configs 目录仍是默认 `<home>/configs`。
    #[tokio::test]
    async fn app_configs_dir_defaults_to_home_configs() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let saved_env = test_env::clear_configs_dir_env();

        assert_eq!(
            app_configs_dir().await.unwrap(),
            temp_dir.path().join("configs")
        );

        test_env::restore_configs_dir_env(saved_env);
        mihomo_platform::paths::clear_home_dir_override();
    }

    /// settings 字段与 env 同时设置时 env 优先。
    #[tokio::test]
    async fn app_configs_dir_env_wins_over_settings_field() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let cloud = temp_dir.path().join("cloud");
        write_settings_with_configs_dir(temp_dir.path(), &cloud).await;
        let saved_env = std::env::var(mihomo_config::manager::paths::CONFIGS_DIR_ENV).ok();
        let env_dir = temp_dir
            .path()
            .join("env-dir")
            .to_string_lossy()
            .into_owned();
        test_env::restore_configs_dir_env(Some(env_dir));

        assert_eq!(
            app_configs_dir().await.unwrap(),
            temp_dir.path().join("env-dir")
        );

        test_env::restore_configs_dir_env(saved_env);
        mihomo_platform::paths::clear_home_dir_override();
    }
}
