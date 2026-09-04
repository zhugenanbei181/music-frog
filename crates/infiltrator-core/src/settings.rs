use anyhow::anyhow;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_platform::defaults::DefaultCredentialStore;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    /// WebDAV 密码只在内存与 OS keyring 中流转，永不落盘（serde 只跳过
    /// 序列化；反序列化仍接受旧 settings.toml 里的明文，交给
    /// [`load_settings`] 的迁移逻辑搬到 keyring）。keyring 命名空间见
    /// [`load_webdav_password`] / [`save_webdav_password`]。
    #[serde(skip_serializing)]
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
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
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    pub webdav: WebDavConfig,
    pub runtime_panel: RuntimePanelConfig,
    pub admin: AdminServerConfig,
    /// profiles/configs 存储目录覆盖（指向 iCloud/Dropbox/Syncthing 等云同步
    /// 目录即可零服务器同步）。空串/纯空白视为未设置；解析优先级
    /// （`INFILTRATOR_CONFIGS_DIR` 环境变量 > 本字段 > home 下 configs）
    /// 见 `mihomo_config::manager::paths::resolve_configs_dir`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_proxy_bypass: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_size: Option<(f32, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_position: Option<(i32, i32)>,
    #[serde(default)]
    pub window_maximized: bool,
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_close_to_tray() -> bool {
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
            close_to_tray: default_close_to_tray(),
            webdav: WebDavConfig::default(),
            runtime_panel: RuntimePanelConfig::default(),
            admin: AdminServerConfig::default(),
            configs_dir: None,
            system_proxy_bypass: None,
            window_size: None,
            window_position: None,
            window_maximized: false,
        }
    }
}

/// WebDAV 凭据在 keyring 中的 service 名：与 mihomo-config 订阅 URL 的
/// 既有先例（`subscription:<profile>`）共用同一 service，key 用 `webdav:`
/// 前缀区分命名空间。
pub const WEBDAV_CREDENTIAL_SERVICE: &str = "MusicFrog-Despicable-Infiltrator";
/// keyring key：整个安装只有一份 WebDAV 账号配置。
pub const WEBDAV_PASSWORD_KEY: &str = "webdav:password";

/// 读取 keyring 中的 WebDAV 密码。空条目与读取失败都归一为 `None`
/// （失败仅 `log::warn`，不让调用方崩溃）。
pub async fn load_webdav_password<S: SecureStore>(store: &S) -> Option<String> {
    match store
        .get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
        .await
    {
        Ok(Some(value)) if !value.is_empty() => Some(value),
        Ok(_) => None,
        Err(err) => {
            log::warn!("webdav password get failed: {err}");
            None
        }
    }
}

/// 把 WebDAV 密码写入 keyring。失败返回 `Err`，由调用方决定是否中断
/// （保存路径应当中断，避免「文件已更新而凭据丢失」的不一致）。
pub async fn save_webdav_password<S: SecureStore>(store: &S, password: &str) -> anyhow::Result<()> {
    store
        .set(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY, password)
        .await
        .map_err(|err| anyhow!("webdav password keyring set failed: {err}"))
}

/// 清除 keyring 中的 WebDAV 密码。条目不存在或删除失败只告警（幂等清空：
/// 「清除」语义下二次清除与目标本就为空都算成功）。
pub async fn clear_webdav_password<S: SecureStore>(store: &S) {
    if let Err(err) = store
        .delete(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
        .await
    {
        log::warn!("webdav password keyring clear failed (ignored): {err}");
    }
}

/// 旧版 settings.toml 明文携带 `webdav.password` 的一次性迁移：写入
/// keyring 后用「不带密码」的序列化重写文件。keyring 写失败时保留内存值
/// 并跳过重写（下次启动重试），绝不因迁移失败让启动崩溃。
async fn migrate_webdav_password_to_keyring<S: SecureStore>(
    settings: &mut AppSettings,
    path: &Path,
    store: &S,
) {
    if settings.webdav.password.is_empty() {
        return;
    }
    match save_webdav_password(store, &settings.webdav.password).await {
        Ok(()) => {
            settings.webdav.password = String::new();
            if let Err(err) = save_settings(path, settings).await {
                // keyring 已有副本；文件重写失败只留下明文等下次迁移。
                log::warn!("failed to rewrite settings without webdav password: {err:#}");
            }
        }
        Err(err) => {
            log::warn!(
                "webdav password keyring migration failed, keeping value in memory only: {err:#}"
            );
        }
    }
}

pub async fn load_settings(path: &Path) -> anyhow::Result<AppSettings> {
    load_settings_with_store(path, &DefaultCredentialStore::default()).await
}

/// 同 [`load_settings`]，但 WebDAV 密码迁移走调用方提供的凭据存储
/// （测试注入内存实现，避免触碰真实 OS keyring）。
pub async fn load_settings_with_store<S: SecureStore>(
    path: &Path,
    store: &S,
) -> anyhow::Result<AppSettings> {
    if path.exists() {
        let content = tokio::fs::read_to_string(path).await?;
        let mut settings: AppSettings = toml::from_str(&content)?;
        migrate_webdav_password_to_keyring(&mut settings, path, store).await;
        Ok(settings)
    } else {
        let legacy_path = path.with_extension("json");
        if legacy_path.exists() {
            let content = tokio::fs::read_to_string(&legacy_path).await?;
            let mut settings: AppSettings = serde_json::from_str(&content)?;
            migrate_webdav_password_to_keyring(&mut settings, &legacy_path, store).await;
            if let Err(err) = save_settings(path, &settings).await {
                log::warn!("failed to migrate settings to toml: {err:#}");
            }
            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }
}

/// UI 水合专用：[`load_settings`] 之后把 keyring 中的 WebDAV 密码填回
/// `settings.webdav.password` 内存镜像（iced 的 `webdav_pass` 域、需要完整
/// 凭据的同步调用方）。password 序列化被跳过，因此水合值不会落盘。
pub async fn load_settings_hydrated(path: &Path) -> anyhow::Result<AppSettings> {
    load_settings_hydrated_with_store(path, &DefaultCredentialStore::default()).await
}

/// 同 [`load_settings_hydrated`]，但 keyring 存取走调用方提供的凭据存储。
pub async fn load_settings_hydrated_with_store<S: SecureStore>(
    path: &Path,
    store: &S,
) -> anyhow::Result<AppSettings> {
    let mut settings = load_settings_with_store(path, store).await?;
    if settings.webdav.password.is_empty()
        && let Some(password) = load_webdav_password(store).await
    {
        settings.webdav.password = password;
    }
    Ok(settings)
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
/// `mihomo_config::manager::paths::resolve_configs_dir_in`）。
/// 全部业务门面的 ConfigManager 构造都必须经由这里，禁止再自行 `new()`。
pub async fn app_config_manager()
-> anyhow::Result<mihomo_config::manager::ConfigManager<DefaultCredentialStore>> {
    let home = mihomo_platform::paths::get_home_dir()?;
    app_config_manager_in(&home).await
}

/// 同 [`app_config_manager`]，但 home 由调用方提供（bootstrap 与测试路径），
/// settings 从 `<home>/settings.toml` 读取。
pub async fn app_config_manager_in(
    home: &Path,
) -> anyhow::Result<mihomo_config::manager::ConfigManager<DefaultCredentialStore>> {
    let settings = load_settings(&settings_path(home)?).await?;
    Ok(
        mihomo_config::manager::ConfigManager::with_home_configs_dir_and_store(
            home.to_path_buf(),
            settings.configs_dir.as_deref(),
            mihomo_platform::defaults::DefaultCredentialStore::default(),
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

    /// 内存凭据存储（仿 session.rs / manager_test 的 MockStore 先例），
    /// `fail_set` 可注入写失败以覆盖迁移的降级路径。
    struct MemoryStore {
        entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
        fail_set: bool,
    }

    impl MemoryStore {
        fn working() -> Self {
            Self {
                entries: std::sync::Mutex::new(std::collections::HashMap::new()),
                fail_set: false,
            }
        }

        fn failing_set() -> Self {
            Self {
                entries: std::sync::Mutex::new(std::collections::HashMap::new()),
                fail_set: true,
            }
        }

        fn get(&self, service: &str, key: &str) -> Option<String> {
            self.entries
                .lock()
                .expect("store lock")
                .get(&format!("{service}/{key}"))
                .cloned()
        }
    }

    #[async_trait::async_trait]
    impl SecureStore for MemoryStore {
        async fn get(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<Option<String>, infiltrator_ports::error::PortError> {
            Ok(self.get(service, key))
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            if self.fail_set {
                return Err(infiltrator_ports::error::PortError::Failed(
                    "injected keyring failure".to_string(),
                ));
            }
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .remove(&format!("{service}/{key}"));
            Ok(())
        }
    }

    fn legacy_toml_with_password() -> String {
        "[webdav]\nenabled = true\nurl = \"https://dav.example.com\"\nusername = \"user\"\npassword = \"s3cret\"\nsync_interval_mins = 30\n"
            .to_string()
    }

    /// 密码永不落盘：save_settings 的 TOML 输出不得包含 password 键。
    #[tokio::test]
    async fn test_webdav_password_never_serialized() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings::default();
        settings.webdav.url = "https://dav.example.com".to_string();
        settings.webdav.password = "s3cret".to_string();
        save_settings(&settings_file, &settings).await.unwrap();

        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("password"), "plaintext leaked: {raw}");
        // 反序列化缺省键回到空串，不影响其他字段往返。
        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.webdav.url, "https://dav.example.com");
    }

    /// 迁移主路径：旧明文 toml 加载后 keyring 有值、内存与重写后的文件
    /// 均无明文；第二次加载保持干净且 keyring 值不丢。
    #[tokio::test]
    async fn test_legacy_plaintext_password_migrates_to_keyring() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        std::fs::write(&settings_file, legacy_toml_with_password()).unwrap();

        let store = MemoryStore::working();
        let loaded = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();

        assert_eq!(
            store
                .get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
                .as_deref(),
            Some("s3cret"),
            "password must land in the credential store"
        );
        assert_eq!(loaded.webdav.password, "", "memory mirror must be blanked");

        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(
            !raw.contains("password"),
            "file must be rewritten clean: {raw}"
        );
        assert!(
            raw.contains("enabled = true"),
            "other fields survive: {raw}"
        );

        // 第二次加载：文件已无明文，keyring 值仍在（不重复迁移）。
        let second = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(second.webdav.password, "");
        assert_eq!(
            store
                .get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
                .as_deref(),
            Some("s3cret")
        );
    }

    /// 迁移降级路径：keyring 写失败时保留内存值、不重写文件（下次启动
    /// 重试），加载本身不得报错。
    #[tokio::test]
    async fn test_migration_keeps_plaintext_when_keyring_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        std::fs::write(&settings_file, legacy_toml_with_password()).unwrap();

        let store = MemoryStore::failing_set();
        let loaded = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();

        assert_eq!(
            loaded.webdav.password, "s3cret",
            "value must stay in memory so the session keeps working"
        );
        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(
            raw.contains("s3cret"),
            "file must be left untouched for retry: {raw}"
        );
    }

    /// 水合：干净的 settings 文件 + keyring 有值 → hydrated 加载把密码填回
    /// 内存镜像；普通加载保持空串。
    #[tokio::test]
    async fn test_load_settings_hydrated_fills_password_from_keyring() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        save_settings(&settings_file, &AppSettings::default())
            .await
            .unwrap();

        let store = MemoryStore::working();
        store
            .set(
                WEBDAV_CREDENTIAL_SERVICE,
                WEBDAV_PASSWORD_KEY,
                "hydrated-pw",
            )
            .await
            .unwrap();

        let plain = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(plain.webdav.password, "");

        let hydrated = load_settings_hydrated_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(hydrated.webdav.password, "hydrated-pw");

        // 水合值只是内存镜像：文件依旧干净。
        let raw = std::fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("password"));
    }

    /// 清除是幂等的：空存储上 clear 不得报错，已有条目被移除。
    #[tokio::test]
    async fn test_clear_webdav_password_is_idempotent() {
        let store = MemoryStore::working();
        clear_webdav_password(&store).await;
        assert_eq!(
            store.get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY),
            None
        );

        store
            .set(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY, "pw")
            .await
            .unwrap();
        clear_webdav_password(&store).await;
        assert_eq!(
            store.get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY),
            None
        );
    }
}
