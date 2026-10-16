use anyhow::anyhow;
use infiltrator_contract::runtime_settings::{
    DEFAULT_RUNTIME_PROBE_TIMEOUT_MS, DEFAULT_RUNTIME_PROBE_URL, RuntimeProbeSettings,
};
use infiltrator_domain::settings::{AppSettings, CURRENT_SETTINGS_SCHEMA_VERSION};
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use std::path::{Path, PathBuf};
use tokio::fs::{create_dir_all, read_to_string, write};
use toml::map::Map;

/// WebDAV 凭据在 keyring 中的 service 名：与 mihomo-config 订阅 URL 的
/// engine-scoped key 共用同一 service，key 用 `webdav:` 前缀区分命名空间。
pub const WEBDAV_CREDENTIAL_SERVICE: &str = "MusicFrog-Infiltrator";
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CredentialMigration {
    Unchanged,
    Persisted,
    Failed,
}

/// 旧版 settings 文档明文携带 `webdav.password` 时迁入 keyring。
/// 调用方决定何时以新 schema 重写设置文件，避免凭据迁移失败时丢失原文件值。
async fn migrate_webdav_password_to_keyring<S: SecureStore>(
    settings: &mut AppSettings,
    store: &S,
) -> CredentialMigration {
    if settings.webdav.password.is_empty() {
        return CredentialMigration::Unchanged;
    }
    match save_webdav_password(store, &settings.webdav.password).await {
        Ok(()) => {
            settings.webdav.password = String::new();
            CredentialMigration::Persisted
        }
        Err(err) => {
            log::warn!(
                "webdav password keyring migration failed, keeping value in memory only: {err:#}"
            );
            CredentialMigration::Failed
        }
    }
}

fn legacy_delay_sort_key(value: &str) -> Option<&'static str> {
    match value {
        "delay_asc" => Some("latency_ascending"),
        "delay_desc" => Some("latency_descending"),
        "name_asc" => Some("name_ascending"),
        "name_desc" => Some("name_descending"),
        _ => None,
    }
}

/// Convert the unversioned settings layout to the current typed schema.
/// Historical keys are read only here and are removed before AppSettings is decoded.
fn migrate_runtime_settings(value: &mut toml::Value) -> anyhow::Result<bool> {
    let table = value
        .as_table_mut()
        .ok_or_else(|| anyhow!("settings document root must be a TOML table"))?;
    let version = match table.get("settings_schema_version") {
        None => 1,
        Some(toml::Value::Integer(version)) => u32::try_from(*version)
            .map_err(|_| anyhow!("settings schema version is outside the supported range"))?,
        Some(_) => return Err(anyhow!("settings schema version must be an integer")),
    };

    match version {
        CURRENT_SETTINGS_SCHEMA_VERSION => {
            if table
                .get("runtime_panel")
                .and_then(toml::Value::as_table)
                .is_some_and(|panel| {
                    ["delay_sort", "delay_test_url", "delay_timeout_ms"]
                        .iter()
                        .any(|key| panel.contains_key(*key))
                })
            {
                return Err(anyhow!(
                    "current settings schema contains retired runtime-panel keys"
                ));
            }
            Ok(false)
        }
        1 => {
            let panel = table
                .entry("runtime_panel")
                .or_insert_with(|| toml::Value::Table(Map::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow!("runtime_panel settings must be a TOML table"))?;

            if panel.contains_key("probe_settings") || panel.contains_key("delay_sort_order") {
                return Err(anyhow!(
                    "unversioned settings mix legacy and typed runtime settings"
                ));
            }

            let url = match panel.remove("delay_test_url") {
                None => DEFAULT_RUNTIME_PROBE_URL.to_owned(),
                Some(toml::Value::String(value)) => value,
                Some(_) => return Err(anyhow!("legacy probe URL must be a string")),
            };
            let timeout_ms = match panel.remove("delay_timeout_ms") {
                None => DEFAULT_RUNTIME_PROBE_TIMEOUT_MS,
                Some(toml::Value::Integer(value)) => u32::try_from(value)
                    .map_err(|_| anyhow!("legacy probe timeout must be a positive u32"))?,
                Some(_) => return Err(anyhow!("legacy probe timeout must be an integer")),
            };
            let probe = RuntimeProbeSettings::new(url, timeout_ms)
                .map_err(|error| anyhow!("invalid legacy runtime probe settings: {error}"))?;

            let sort_order = match panel.remove("delay_sort") {
                None => "latency_ascending",
                Some(toml::Value::String(value)) => legacy_delay_sort_key(&value)
                    .ok_or_else(|| anyhow!("unknown legacy runtime delay sort key: {value}"))?,
                Some(_) => return Err(anyhow!("legacy runtime delay sort key must be a string")),
            };
            let mut probe_table = Map::new();
            probe_table.insert(
                "url".to_owned(),
                toml::Value::String(probe.url().to_owned()),
            );
            probe_table.insert(
                "timeout_ms".to_owned(),
                toml::Value::Integer(i64::from(probe.timeout_ms())),
            );
            panel.insert("probe_settings".to_owned(), toml::Value::Table(probe_table));
            panel.insert(
                "delay_sort_order".to_owned(),
                toml::Value::String(sort_order.to_owned()),
            );
            table.insert(
                "settings_schema_version".to_owned(),
                toml::Value::Integer(i64::from(CURRENT_SETTINGS_SCHEMA_VERSION)),
            );
            Ok(true)
        }
        other => Err(anyhow!(
            "settings schema version {other} is unsupported; current version is {CURRENT_SETTINGS_SCHEMA_VERSION}"
        )),
    }
}

fn decode_settings_document(mut value: toml::Value) -> anyhow::Result<(AppSettings, bool)> {
    let migrated = migrate_runtime_settings(&mut value)?;
    let settings = value
        .try_into::<AppSettings>()
        .map_err(|error| anyhow!("invalid typed settings document: {error}"))?;
    Ok((settings, migrated))
}

pub(crate) fn parse_settings_toml(content: &str) -> anyhow::Result<(AppSettings, bool)> {
    let value = toml::from_str(content)?;
    decode_settings_document(value)
}

fn migrate_runtime_settings_json(value: &mut serde_json::Value) -> anyhow::Result<bool> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings document root must be a JSON object"))?;
    let version = match root.get("settings_schema_version") {
        None => 1,
        Some(value) => u32::try_from(
            value
                .as_u64()
                .ok_or_else(|| anyhow!("settings schema version must be an integer"))?,
        )
        .map_err(|_| anyhow!("settings schema version is outside the supported range"))?,
    };
    match version {
        CURRENT_SETTINGS_SCHEMA_VERSION => {
            if root
                .get("runtime_panel")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|panel| {
                    ["delay_sort", "delay_test_url", "delay_timeout_ms"]
                        .iter()
                        .any(|key| panel.contains_key(*key))
                })
            {
                return Err(anyhow!(
                    "current settings schema contains retired runtime-panel keys"
                ));
            }
            Ok(false)
        }
        1 => {
            let panel = root
                .entry("runtime_panel")
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                .as_object_mut()
                .ok_or_else(|| anyhow!("runtime_panel settings must be a JSON object"))?;
            if panel.contains_key("probe_settings") || panel.contains_key("delay_sort_order") {
                return Err(anyhow!(
                    "unversioned settings mix legacy and typed runtime settings"
                ));
            }
            let url = match panel.remove("delay_test_url") {
                None => DEFAULT_RUNTIME_PROBE_URL.to_owned(),
                Some(serde_json::Value::String(value)) => value,
                Some(_) => return Err(anyhow!("legacy probe URL must be a string")),
            };
            let timeout_ms = match panel.remove("delay_timeout_ms") {
                None => DEFAULT_RUNTIME_PROBE_TIMEOUT_MS,
                Some(value) => u32::try_from(
                    value
                        .as_u64()
                        .ok_or_else(|| anyhow!("legacy probe timeout must be a positive u32"))?,
                )
                .map_err(|_| anyhow!("legacy probe timeout must be a positive u32"))?,
            };
            let settings = RuntimeProbeSettings::new(url, timeout_ms)
                .map_err(|error| anyhow!("invalid legacy runtime probe settings: {error}"))?;
            let sort_order = match panel.remove("delay_sort") {
                None => "latency_ascending",
                Some(serde_json::Value::String(value)) => legacy_delay_sort_key(&value)
                    .ok_or_else(|| anyhow!("unknown legacy runtime delay sort key: {value}"))?,
                Some(_) => return Err(anyhow!("legacy runtime delay sort key must be a string")),
            };
            panel.insert(
                "probe_settings".to_owned(),
                serde_json::json!({
                    "url": settings.url(),
                    "timeout_ms": settings.timeout_ms(),
                }),
            );
            panel.insert(
                "delay_sort_order".to_owned(),
                serde_json::Value::String(sort_order.to_owned()),
            );
            root.insert(
                "settings_schema_version".to_owned(),
                serde_json::Value::from(CURRENT_SETTINGS_SCHEMA_VERSION),
            );
            Ok(true)
        }
        other => Err(anyhow!(
            "settings schema version {other} is unsupported; current version is {CURRENT_SETTINGS_SCHEMA_VERSION}"
        )),
    }
}

fn decode_settings_json(mut value: serde_json::Value) -> anyhow::Result<(AppSettings, bool)> {
    let migrated = migrate_runtime_settings_json(&mut value)?;
    let settings = serde_json::from_value(value)
        .map_err(|error| anyhow!("invalid typed settings document: {error}"))?;
    Ok((settings, migrated))
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
        let content = read_to_string(path).await?;
        let (mut settings, migrated) = parse_settings_toml(&content)?;
        let credential_migration = migrate_webdav_password_to_keyring(&mut settings, store).await;
        if credential_migration == CredentialMigration::Failed {
            return Ok(settings);
        }
        if migrated || credential_migration == CredentialMigration::Persisted {
            save_settings(path, &settings).await?;
        }
        Ok(settings)
    } else {
        let legacy_path = path.with_extension("json");
        if legacy_path.exists() {
            let content = read_to_string(&legacy_path).await?;
            let json_value: serde_json::Value = serde_json::from_str(&content)?;
            let (mut settings, _) = decode_settings_json(json_value)?;
            let credential_migration =
                migrate_webdav_password_to_keyring(&mut settings, store).await;
            if credential_migration != CredentialMigration::Failed {
                save_settings(path, &settings).await?;
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
        create_dir_all(parent).await?;
    }
    let content = toml::to_string_pretty(settings)?;
    write(path, content).await?;
    Ok(())
}

pub fn settings_path(base_dir: &Path) -> anyhow::Result<PathBuf> {
    if base_dir.as_os_str().is_empty() {
        return Err(anyhow!("settings base dir is empty"));
    }
    Ok(base_dir.join("settings.toml"))
}

/// 核心内部规范工厂：按真实 home（含测试 override）构造 ConfigManager，
/// configs 目录跟随 settings 的 `configs_dir` 字段（解析优先级见
/// `mihomo_config::manager::paths::resolve_configs_dir_in`）。
/// 全部业务门面的 ConfigManager 构造都必须经由这里，禁止再自行 `new()`。
pub async fn app_config_manager() -> anyhow::Result<ConfigManager<DefaultCredentialStore>> {
    let home = get_home_dir()?;
    app_config_manager_in(&home).await
}

/// Product reset uses a distinct initialization policy so it can reach and
/// resume its durable intent before ordinary profile mutations are enabled.
pub async fn app_config_manager_for_profile_reset()
-> anyhow::Result<ConfigManager<DefaultCredentialStore>> {
    let home = get_home_dir()?;
    app_config_manager_in_with_policy(
        &home,
        ConfigManagerInitializationPolicy::ProfileResetRecovery,
    )
    .await
}

/// 同 [`app_config_manager`]，但 home 由调用方提供（bootstrap 与测试路径），
/// settings 从 `<home>/settings.toml` 读取。
pub async fn app_config_manager_in(
    home: &Path,
) -> anyhow::Result<ConfigManager<DefaultCredentialStore>> {
    app_config_manager_in_with_policy(home, ConfigManagerInitializationPolicy::Standard).await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigManagerInitializationPolicy {
    Standard,
    ProfileResetRecovery,
}

async fn app_config_manager_in_with_policy(
    home: &Path,
    policy: ConfigManagerInitializationPolicy,
) -> anyhow::Result<ConfigManager<DefaultCredentialStore>> {
    let settings = load_settings(&settings_path(home)?).await?;
    let manager = ConfigManager::with_home_configs_dir_and_store(
        home.to_path_buf(),
        settings.configs_dir.as_deref(),
        DefaultCredentialStore::default(),
    )?;
    match policy {
        ConfigManagerInitializationPolicy::Standard => {
            manager.initialize_profile_namespace().await?;
        }
        ConfigManagerInitializationPolicy::ProfileResetRecovery => {
            manager
                .initialize_profile_namespace_for_reset_recovery()
                .await?;
        }
    }
    Ok(manager)
}

/// 云同步重定向测试的全局状态护栏（仅测试构建）：持有 `TEST_LOCK` 串行化，
/// 构造时保存并清除 `INFILTRATOR_CONFIGS_DIR`、把 home override 指到指定
/// 目录；`Drop` 时全部恢复，断言失败也不泄漏全局状态。
#[cfg(test)]
pub(crate) mod test_support {
    #[cfg(test)]
    use infiltrator_platform::paths::{get_home_dir, set_home_dir_override};
    #[cfg(test)]
    use mihomo_config::manager::paths::CONFIGS_DIR_ENV;
    #[cfg(test)]
    use std::env::remove_var;
    #[cfg(test)]
    use std::env::set_var;
    #[cfg(test)]
    use std::env::var;
    use std::path::{Path, PathBuf};
    #[cfg(test)]
    use tokio::sync::MutexGuard;

    use super::{AppSettings, save_settings, settings_path};

    pub(crate) struct RedirectGuard {
        env_key: &'static str,
        prev_env: Option<String>,
        prev_home: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl RedirectGuard {
        /// 接管全局状态并把 home override 指到 `home`。
        pub(crate) async fn acquire(home: PathBuf) -> Self {
            let _lock = infiltrator_platform::TEST_LOCK.lock().await;
            let env_key = CONFIGS_DIR_ENV;
            let prev_env = var(env_key).ok();
            unsafe { remove_var(env_key) };
            let prev_home = get_home_dir().expect("current home");
            assert!(set_home_dir_override(home));
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
                Some(value) => unsafe { set_var(self.env_key, value) },
                None => unsafe { remove_var(self.env_key) },
            }
            set_home_dir_override(self.prev_home.clone());
        }
    }
}

#[cfg(test)]
#[path = "settings_io_tests.rs"]
mod tests;
