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

/// WebDAV 凭据在 keyring 中的 service 名：与 mihomo-config 订阅 URL 的
/// engine-scoped key 共用同一 service，key 用 `webdav:` 前缀区分命名空间。
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
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
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
            let mut probe_table = toml::map::Map::new();
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
mod tests {
    use super::*;
    use infiltrator_contract::runtime_settings::{RuntimeDelaySortOrder, RuntimeProbeSettings};
    use infiltrator_domain::settings::AdminServerConfig;
    #[cfg(test)]
    use infiltrator_ports::error::PortError;
    use mihomo_config::manager::paths::engine_config_dir;
    #[cfg(test)]
    use std::collections::HashMap;
    #[cfg(test)]
    use std::fs;
    use std::path::PathBuf;
    #[cfg(test)]
    use std::result;
    #[cfg(test)]
    use std::sync::Mutex;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(settings.use_bundled_core);
        assert!(settings.notifications_enabled);
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.theme, "system");
        assert!(settings.runtime_panel.auto_refresh);
        assert_eq!(
            settings.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyAscending
        );
        assert_eq!(
            settings.runtime_panel.probe_settings,
            RuntimeProbeSettings::default()
        );
        assert_eq!(
            settings.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(settings.runtime_panel.connection_sort, "download_desc");
    }

    #[tokio::test]
    async fn legacy_runtime_settings_migrate_once_to_typed_versioned_values() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(
            &settings_file,
            r#"[runtime_panel]
auto_refresh = false
delay_sort = "name_desc"
delay_test_url = "https://probe.example.test/check"
delay_timeout_ms = 60000
"#,
        )
        .unwrap();

        let migrated = load_settings(&settings_file).await.unwrap();
        assert_eq!(
            migrated.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            migrated.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::NameDescending
        );
        assert_eq!(
            migrated.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/check", 60_000).unwrap()
        );

        let canonical = fs::read_to_string(&settings_file).unwrap();
        assert!(canonical.contains("settings_schema_version = 2"));
        assert!(canonical.contains(r#"delay_sort_order = "name_descending""#));
        assert!(canonical.contains("timeout_ms = 60000"));
        for retired in ["delay_sort =", "delay_test_url =", "delay_timeout_ms ="] {
            assert!(
                !canonical.contains(retired),
                "retired key remains: {retired}"
            );
        }

        let second_load = load_settings(&settings_file).await.unwrap();
        assert_eq!(second_load, migrated);
        assert_eq!(fs::read_to_string(&settings_file).unwrap(), canonical);
        let (_, migrated_again) = parse_settings_toml(&canonical).unwrap();
        assert!(
            !migrated_again,
            "schema v2 must not run the v1 migration again"
        );
    }

    #[test]
    fn explicit_schema_v1_runtime_settings_upgrade_to_v2_typed_values() {
        let legacy = r#"settings_schema_version = 1

[runtime_panel]
delay_sort = "delay_desc"
delay_test_url = "https://probe.example.test/v1"
delay_timeout_ms = 45000
"#;

        let (settings, migrated) = parse_settings_toml(legacy).unwrap();

        assert!(migrated);
        assert_eq!(
            settings.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            settings.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyDescending
        );
        assert_eq!(
            settings.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/v1", 45_000).unwrap()
        );
    }

    #[tokio::test]
    async fn invalid_legacy_runtime_settings_fail_closed_without_rewriting_the_file() {
        let invalid_documents = [
            r#"[runtime_panel]
delay_test_url = "file:///etc/passwd"
"#,
            "[runtime_panel]\ndelay_timeout_ms = 0\n",
            r#"[runtime_panel]
delay_sort = "unknown"
"#,
        ];
        for invalid in invalid_documents {
            let temp_dir = tempfile::tempdir().unwrap();
            let settings_file = temp_dir.path().join("settings.toml");
            fs::write(&settings_file, invalid).unwrap();

            assert!(load_settings(&settings_file).await.is_err());
            assert_eq!(fs::read_to_string(&settings_file).unwrap(), invalid);
        }
    }

    #[tokio::test]
    async fn legacy_json_settings_keep_nullable_fields_and_migrate_runtime_values_to_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        let legacy_json_path = settings_file.with_extension("json");
        fs::write(
            &legacy_json_path,
            r#"{
                "editor_path": null,
                "configs_dir": null,
                "language": "en-US",
                "runtime_panel": {
                    "auto_refresh": false,
                    "delay_sort": "delay_desc",
                    "delay_test_url": "https://probe.example.test/json",
                    "delay_timeout_ms": 42000
                }
            }"#,
        )
        .unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.language, "en-US");
        assert_eq!(
            loaded.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            loaded.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyDescending
        );
        assert_eq!(
            loaded.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/json", 42_000).unwrap()
        );
        let canonical = fs::read_to_string(&settings_file).unwrap();
        assert!(canonical.contains("settings_schema_version = 2"));
        assert!(!canonical.contains("delay_test_url"));
        assert!(!canonical.contains("delay_timeout_ms"));
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
        fs::write(&settings_file, legacy).unwrap();

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

        let raw = fs::read_to_string(&settings_file).unwrap();
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
        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("configs_dir"));
    }

    /// 旧版 settings.toml 没有 configs_dir 键：必须反序列化为 None 而不是报错。
    #[tokio::test]
    async fn test_legacy_settings_without_configs_dir_load_as_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

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
        fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

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

        let raw = fs::read_to_string(&settings_file).unwrap();
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
        fs::create_dir_all(&cloud).unwrap();
        let guard = test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        let manager = app_config_manager_in(&home).await.unwrap();
        let engine_dir = engine_config_dir(&cloud, &mihomo_config::engine_id());
        assert_eq!(manager.config_dir(), engine_dir.as_path());

        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), engine_dir.as_path());
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
        let engine_dir = engine_config_dir(&home.join("configs"), &mihomo_config::engine_id());
        assert_eq!(manager.config_dir(), engine_dir.as_path());

        // 字段为纯空白等价于未设置。
        guard.set_configs_dir(&home, Some("   ")).await;
        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), engine_dir.as_path());
    }

    /// 内存凭据存储（仿 session.rs / manager_test 的 MockStore 先例），
    /// `fail_set` 可注入写失败以覆盖迁移的降级路径。
    struct MemoryStore {
        entries: Mutex<HashMap<String, String>>,
        fail_set: bool,
    }

    impl MemoryStore {
        fn working() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_set: false,
            }
        }

        fn failing_set() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
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
        async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
            Ok(self.get(service, key))
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> result::Result<(), PortError> {
            if self.fail_set {
                return Err(PortError::Failed("injected keyring failure".to_string()));
            }
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
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

        let raw = fs::read_to_string(&settings_file).unwrap();
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
        fs::write(&settings_file, legacy_toml_with_password()).unwrap();

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

        let raw = fs::read_to_string(&settings_file).unwrap();
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
        fs::write(&settings_file, legacy_toml_with_password()).unwrap();

        let store = MemoryStore::failing_set();
        let loaded = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();

        assert_eq!(
            loaded.webdav.password, "s3cret",
            "value must stay in memory so the session keeps working"
        );
        let raw = fs::read_to_string(&settings_file).unwrap();
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
        let raw = fs::read_to_string(&settings_file).unwrap();
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
