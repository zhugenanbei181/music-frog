//! Shared plumbing for the FFI surface: the global tokio runtime, generic
//! string/list normalization helpers, error mapping into [`FfiStatus`], and
//! the mihomo controller client used by every live-query passthrough.

use std::sync::OnceLock;

use mihomo_api::client::MihomoClient;
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_platform::paths::get_home_dir;
use tokio::runtime::Runtime;

use super::session::shared_core;
use crate::ffi::{FfiErrorCode, FfiStatus};
pub(super) fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create tokio runtime"))
}

pub(super) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(super) fn sanitize_list(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.map(|items| {
        items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

/// `settings.configs_dir` redirect target, already trimmed by the manager
/// resolution; `None` keeps the default `<home>/configs` behavior. A settings
/// load failure must not block the default path, so it degrades to `None`.
pub(super) async fn configs_dir_override() -> Option<String> {
    let home = mihomo_platform::paths::get_home_dir().ok()?;
    let path = infiltrator_core::settings_io::settings_path(&home).ok()?;
    match infiltrator_core::settings_io::load_settings(&path).await {
        Ok(settings) => settings.configs_dir,
        Err(err) => {
            log::warn!("settings load failed, configs_dir override ignored: {err:#}");
            None
        }
    }
}

/// ConfigManager wired for the configs-dir redirect. The `INFILTRATOR_CONFIGS_DIR`
/// env keeps priority over the settings field inside `resolve_configs_dir_in`;
/// with no override anywhere this uses `<home>/configs`.
pub(super) async fn build_config_manager()
-> Result<ConfigManager<DefaultCredentialStore>, FfiStatus> {
    let home = get_home_dir().map_err(map_mihomo_error)?;
    let override_dir = configs_dir_override().await;
    ConfigManager::with_home_configs_dir_and_store(
        home,
        override_dir.as_deref(),
        DefaultCredentialStore::default(),
    )
    .map_err(map_mihomo_error)
}

pub(super) async fn build_controller_client() -> Result<MihomoClient, FfiStatus> {
    // Prefer the shared endpoint port: URL and secret are re-resolved from
    // the current profile on every call (port rotation and secret aware).
    if let Ok(core) = shared_core().await {
        match core.endpoints.resolve().await {
            Ok(endpoint) => match MihomoClient::new(&endpoint.url, endpoint.secret) {
                Ok(client) => return Ok(client),
                Err(err) => {
                    log::debug!(
                        "application endpoint client unavailable, using legacy resolution: {err}"
                    );
                }
            },
            Err(err) => {
                log::debug!("application endpoint unavailable, using legacy resolution: {err}");
            }
        }
    }
    let manager = build_config_manager().await?;
    let controller_url = match manager.get_external_controller().await {
        Ok(url) => url,
        Err(err) => {
            if let Some(bridge) = get_android_bridge()
                && let Some(url) = bridge.core_controller_url()
            {
                return MihomoClient::new(&url, None).map_err(map_mihomo_error);
            }
            return Err(map_mihomo_error(err));
        }
    };
    MihomoClient::new(&controller_url, None).map_err(map_mihomo_error)
}

pub(super) fn map_anyhow_error(err: anyhow::Error) -> FfiStatus {
    if let Some(source) = err.downcast_ref::<MihomoError>() {
        return map_mihomo_error_ref(source);
    }
    FfiStatus::err(FfiErrorCode::Unknown, err.to_string())
}

pub(super) fn map_mihomo_error(err: MihomoError) -> FfiStatus {
    map_mihomo_error_ref(&err)
}

fn map_mihomo_error_ref(err: &MihomoError) -> FfiStatus {
    match err {
        MihomoError::Http(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Io(_) => FfiStatus::err(FfiErrorCode::Io, err.to_string()),
        MihomoError::Json(_) | MihomoError::Yaml(_) | MihomoError::YamlEmit(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::UrlParse(_) => FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string()),
        MihomoError::WebSocket(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Config(_) | MihomoError::Service(_) | MihomoError::Version(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::Proxy(_) | MihomoError::NotFound(_) => {
            FfiStatus::err(FfiErrorCode::NotReady, err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_core::settings_io::save_settings;
    use infiltrator_domain::settings::AppSettings;
    use mihomo_platform::TEST_LOCK;
    use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

    async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("infiltrator-android-cc-{tag}-{unique}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    async fn write_settings_with_configs_dir(home: &std::path::Path, configs_dir: &str) {
        let settings = AppSettings {
            configs_dir: Some(configs_dir.to_string()),
            ..AppSettings::default()
        };
        let path = infiltrator_core::settings_io::settings_path(home).expect("settings path resolves");
        save_settings(&path, &settings)
            .await
            .expect("save settings");
    }

    fn set_env(value: &str) {
        unsafe { std::env::set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { std::env::remove_var(CONFIGS_DIR_ENV) };
    }

    fn restore_env(saved: Option<String>) {
        match saved {
            Some(value) => set_env(&value),
            None => clear_env(),
        }
    }

    async fn configs_parent(manager: &ConfigManager<DefaultCredentialStore>) -> PathBuf {
        manager
            .get_current_path()
            .await
            .expect("current profile path resolves")
            .parent()
            .expect("profile path has a parent")
            .to_path_buf()
    }

    #[tokio::test]
    async fn config_manager_follows_settings_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("settings-redirect");
        set_home_dir_override(home.clone());
        write_settings_with_configs_dir(&home, "cloud/profiles").await;

        let manager = build_config_manager()
            .await
            .expect("manager builds with settings override");
        assert_eq!(
            configs_parent(&manager).await,
            home.join("cloud").join("profiles")
        );

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn env_configs_dir_wins_over_settings_field() {
        let _guard = test_lock().await;
        let home = make_test_home("env-priority");
        set_home_dir_override(home.clone());
        write_settings_with_configs_dir(&home, "cloud/profiles").await;

        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        let env_dir = home.join("env-cloud");
        set_env(env_dir.to_str().unwrap());

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(configs_parent(&manager).await, env_dir);

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn no_override_keeps_default_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("default-dir");
        set_home_dir_override(home.clone());

        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        clear_env();

        let manager = build_config_manager().await.expect("manager builds");
        assert_eq!(configs_parent(&manager).await, home.join("configs"));

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }
}
