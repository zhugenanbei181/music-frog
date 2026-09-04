//! WebDAV backup surface: credential/settings storage and validation plus
//! the on-demand sync run driven by the sync engine planner/executor.

use std::path::PathBuf;

use dav_client::DavClient;
use dav_client::client::WebDavClient;
use infiltrator_core::settings::{
    AppSettings, clear_webdav_password, load_settings, load_settings_hydrated_with_store,
    save_settings, save_webdav_password, settings_path,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_platform::paths::get_home_dir;
use mihomo_platform::traits::DefaultCredentialStore;
use state_store::StateStore;
use sync_engine::{SyncPlanner, executor::SyncExecutor};

use super::support::{get_runtime, map_anyhow_error, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- WebDAV API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<WebDavSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSyncResult {
    pub status: FfiStatus,
    pub success_count: u32,
    pub failed_count: u32,
    pub total_actions: u32,
}

#[uniffi::export]
pub async fn webdav_settings() -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_webdav_settings().await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_settings_save(settings: WebDavSettings) -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_webdav_settings(settings).await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_test(settings: WebDavSettings) -> FfiStatus {
    get_runtime()
        .spawn(async move { test_webdav_settings(settings).await })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn webdav_sync_now() -> WebDavSyncResult {
    get_runtime()
        .spawn(async move {
            match sync_webdav_now().await {
                Ok(summary) => WebDavSyncResult {
                    status: FfiStatus::ok(),
                    success_count: summary.success_count as u32,
                    failed_count: summary.failed_count as u32,
                    total_actions: summary.total_actions as u32,
                },
                Err(status) => WebDavSyncResult {
                    status,
                    success_count: 0,
                    failed_count: 0,
                    total_actions: 0,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSyncResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            success_count: 0,
            failed_count: 0,
            total_actions: 0,
        })
}

async fn load_webdav_settings() -> Result<WebDavSettings, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    load_webdav_settings_in(&home, &DefaultCredentialStore::default()).await
}

/// 同 [`load_webdav_settings`]，但 home 与 keyring 凭据存储均由调用方注入
/// （测试注入临时目录 + 内存实现，避免触碰全局 home override 与真实
/// OS keyring）。
async fn load_webdav_settings_in<S: SecureStore>(
    home: &std::path::Path,
    store: &S,
) -> Result<WebDavSettings, FfiStatus> {
    let settings = load_hydrated_app_settings_in(home, store).await?;
    Ok(webdav_settings_from_core(&settings.webdav))
}

async fn save_webdav_settings(settings: WebDavSettings) -> Result<WebDavSettings, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    save_webdav_settings_in(&home, settings, &DefaultCredentialStore::default()).await
}

/// 同 [`save_webdav_settings`]，但 home 与 keyring 凭据存储由调用方注入。
/// 密码只进 OS keyring：空串=清除条目，非空=写入；keyring 写失败时整体
/// 不落盘，保持「settings 文件 + keyring」状态一致（避免其他字段更新而
/// 凭据悄悄丢失，与 iced 桌面端保存语义一致）。
async fn save_webdav_settings_in<S: SecureStore>(
    home: &std::path::Path,
    settings: WebDavSettings,
    store: &S,
) -> Result<WebDavSettings, FfiStatus> {
    let mut app_settings = load_app_settings_in(home).await?;
    if settings.password.is_empty() {
        clear_webdav_password(store).await;
    } else {
        save_webdav_password(store, &settings.password)
            .await
            .map_err(map_anyhow_error)?;
    }
    let mut core_config = webdav_settings_to_core(settings.clone());
    // password 的序列化被 core 跳过；这里仍显式清空内存镜像，保证不落盘。
    core_config.password = String::new();
    app_settings.webdav = core_config;
    save_settings(
        &settings_path(home)
            .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?,
        &app_settings,
    )
    .await
    .map_err(map_anyhow_error)?;
    // 回显完整记录（含密码内存镜像），与迁移前 FFI 行为一致。
    Ok(settings)
}

async fn test_webdav_settings(settings: WebDavSettings) -> FfiStatus {
    crate::tls::ensure_rustls_provider();
    let config = webdav_settings_to_core(settings);
    if let Err(status) = validate_webdav_config(&config) {
        return status;
    }
    let dav = match WebDavClient::new(&config.url, &config.username, &config.password) {
        Ok(client) => client,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            );
        }
    };
    match dav.list("/").await {
        Ok(_) => FfiStatus::ok(),
        Err(err) => FfiStatus::err(
            FfiErrorCode::Network,
            format!("connection test failed: {err}"),
        ),
    }
}

#[derive(Debug, Default)]
struct WebDavSyncSummary {
    success_count: usize,
    failed_count: usize,
    total_actions: usize,
}

async fn sync_webdav_now() -> Result<WebDavSyncSummary, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    sync_webdav_now_in(&home, &DefaultCredentialStore::default()).await
}

/// 同 [`sync_webdav_now`]，但 home 与 keyring 凭据存储由调用方注入。
/// 密码经水合加载从 OS keyring 取回（settings.toml 已不携带明文）。
async fn sync_webdav_now_in<S: SecureStore>(
    home: &std::path::Path,
    store: &S,
) -> Result<WebDavSyncSummary, FfiStatus> {
    crate::tls::ensure_rustls_provider();
    let settings = load_hydrated_app_settings_in(home, store).await?;
    if !settings.webdav.enabled {
        return Err(FfiStatus::err(FfiErrorCode::NotReady, "WebDAV is disabled"));
    }
    // 本地同步根跟随 configs 目录重定向（env > settings.configs_dir > home/configs）。
    let local_root = mihomo_config::manager::paths::resolve_configs_dir_in(
        settings.configs_dir.as_deref(),
        home,
    )
    .map_err(map_mihomo_error)?;
    run_webdav_sync(&settings.webdav, local_root, home).await
}

async fn run_webdav_sync(
    config: &infiltrator_core::settings::WebDavConfig,
    local_root: PathBuf,
    home: &std::path::Path,
) -> Result<WebDavSyncSummary, FfiStatus> {
    validate_webdav_config(config)?;
    let dav =
        WebDavClient::new(&config.url, &config.username, &config.password).map_err(|err| {
            FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            )
        })?;

    if !local_root.exists() {
        tokio::fs::create_dir_all(&local_root)
            .await
            .map_err(|e| FfiStatus::err(FfiErrorCode::Io, e.to_string()))?;
    }
    let db_path = home.join("sync_state.db").to_string_lossy().to_string();
    let store = StateStore::new(&db_path).await.map_err(map_anyhow_error)?;

    let planner = SyncPlanner::new(local_root, "/".to_string(), &dav, &store);
    let actions = planner.build_plan().await.map_err(map_anyhow_error)?;

    if actions.is_empty() {
        return Ok(WebDavSyncSummary::default());
    }

    let executor = SyncExecutor::new(&dav, &store);
    let total_actions = actions.len();
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    for action in actions {
        match executor.execute(action).await {
            Ok(()) => success_count = success_count.saturating_add(1),
            Err(_) => failed_count = failed_count.saturating_add(1),
        }
    }

    Ok(WebDavSyncSummary {
        success_count,
        failed_count,
        total_actions,
    })
}

/// 无密码加载：settings.toml 本体（不含 keyring 明文）。仅限保存路径等
/// 不需要完整凭据的调用方。
async fn load_app_settings_in(home: &std::path::Path) -> Result<AppSettings, FfiStatus> {
    let path = settings_path(home)
        .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?;
    load_settings(&path).await.map_err(map_anyhow_error)
}

/// 水合加载：[`load_app_settings_in`] 之后把 OS keyring 中的 WebDAV 密码
/// （`webdav:password`）填回 `settings.webdav.password` 内存镜像。
/// password 的序列化被 core 跳过，因此水合值不会落盘。
async fn load_hydrated_app_settings_in<S: SecureStore>(
    home: &std::path::Path,
    store: &S,
) -> Result<AppSettings, FfiStatus> {
    let path = settings_path(home)
        .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?;
    load_settings_hydrated_with_store(&path, store)
        .await
        .map_err(map_anyhow_error)
}

fn webdav_settings_from_core(config: &infiltrator_core::settings::WebDavConfig) -> WebDavSettings {
    WebDavSettings {
        enabled: config.enabled,
        url: config.url.clone(),
        username: config.username.clone(),
        password: config.password.clone(),
        sync_interval_mins: config.sync_interval_mins,
        sync_on_startup: config.sync_on_startup,
    }
}

fn webdav_settings_to_core(settings: WebDavSettings) -> infiltrator_core::settings::WebDavConfig {
    infiltrator_core::settings::WebDavConfig {
        enabled: settings.enabled,
        url: settings.url.trim().to_string(),
        username: settings.username.trim().to_string(),
        password: settings.password,
        sync_interval_mins: settings.sync_interval_mins,
        sync_on_startup: settings.sync_on_startup,
    }
}

fn validate_webdav_config(
    config: &infiltrator_core::settings::WebDavConfig,
) -> Result<(), FfiStatus> {
    if config.url.trim().is_empty() {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "WebDAV URL is empty",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_core::settings::{WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 内存凭据存储（仿 core settings.rs 的 MemoryStore 先例），避免测试
    /// 触碰真实 OS keyring。
    struct MemoryStore {
        entries: Mutex<std::collections::HashMap<String, String>>,
    }

    impl Default for MemoryStore {
        fn default() -> Self {
            Self {
                entries: Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl MemoryStore {
        fn peek(&self, service: &str, key: &str) -> Option<String> {
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
            Ok(self.peek(service, key))
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
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

    /// 独立临时 home（同 crate 内既有测试的先例：手造唯一目录，结束时清理），
    /// 走 `_in` 通道注入，不触碰全局 home override。
    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("infiltrator-android-webdav-{tag}-{unique}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    fn record(password: &str) -> WebDavSettings {
        WebDavSettings {
            enabled: true,
            url: "https://dav.example.com".to_string(),
            username: "user".to_string(),
            password: password.to_string(),
            sync_interval_mins: 30,
            sync_on_startup: false,
        }
    }

    fn settings_file(home: &std::path::Path) -> PathBuf {
        settings_path(home).expect("settings path resolves")
    }

    /// 密码经 helper 往返（保存进 keyring、水合读回内存镜像），settings
    /// 文件全程无明文；空密码=清除 keyring 条目。走 home 注入的 `_in`
    /// 内部通道，不触碰全局 home override，也不触碰真实 OS keyring。
    #[tokio::test]
    async fn webdav_password_roundtrips_via_helper_and_settings_file_stays_clean() {
        let home = make_test_home("roundtrip");
        let store = MemoryStore::default();

        // 保存：密码只进 keyring。
        let saved = save_webdav_settings_in(&home, record("s3cret"), &store)
            .await
            .expect("save succeeds");
        assert_eq!(
            saved.password, "s3cret",
            "record echoes the in-memory value"
        );
        assert_eq!(
            store
                .peek(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
                .as_deref(),
            Some("s3cret"),
            "password must land in the credential store"
        );

        // settings 文件无明文。
        let raw = std::fs::read_to_string(settings_file(&home)).expect("settings file written");
        assert!(!raw.contains("password"), "plaintext leaked: {raw}");
        assert!(!raw.contains("s3cret"), "plaintext leaked: {raw}");
        assert!(
            raw.contains("enabled = true"),
            "other fields survive: {raw}"
        );

        // 读取：水合把 keyring 中的密码填回 FFI 记录。
        let loaded = load_webdav_settings_in(&home, &store)
            .await
            .expect("load succeeds");
        assert_eq!(loaded.password, "s3cret");
        assert_eq!(loaded.url, "https://dav.example.com");

        // 空密码=清除条目：keyring 为空，后续加载回到空串。
        let cleared = save_webdav_settings_in(&home, record(""), &store)
            .await
            .expect("clear-save succeeds");
        assert_eq!(cleared.password, "");
        assert_eq!(
            store.peek(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY),
            None
        );
        let reloaded = load_webdav_settings_in(&home, &store)
            .await
            .expect("reload succeeds");
        assert_eq!(reloaded.password, "");

        let raw_after = std::fs::read_to_string(settings_file(&home)).expect("settings readable");
        assert!(
            !raw_after.contains("s3cret"),
            "plaintext leaked: {raw_after}"
        );
        let _ = std::fs::remove_dir_all(home);
    }

    /// 同步入口的禁用短路：WebDAV 未启用时水合加载照常工作，但同步拒绝
    /// 执行（NotReady），不创建任何 sync 状态。
    #[tokio::test]
    async fn sync_webdav_now_in_rejects_when_disabled() {
        let home = make_test_home("sync-disabled");
        let store = MemoryStore::default();
        // 未保存过任何设置：webdav.enabled 缺省 false。
        let err = sync_webdav_now_in(&home, &store)
            .await
            .expect_err("disabled webdav must fail the sync");
        assert_eq!(err.code, FfiErrorCode::NotReady);
        assert!(format!("{:?}", err.message).contains("disabled"));
        let _ = std::fs::remove_dir_all(home);
    }
}
