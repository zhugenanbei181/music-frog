use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "".to_string(),
            username: "".to_string(),
            password: "".to_string(),
            sync_interval_mins: 60,
            sync_on_startup: false,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RuntimePanelConfig {
    pub auto_refresh: bool,
    pub delay_sort: String,
    pub delay_test_url: String,
    pub delay_timeout_ms: u32,
    pub connection_filter: String,
    pub connection_sort: String,
}

impl Default for RuntimePanelConfig {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            delay_sort: "delay_asc".to_string(),
            delay_test_url: "http://www.gstatic.com/generate_204".to_string(),
            delay_timeout_ms: 5000,
            connection_filter: String::new(),
            connection_sort: "download_desc".to_string(),
        }
    }
}

/// Admin Web UI server settings, shared by both desktop frontends.
///
/// Defaults mirror the legacy Tauri client, which always serves the admin UI
/// on loopback starting at port 25210. The server only ever binds 127.0.0.1.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AdminServerConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 25210,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub editor_path: Option<String>,
    pub use_bundled_core: bool,
    pub core_channel: String,
    pub language: String,
    pub theme: String,
    /// 0.20 OS 系统通知总开关（订阅自动更新 / WebDAV 周期同步 / 内核错误）。
    /// 缺省开启；旧 settings.toml 无此键时按 true 反序列化（向后兼容约定）。
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    pub webdav: WebDavConfig,
    pub runtime_panel: RuntimePanelConfig,
    pub admin: AdminServerConfig,
    /// profiles/configs 存储目录覆盖（指向 iCloud/Dropbox/Syncthing 等云同步
    /// 目录即可零服务器同步）。空串/纯空白视为未设置；解析优先级
    /// （`INFILTRATOR_CONFIGS_DIR` 环境变量 > 本字段 > home 下 configs）
    /// 见 `mihomo_config::manager::paths::resolve_configs_dir`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs_dir: Option<String>,
}

fn default_notifications_enabled() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_path: None,
            use_bundled_core: true,
            core_channel: "stable".to_string(),
            language: "zh-CN".to_string(),
            theme: "system".to_string(),
            notifications_enabled: default_notifications_enabled(),
            webdav: WebDavConfig::default(),
            runtime_panel: RuntimePanelConfig::default(),
            admin: AdminServerConfig::default(),
            configs_dir: None,
        }
    }
}

pub async fn load_settings(path: &Path) -> anyhow::Result<AppSettings> {
    if path.exists() {
        let content = tokio::fs::read_to_string(path).await?;
        let settings: AppSettings = toml::from_str(&content)?;
        Ok(settings)
    } else {
        let legacy_path = path.with_extension("json");
        if legacy_path.exists() {
            let content = tokio::fs::read_to_string(&legacy_path).await?;
            let settings: AppSettings = serde_json::from_str(&content)?;
            if let Err(err) = save_settings(path, &settings).await {
                log::warn!("failed to migrate settings to toml: {err:#}");
            }
            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }
}

pub async fn save_settings(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = toml::to_string_pretty(settings)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

pub fn settings_path(base_dir: &Path) -> anyhow::Result<std::path::PathBuf> {
    if base_dir.as_os_str().is_empty() {
        return Err(anyhow!("settings base dir is empty"));
    }
    Ok(base_dir.join("settings.toml"))
}

/// 核心内部规范工厂：按真实 home（含测试 override）构造 ConfigManager，
/// configs 目录跟随 settings 的 `configs_dir` 字段（解析优先级见
/// `mihomo_config::manager::paths::resolve_configs_dir`）。
/// 全部业务门面的 ConfigManager 构造都必须经由这里，禁止再自行 `new()`。
pub async fn app_config_manager() -> anyhow::Result<mihomo_config::manager::ConfigManager> {
    let home = mihomo_platform::paths::get_home_dir()?;
    app_config_manager_in(&home).await
}

/// 同 [`app_config_manager`]，但 home 由调用方提供（bootstrap 与测试路径），
/// settings 从 `<home>/settings.toml` 读取。
pub async fn app_config_manager_in(
    home: &Path,
) -> anyhow::Result<mihomo_config::manager::ConfigManager> {
    let settings = load_settings(&settings_path(home)?).await?;
    Ok(
        mihomo_config::manager::ConfigManager::with_home_configs_dir_and_store(
            home.to_path_buf(),
            settings.configs_dir.as_deref(),
            mihomo_platform::traits::DefaultCredentialStore::default(),
        )?,
    )
}

/// 云同步重定向测试的全局状态护栏（仅测试构建）：持有 `TEST_LOCK` 串行化，
/// 构造时保存并清除 `INFILTRATOR_CONFIGS_DIR`、把 home override 指到指定
/// 目录；`Drop` 时全部恢复，断言失败也不泄漏全局状态。
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use crate::settings::{AppSettings, save_settings, settings_path};

    pub(crate) struct RedirectGuard {
        env_key: &'static str,
        prev_env: Option<String>,
        prev_home: PathBuf,
        _lock: tokio::sync::MutexGuard<'static, ()>,
    }

    impl RedirectGuard {
        /// 接管全局状态并把 home override 指到 `home`。
        pub(crate) async fn acquire(home: PathBuf) -> Self {
            let _lock = mihomo_platform::TEST_LOCK.lock().await;
            let env_key = mihomo_config::manager::paths::CONFIGS_DIR_ENV;
            let prev_env = std::env::var(env_key).ok();
            unsafe { std::env::remove_var(env_key) };
            let prev_home = mihomo_platform::paths::get_home_dir().expect("current home");
            assert!(mihomo_platform::paths::set_home_dir_override(home));
            Self {
                env_key,
                prev_env,
                prev_home,
                _lock,
            }
        }

        /// 写入 `<home>/settings.toml` 的 `configs_dir` 字段。
        pub(crate) async fn set_configs_dir(&self, home: &Path, configs_dir: Option<&str>) {
            let settings = AppSettings {
                configs_dir: configs_dir.map(str::to_string),
                ..AppSettings::default()
            };
            save_settings(&settings_path(home).expect("settings path"), &settings)
                .await
                .expect("save settings");
        }
    }

    impl Drop for RedirectGuard {
        fn drop(&mut self) {
            match self.prev_env.take() {
                Some(value) => unsafe { std::env::set_var(self.env_key, value) },
                None => unsafe { std::env::remove_var(self.env_key) },
            }
            mihomo_platform::paths::set_home_dir_override(self.prev_home.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(settings.use_bundled_core);
        assert!(settings.notifications_enabled);
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.theme, "system");
        assert!(settings.runtime_panel.auto_refresh);
        assert_eq!(settings.runtime_panel.delay_sort, "delay_asc");
        assert_eq!(
            settings.runtime_panel.delay_test_url,
            "http://www.gstatic.com/generate_204"
        );
        assert_eq!(settings.runtime_panel.delay_timeout_ms, 5000);
        assert_eq!(settings.runtime_panel.connection_sort, "download_desc");
    }

    #[test]
    fn test_admin_server_config_default_matches_tauri_behavior() {
        let admin = AdminServerConfig::default();
        assert!(
            admin.enabled,
            "admin server is on by default like src-tauri"
        );
        assert_eq!(admin.port, 25210);
    }

    #[tokio::test]
    async fn test_settings_without_admin_section_get_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        // A pre-admin settings file: must deserialize with admin defaults
        // instead of failing, so old installs keep working (serde back-compat).
        let legacy = "[runtime_panel]\nauto_refresh = true\n";
        std::fs::write(&settings_file, legacy).unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.admin, AdminServerConfig::default());
    }

    #[tokio::test]
    async fn test_save_and_load_admin_section() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings::default();
        settings.admin.enabled = false;
        settings.admin.port = 3000;

        save_settings(&settings_file, &settings).await.unwrap();
        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(!loaded.admin.enabled);
        assert_eq!(loaded.admin.port, 3000);
    }

    #[test]
    fn test_settings_path() {
        let base_dir = PathBuf::from("test_dir");
        let path = settings_path(&base_dir).expect("valid base dir should work");
        assert_eq!(path, base_dir.join("settings.toml"));

        let empty_dir = PathBuf::from("");
        assert!(settings_path(&empty_dir).is_err());
    }

    #[tokio::test]
    async fn test_save_and_load_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings {
            language: "en-US".to_string(),
            ..AppSettings::default()
        };
        settings.webdav.enabled = true;

        save_settings(&settings_file, &settings).await.unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.language, "en-US");
        assert!(loaded.webdav.enabled);
        assert!(loaded.runtime_panel.auto_refresh);
    }

    #[test]
    fn test_configs_dir_default_is_none() {
        assert_eq!(AppSettings::default().configs_dir, None);
    }

    /// serde 往返：configs_dir 存入 settings.toml 后原样读回。
    #[tokio::test]
    async fn test_configs_dir_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let settings = AppSettings {
            configs_dir: Some("~/Library/Mobile Documents/iCloud~Drive/Profiles".to_string()),
            ..AppSettings::default()
        };

        save_settings(&settings_file, &settings).await.unwrap();
        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(
            loaded.configs_dir,
            Some("~/Library/Mobile Documents/iCloud~Drive/Profiles".to_string())
        );

        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(raw.contains("configs_dir"), "field must be persisted");
    }

    /// 未设置时字段不得写进 settings.toml（skip_serializing_if），否则每次
    /// 保存都会给旧配置文件引入新键。
    #[tokio::test]
    async fn test_configs_dir_absent_from_toml_when_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        save_settings(&settings_file, &AppSettings::default())
            .await
            .unwrap();
        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("configs_dir"));
    }

    /// 旧版 settings.toml 没有 configs_dir 键：必须反序列化为 None 而不是报错。
    #[tokio::test]
    async fn test_legacy_settings_without_configs_dir_load_as_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        std::fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.configs_dir, None);
        assert_eq!(loaded.language, "en-US");
    }

    /// 旧版 settings.toml 没有 notifications_enabled 键：必须反序列化为 true
    /// 而不是报错（serde 向后兼容约定，同 :260 注释的 admin 缺省先例）。
    #[tokio::test]
    async fn test_legacy_settings_without_notifications_enabled_default_true() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        std::fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(loaded.notifications_enabled);
        assert_eq!(loaded.language, "en-US");
    }

    /// serde 往返：显式关闭通知后存盘再读回仍为关闭，且字段必须落盘。
    #[tokio::test]
    async fn test_notifications_enabled_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let settings = AppSettings {
            notifications_enabled: false,
            ..AppSettings::default()
        };
        save_settings(&settings_file, &settings).await.unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(!loaded.notifications_enabled);

        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(
            raw.contains("notifications_enabled"),
            "field must be persisted"
        );
    }

    /// 工厂跟随 settings 的 `configs_dir`：显式 home 与无参工厂（走全局
    /// home override）必须得到同一重定向结果，且默认目录不被创建。
    #[tokio::test]
    async fn test_app_config_manager_follows_settings_configs_dir() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let cloud = home.join("cloud").join("profiles");
        std::fs::create_dir_all(&cloud).unwrap();
        let guard = test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        let manager = app_config_manager_in(&home).await.unwrap();
        assert_eq!(manager.config_dir(), cloud.as_path());

        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), cloud.as_path());
        assert!(!home.join("configs").exists());
    }

    /// `configs_dir` 缺失 / 纯空白时必须与旧行为一致：默认 `<home>/configs`。
    #[tokio::test]
    async fn test_app_config_manager_defaults_when_configs_dir_unset() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let guard = test_support::RedirectGuard::acquire(home.clone()).await;

        // 无 settings 文件。
        let manager = app_config_manager_in(&home).await.unwrap();
        assert_eq!(manager.config_dir(), home.join("configs").as_path());

        // 字段为纯空白等价于未设置。
        guard.set_configs_dir(&home, Some("   ")).await;
        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), home.join("configs").as_path());
    }
}
